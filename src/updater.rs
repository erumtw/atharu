//! Updating Atharu from a release tag (D28, D40; the Settings › Update button, 0.2.6). Shared by `atharu update`
//! (cli/update.rs) and the Mac app's button (the settings slice), like `recording/`: both read the newest release tag from the
//! checkout the binary was built from and run the same `scripts/install.sh --version <tag>`, which builds that tag in a temp
//! worktree and never touches the working tree.
//!
//! The button runs a build script from the server, so nothing a request carries reaches the command line: the program, its
//! arguments (`--version <tag> --app`) and the log path are fixed here; the tag comes from `git tag --list` and passed
//! [`semver`].
use crate::build_info;
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// The checkout to read tags from: `ATHARU_REPO` at run time (e.g. the clone moved), else the one baked in
/// by install.sh (it builds from a temp worktree, so CARGO_MANIFEST_DIR is gone by then), else the source dir.
pub fn repo() -> String {
    std::env::var("ATHARU_REPO").unwrap_or_else(|_| {
        option_env!("ATHARU_REPO")
            .unwrap_or(env!("CARGO_MANIFEST_DIR"))
            .to_string()
    })
}

/// This binary's release version, `None` for a dev build (not built from a release tag).
pub fn installed() -> Option<&'static str> {
    build_info::TAG.map(|_| build_info::VERSION)
}

/// `git -C repo …` → trimmed stdout, or git's own stderr as the refusal.
fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("can't run git: {e}"))?;
    if !o.status.success() {
        let why = String::from_utf8_lossy(&o.stderr).trim().to_string();
        return Err(format!(
            "git {} failed in {}: {why}",
            args[0],
            repo.display()
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// "1.2.3" or "v1.2.3-rc1" → ((1, 2, 3), is a final release); a final release outranks its pre-releases.
pub fn semver(s: &str) -> Option<((u64, u64, u64), bool)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let (core, pre) = s.split_once('-').map_or((s, false), |(c, _)| (c, true));
    let mut n = core.split('.').map(|p| p.parse::<u64>().ok());
    let v = (n.next()??, n.next()??, n.next()??);
    n.next().is_none().then_some((v, !pre))
}

/// Fetch the tags (a failure is only noted: the local tags still answer), then the newest release tag by semver, the same pick
/// as install.sh (pre-releases only when pinned by hand). `Ok(None)` = no release tags yet.
pub fn newest(repo: &Path) -> Result<Option<String>, String> {
    newest_noting(repo, &mut |_| {})
}

/// [`newest`], telling `note` why the fetch failed (the CLI prints it as a warning).
pub fn newest_noting(repo: &Path, note: &mut dyn FnMut(&str)) -> Result<Option<String>, String> {
    if let Err(why) = git(repo, &["fetch", "--tags", "--quiet"]) {
        note(&why);
    }
    let tags = git(repo, &["tag", "--list", "v[0-9]*", "--sort=-v:refname"])?;
    Ok(tags
        .lines()
        .find(|t| semver(t).is_some_and(|(_, release)| release))
        .map(str::to_string))
}

/// Is `tag` newer than this build? A dev build (`None`) is older than any release.
pub fn is_newer(installed: Option<&str>, tag: &str) -> bool {
    match installed {
        Some(v) => semver(v) < semver(tag),
        None => true,
    }
}

/// Where the Settings button is: one update at a time, in memory (a restart ends it).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum UpdateState {
    #[default]
    Idle,
    /// Checked: this is the newest tag and it is what runs.
    UpToDate(String),
    /// Checked: a newer tag exists.
    Available(String),
    /// `install.sh` is running; its last line is tailed from the log when the page asks ([`tail`]).
    Running {
        to: String,
        started: Instant,
    },
    Done(String),
    Failed {
        /// The tag being installed; empty when the failure was the check itself.
        to: String,
        why: String,
    },
}

/// The button's state plus `busy`: an install process is alive. It outlives a timed-out row (the page stops waiting, the install
/// does not), and no second install starts while it is set.
#[derive(Default)]
pub struct Updater {
    state: Mutex<UpdateState>,
    busy: AtomicBool,
}

pub type Shared = Arc<Updater>;

pub fn lock(s: &Shared) -> MutexGuard<'_, UpdateState> {
    s.state.lock().unwrap_or_else(|e| e.into_inner())
}

/// An install is still running, even if the row already gave up on it.
pub fn busy(s: &Shared) -> bool {
    s.busy.load(Ordering::SeqCst)
}

/// How long an install may take before the page gives up waiting for it.
const DEADLINE: Duration = Duration::from_secs(20 * 60);

/// Start `scripts/install.sh --version <tag> --app` in the background; `false` = one is already running. Its output goes to
/// `log` (overwritten each run), a file, never a pipe: when the app restarts mid-install nothing is left to SIGPIPE the build.
pub fn start(state: &Shared, repo: PathBuf, tag: String, log: PathBuf) -> bool {
    start_with(state, repo, tag, log, DEADLINE)
}

/// [`start`] with the deadline as a parameter (tests use a short one).
pub fn start_with(
    state: &Shared,
    repo: PathBuf,
    tag: String,
    log: PathBuf,
    deadline: Duration,
) -> bool {
    // Belt and braces: the tag reaches the command line, so it must look like one (`newest` only ever returns such).
    if !tag.starts_with('v') || semver(&tag).is_none_or(|(_, release)| !release) {
        return false;
    }
    // One install at a time, across retries after a timeout too: `busy` is held until the process itself is gone.
    if state.busy.swap(true, Ordering::SeqCst) {
        return false;
    }
    *lock(state) = UpdateState::Running {
        to: tag.clone(),
        started: Instant::now(),
    };
    let (done, waiting) = mpsc::channel::<()>();
    let (worker, t) = (state.clone(), tag.clone());
    std::thread::spawn(move || {
        let res = run(&repo, &t, &log);
        // The final state first, `busy` after it: a start that slips in once `busy` is clear can't be overwritten.
        *lock(&worker) = match res {
            Ok(()) => UpdateState::Done(t),
            Err(why) => UpdateState::Failed { to: t, why },
        };
        worker.busy.store(false, Ordering::SeqCst);
        let _ = done.send(());
    });
    // The deadline: past it the row stops waiting. The install itself is left alone (killing a half-done build or bundle
    // swap would be worse than waiting) and, when it ends, the worker above still writes how it went.
    let timer = state.clone();
    std::thread::spawn(move || {
        if waiting.recv_timeout(deadline) == Err(mpsc::RecvTimeoutError::Timeout) {
            let mut s = lock(&timer);
            if matches!(*s, UpdateState::Running { .. }) {
                *s = UpdateState::Failed {
                    to: tag,
                    why: format!(
                        "Took longer than {} min; still running in the background, see the log",
                        deadline.as_secs().div_ceil(60).max(1)
                    ),
                };
            }
        }
    });
    true
}

/// Runs the script to its end (no deadline here) and says how it went.
fn run(repo: &Path, tag: &str, log: &Path) -> Result<(), String> {
    let from = installed().map_or("dev".to_string(), |v| format!("v{v}"));
    let out =
        std::fs::File::create(log).map_err(|e| format!("can't write {}: {e}", log.display()))?;
    let err = out
        .try_clone()
        .map_err(|e| format!("can't write {}: {e}", log.display()))?;
    let status = Command::new("bash")
        .arg(repo.join("scripts/install.sh"))
        .args(["--version", tag, "--app"])
        // This server runs inside the app install.sh replaces: it must not quit it (see install.sh).
        .env("ATHARU_UPDATE_FROM_APP", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .status()
        .map_err(|e| format!("can't run scripts/install.sh: {e}"))?;
    if status.success() {
        return Ok(());
    }
    // install.sh refuses to run beside another install (its lock): say that, not a bare exit code.
    if let Some(rest) = tail(log).strip_prefix("another install is running") {
        return Err(format!(
            "Another install is running{rest}. Nothing was changed."
        ));
    }
    let code = status
        .code()
        .map_or("a signal".to_string(), |c| c.to_string());
    Err(format!(
        "install.sh exited {code}; {from} is still installed"
    ))
}

/// The last readable line of the install's log, for the page's progress line (the file's tail, not all of it).
pub fn tail(log: &Path) -> String {
    let Ok(mut f) = std::fs::File::open(log) else {
        return String::new();
    };
    let len = f.metadata().map_or(0, |m| m.len());
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(4096)));
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf)
        .lines()
        .rev()
        .map(clean)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
}

/// A progress line as the page shows it: the part after the last carriage return, escape sequences and control characters
/// dropped, at most 120 characters.
fn clean(raw: &str) -> String {
    let seg = raw
        .rsplit('\r')
        .find(|s| !s.trim().is_empty())
        .unwrap_or("");
    let mut out = String::new();
    let mut chars = seg.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // `ESC [ … letter`: colours and cursor moves.
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c == '\t' {
            out.push(' ');
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out.trim().chars().take(120).collect()
}

/// Read a settled state without holding the lock.
pub fn snapshot(s: &Shared) -> UpdateState {
    lock(s).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_orders_numerically_and_final_after_pre() {
        assert!(semver("v0.10.0") > semver("v0.9.9"));
        assert!(semver("0.3.0") > semver("v0.3.0-rc1"));
        assert_eq!(semver("v1.2"), None);
        assert_eq!(semver("v1.2.3.4"), None);
    }

    #[test]
    fn is_newer_table() {
        // A dev build is older than any release.
        assert!(is_newer(None, "v0.2.6"));
        assert!(is_newer(Some("0.2.5"), "v0.2.6"));
        assert!(is_newer(Some("0.2.9"), "v0.10.0"));
        assert!(!is_newer(Some("0.2.6"), "v0.2.6"));
        assert!(!is_newer(Some("0.3.0"), "v0.2.6"));
        // A pre-release is older than its final release, newer than the release before it.
        assert!(is_newer(Some("0.3.0-rc1"), "v0.3.0"));
        assert!(!is_newer(Some("0.3.0"), "v0.3.0-rc1"));
        assert!(is_newer(Some("0.2.6"), "v0.3.0-rc1"));
    }

    #[test]
    fn output_lines_are_cleaned_for_the_page() {
        assert_eq!(
            clean("   Compiling atharu v0.2.7\n"),
            "Compiling atharu v0.2.7"
        );
        // cargo's progress rewrites one line with \r: the last readable part wins.
        assert_eq!(clean("Building 1/9\rBuilding 7/9\r\n"), "Building 7/9");
        assert_eq!(
            clean("\x1b[1;32mFinished\x1b[0m release\n"),
            "Finished release"
        );
        assert_eq!(clean(&"x".repeat(300)).chars().count(), 120);
        assert_eq!(clean("\n"), "");
    }

    #[test]
    fn a_second_start_while_running_is_refused() {
        let state: Shared = Arc::default();
        state.busy.store(true, Ordering::SeqCst);
        *lock(&state) = UpdateState::Running {
            to: "v0.2.7".into(),
            started: Instant::now(),
        };
        assert!(!start(
            &state,
            PathBuf::from("/nonexistent"),
            "v0.2.8".into(),
            std::env::temp_dir().join("atharu-update-refused.log")
        ));
        // The running one is untouched.
        assert!(matches!(snapshot(&state), UpdateState::Running { to, .. } if to == "v0.2.7"));
        // Nor does a tag that isn't a release tag start anything.
        state.busy.store(false, Ordering::SeqCst);
        *lock(&state) = UpdateState::Idle;
        for bad in ["v0.2.8 --evil", "main", "v1.2", "v0.3.0-rc1", "$(x)"] {
            assert!(
                !start(
                    &state,
                    PathBuf::from("/nonexistent"),
                    bad.into(),
                    PathBuf::from("/nonexistent/x")
                ),
                "{bad}"
            );
        }
        assert_eq!(snapshot(&state), UpdateState::Idle);
    }

    /// A temp repo whose `scripts/install.sh` is `body`; `settle` waits (up to ~8 s) until the run is over for the page.
    fn run_fake(name: &str, body: &str, deadline: Duration) -> (Shared, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("atharu-updater-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("scripts/install.sh"), body).unwrap();
        let state: Shared = Arc::default();
        let log = dir.join("update.log");
        assert!(start_with(
            &state,
            dir.clone(),
            "v0.2.7".into(),
            log.clone(),
            deadline
        ));
        (state, log)
    }

    fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
        for _ in 0..160 {
            if f() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out waiting for {what}");
    }

    fn settled(state: &Shared) -> bool {
        !matches!(snapshot(state), UpdateState::Running { .. })
    }

    #[test]
    fn a_script_run_from_the_app_gets_the_flag_and_logs_to_a_file() {
        // Asserts the fixed env (install.sh must not quit the app) and the fixed args; three lines, the last one is the tail.
        let (state, log) = run_fake(
            "ok",
            "[ \"$ATHARU_UPDATE_FROM_APP\" = 1 ] || exit 9\necho \"args: $*\"\necho 'step two' >&2\necho '   Finished release   '\n",
            Duration::from_secs(30),
        );
        wait_until("done", || settled(&state) && !busy(&state));
        assert_eq!(snapshot(&state), UpdateState::Done("v0.2.7".into()));
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("args: --version v0.2.7 --app"), "{text}");
        assert!(text.contains("step two"), "{text}");
        assert_eq!(tail(&log), "Finished release");
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn a_failing_script_ends_failed_with_its_code() {
        let (state, log) = run_fake("fail", "echo boom\nexit 3\n", Duration::from_secs(30));
        wait_until("failed", || settled(&state) && !busy(&state));
        match snapshot(&state) {
            UpdateState::Failed { to, why } => {
                assert_eq!(to, "v0.2.7");
                assert!(why.starts_with("install.sh exited 3; "), "{why}");
                assert!(why.ends_with("is still installed"), "{why}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(tail(&log), "boom");
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn install_sh_refusing_to_run_beside_another_is_said_in_words() {
        let (state, log) = run_fake(
            "locked",
            "echo 'another install is running (pid 4242)'\nexit 1\n",
            Duration::from_secs(30),
        );
        wait_until("failed", || settled(&state) && !busy(&state));
        match snapshot(&state) {
            UpdateState::Failed { why, .. } => assert_eq!(
                why,
                "Another install is running (pid 4242). Nothing was changed."
            ),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn past_the_deadline_the_row_gives_up_but_the_install_still_holds_the_slot() {
        let (state, log) = run_fake(
            "slow",
            "echo started\nsleep 2\necho finished\n",
            Duration::from_millis(300),
        );
        wait_until("the timeout row", || settled(&state));
        match snapshot(&state) {
            UpdateState::Failed { why, .. } => {
                assert!(why.starts_with("Took longer than "), "{why}");
                assert!(why.contains("still running in the background"), "{why}");
            }
            other => panic!("{other:?}"),
        }
        // The process is alive: still busy, and a retry is refused (it would fight over the same worktree and app bundle).
        assert!(busy(&state));
        assert!(!start_with(
            &state,
            PathBuf::from("/nonexistent"),
            "v0.2.8".into(),
            std::env::temp_dir().join("atharu-update-busy.log"),
            Duration::from_secs(1)
        ));
        // When it ends, the row says how it went and the slot is free again.
        wait_until("the end", || !busy(&state));
        assert_eq!(snapshot(&state), UpdateState::Done("v0.2.7".into()));
        assert_eq!(tail(&log), "finished");
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }
}
