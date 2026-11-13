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
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
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
    /// `install.sh` is running; `last` is the last line it printed.
    Running {
        to: String,
        started: Instant,
        last: String,
    },
    Done(String),
    Failed {
        /// The tag being installed; empty when the failure was the check itself.
        to: String,
        why: String,
    },
}

pub type Shared = Arc<Mutex<UpdateState>>;

pub fn lock(s: &Shared) -> MutexGuard<'_, UpdateState> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start `scripts/install.sh --version <tag> --app` in the background; `false` = one is already running. The output goes to
/// `log` (overwritten each run) and its last line to the state; nowhere else.
pub fn start(state: &Shared, repo: PathBuf, tag: String, log: PathBuf) -> bool {
    // Belt and braces: the tag reaches the command line, so it must look like one (`newest` only ever returns such).
    if !tag.starts_with('v') || semver(&tag).is_none_or(|(_, release)| !release) {
        return false;
    }
    {
        let mut s = lock(state);
        if matches!(*s, UpdateState::Running { .. }) {
            return false;
        }
        *s = UpdateState::Running {
            to: tag.clone(),
            started: Instant::now(),
            last: String::new(),
        };
    }
    let state = state.clone();
    std::thread::spawn(move || {
        let res = run(&state, &repo, &tag, &log);
        *lock(&state) = match res {
            Ok(()) => UpdateState::Done(tag),
            Err(why) => UpdateState::Failed { to: tag, why },
        };
    });
    true
}

fn run(state: &Shared, repo: &Path, tag: &str, log: &Path) -> Result<(), String> {
    let from = installed().map_or("dev".to_string(), |v| format!("v{v}"));
    // The log is for reading a failure afterwards; not being able to write it must not stop the update.
    let log = std::fs::File::create(log)
        .ok()
        .map(|f| Arc::new(Mutex::new(f)));
    let mut child = Command::new("bash")
        .arg(repo.join("scripts/install.sh"))
        .args(["--version", tag, "--app"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("can't run scripts/install.sh: {e}"))?;
    let pumps: Vec<_> = [
        child.stdout.take().map(|r| pump(r, state, &log)),
        child.stderr.take().map(|r| pump(r, state, &log)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let status = child
        .wait()
        .map_err(|e| format!("can't wait for scripts/install.sh: {e}"))?;
    for p in pumps {
        let _ = p.join();
    }
    if status.success() {
        return Ok(());
    }
    let code = status
        .code()
        .map_or("a signal".to_string(), |c| c.to_string());
    Err(format!(
        "install.sh exited {code}; {from} is still installed"
    ))
}

/// One output stream: every line to the log file, and the last readable one to the state.
fn pump<R: Read + Send + 'static>(
    r: R,
    state: &Shared,
    log: &Option<Arc<Mutex<std::fs::File>>>,
) -> std::thread::JoinHandle<()> {
    let (state, log) = (state.clone(), log.clone());
    std::thread::spawn(move || {
        let mut r = BufReader::new(r);
        let mut buf = Vec::new();
        while r.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
            if let Some(f) = &log {
                let _ = f.lock().unwrap_or_else(|e| e.into_inner()).write_all(&buf);
            }
            let line = clean(&String::from_utf8_lossy(&buf));
            buf.clear();
            if line.is_empty() {
                continue;
            }
            if let UpdateState::Running { last, .. } = &mut *lock(&state) {
                *last = line;
            }
        }
    })
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
        *lock(&state) = UpdateState::Running {
            to: "v0.2.7".into(),
            started: Instant::now(),
            last: String::new(),
        };
        assert!(!start(
            &state,
            PathBuf::from("/nonexistent"),
            "v0.2.8".into(),
            std::env::temp_dir().join("atharu-update-refused.log")
        ));
        // The running one is untouched.
        assert!(matches!(snapshot(&state), UpdateState::Running { to, .. } if to == "v0.2.7"));
    }

    #[test]
    fn a_failing_script_ends_failed_with_the_log_and_the_last_line() {
        let dir = std::env::temp_dir().join(format!("atharu-updater-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(
            dir.join("scripts/install.sh"),
            "echo \"args: $*\"; echo 'step two' >&2; exit 3\n",
        )
        .unwrap();
        let state: Shared = Arc::default();
        let log = dir.join("update.log");
        assert!(start(&state, dir.clone(), "v0.2.7".into(), log.clone()));
        for _ in 0..100 {
            if !matches!(snapshot(&state), UpdateState::Running { .. }) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        match snapshot(&state) {
            UpdateState::Failed { to, why } => {
                assert_eq!(to, "v0.2.7");
                assert!(why.starts_with("install.sh exited 3; "), "{why}");
                assert!(why.ends_with("is still installed"), "{why}");
            }
            other => panic!("{other:?}"),
        }
        // The fixed arguments, and both streams, are in the log.
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("args: --version v0.2.7 --app"), "{text}");
        assert!(text.contains("step two"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
