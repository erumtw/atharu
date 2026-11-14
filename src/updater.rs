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
    /// The progress bar's memory while an install runs: the step it is in, since when, and the furthest the bar has shown.
    track: Mutex<Track>,
}

/// What the bar remembers between polls (reset at every start).
pub struct Track {
    step: u8,
    step_started: Instant,
    max_shown: f32,
}

impl Default for Track {
    fn default() -> Track {
        Track {
            step: 0,
            step_started: Instant::now(),
            max_shown: 0.0,
        }
    }
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
    *state.track.lock().unwrap_or_else(|e| e.into_inner()) = Track::default();
    let (done, waiting) = mpsc::channel::<()>();
    let (worker, t) = (state.clone(), tag.clone());
    std::thread::spawn(move || {
        let res = run(&repo, &t, &log);
        // A whole run teaches the next run's estimate (a partial or failed one would teach nonsense).
        if res.is_ok() {
            record_times(&log);
        }
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

// ---- The progress bar (0.2.8) ------------------------------------------------------------------------------------------------
// install.sh prints "▸ step N/M name" when a phase starts and "⏱ name: XmYYs" when it ends. The bar is real per step (each step's
// share is how long it took LAST time, kept in update-times.txt next to the log) and shaped for the eye: it starts slowly and
// speeds up ([`shown`]: studies of progress bars find an accelerating bar is judged faster than a steady one, and a bar that
// slows at the end worst), it never moves backwards and never reaches 100 % before the install is done.

/// A phase as install.sh announced it.
#[derive(Debug, PartialEq)]
pub struct Step {
    pub n: u8,
    pub of: u8,
    pub name: &'static str,
}

/// The owner's words for a phase.
pub fn label(name: &str) -> &'static str {
    match name {
        "source" => "Getting the source",
        "cli" => "Building atharu",
        "app" => "Building the Mac app",
        "swap" => "Replacing the app",
        _ => "Installing",
    }
}

/// The last `▸ step N/M name` line in the log's tail (the last 8 KB), if any.
pub fn current_step(log: &Path) -> Option<Step> {
    let text = read_tail(log, 8192);
    text.lines().rev().find_map(|l| {
        let rest = l.trim().strip_prefix("▸ step ")?;
        let (frac, name) = rest.split_once(' ')?;
        let (n, of) = frac.split_once('/')?;
        let name = ["source", "cli", "app", "swap"]
            .into_iter()
            .find(|k| *k == name.trim())?;
        Some(Step {
            n: n.parse().ok()?,
            of: of.parse().ok()?,
            name,
        })
    })
}

fn read_tail(log: &Path, bytes: u64) -> String {
    let Ok(mut f) = std::fs::File::open(log) else {
        return String::new();
    };
    let len = f.metadata().map_or(0, |m| m.len());
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(bytes)));
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

/// Seconds each phase took last time (source, cli, app, swap); `known` = read from a file, not the defaults.
#[derive(Debug, PartialEq)]
pub struct History {
    pub secs: [u32; 4],
    pub known: bool,
}

pub const DEFAULT_TIMES: [u32; 4] = [10, 60, 90, 5];
const TIME_KEYS: [&str; 4] = ["source", "cli", "app", "swap"];

/// `update-times.txt`, next to the log.
pub fn times_path(log: &Path) -> PathBuf {
    log.with_file_name("update-times.txt")
}

/// Missing or garbled → the defaults, and no time estimate is shown (`known` false).
pub fn read_history(path: &Path) -> History {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut secs = [0u32; 4];
    let mut seen = 0;
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if let (Some(i), Ok(v)) = (
            TIME_KEYS.iter().position(|t| *t == k.trim()),
            v.trim().parse::<u32>(),
        ) {
            secs[i] = v.max(1);
            seen |= 1 << i;
        }
    }
    if seen == 0b1111 {
        History { secs, known: true }
    } else {
        History {
            secs: DEFAULT_TIMES,
            known: false,
        }
    }
}

/// Written whole to a temp file, then renamed over, so a poll never reads half of it.
pub fn write_history(path: &Path, secs: [u32; 4]) -> std::io::Result<()> {
    let body: String = TIME_KEYS
        .iter()
        .zip(secs)
        .map(|(k, v)| format!("{k}={v}\n"))
        .collect();
    let tmp = path.with_extension("txt.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(tmp, path)
}

/// The four `⏱ <name>: XmYYs` lines of a whole run (source, cli, app build, swap), in seconds; `None` unless all four are there.
pub fn parse_timings(log: &str) -> Option<[u32; 4]> {
    let mut out = [None; 4];
    for line in log.lines() {
        let Some((name, dur)) = line
            .trim()
            .strip_prefix("⏱ ")
            .and_then(|r| r.split_once(": "))
        else {
            continue;
        };
        let i = match name {
            "source" => 0,
            "cli" => 1,
            "app build" => 2,
            "swap" => 3,
            _ => continue,
        };
        let (m, sec) = dur.trim().trim_end_matches('s').split_once('m')?;
        out[i] = Some(m.parse::<u32>().ok()? * 60 + sec.parse::<u32>().ok()?);
    }
    Some([out[0]?, out[1]?, out[2]?, out[3]?])
}

fn record_times(log: &Path) {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    if let Some(secs) = parse_timings(&text) {
        let _ = write_history(&times_path(log), secs);
    }
}

/// How far the work really is, 0..1: the finished steps' expected seconds plus the current step's elapsed share (at most 95 % of
/// it: a step that has not announced its end is never counted as done), over the expected total.
pub fn real_fraction(done_secs: &[u32], cur_expected: u32, cur_elapsed: u32, total: u32) -> f32 {
    let done: u32 = done_secs.iter().sum();
    let cur = (cur_elapsed as f32 / cur_expected.max(1) as f32).min(0.95) * cur_expected as f32;
    ((done as f32 + cur) / total.max(1) as f32).clamp(0.0, 1.0)
}

/// What the bar shows for `real` work done (the perceived curve, owner's choice): slow start, fast finish, `0.97 × real^1.6`.
/// Never 100 % (that is the Done row).
pub fn shown(real: f32) -> f32 {
    0.97 * real.clamp(0.0, 1.0).powf(1.6)
}

/// "about 2 min left" / "less than a minute left" / "taking longer than usual" / nothing. Only with a history to go by.
pub fn eta(known: bool, done_secs: u32, cur_expected: u32, cur_elapsed: u32, total: u32) -> String {
    if !known {
        return String::new();
    }
    if cur_elapsed as f32 > 1.5 * cur_expected as f32 {
        return "taking longer than usual".into();
    }
    match total.saturating_sub(done_secs + cur_elapsed) {
        90.. => format!(
            "about {} min left",
            total.saturating_sub(done_secs + cur_elapsed).div_ceil(60)
        ),
        15..=89 => "less than a minute left".into(),
        _ => String::new(),
    }
}

/// What the Update row draws while it runs.
#[derive(Debug, PartialEq)]
pub struct Progress {
    pub pct: u32,
    pub n: u8,
    pub of: u8,
    pub label: &'static str,
    pub eta: String,
}

/// Reads the log's current step and moves the bar: monotonic (the furthest shown is kept), capped below 100. `None` unless an
/// install is running. Called by each poll of the row (every 2 s), which is also how a step change is noticed.
pub fn progress(state: &Shared, log: &Path) -> Option<Progress> {
    if !matches!(*lock(state), UpdateState::Running { .. }) {
        return None;
    }
    let step = current_step(log);
    let hist = read_history(&times_path(log));
    let mut t = state.track.lock().unwrap_or_else(|e| e.into_inner());
    // Before the first marker (the script has only just started) it is step 1 of an app update.
    let (n, of, name) = step
        .as_ref()
        .map_or((1, 4, "source"), |s| (s.n, s.of, s.name));
    if t.step != n {
        t.step = n;
        t.step_started = Instant::now();
    }
    let expected = &hist.secs[..(of as usize).clamp(1, 4)];
    let idx = (n as usize).clamp(1, expected.len()) - 1;
    let (done, cur_expected) = (&expected[..idx], expected[idx]);
    let elapsed = t.step_started.elapsed().as_secs() as u32;
    let total: u32 = expected.iter().sum();
    let now = shown(real_fraction(done, cur_expected, elapsed, total));
    t.max_shown = t.max_shown.max(now);
    Some(Progress {
        pct: ((t.max_shown * 100.0) as u32).min(97),
        n,
        of,
        label: label(name),
        eta: eta(hist.known, done.iter().sum(), cur_expected, elapsed, total),
    })
}

/// The last readable line of the install's log, for the page's progress line (the file's tail, not all of it).
pub fn tail(log: &Path) -> String {
    read_tail(log, 4096)
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

    // ---- the progress bar ----

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("atharu-prog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn shown_starts_slowly_and_finishes_fast() {
        for (real, want) in [
            (0.0, 0.0),
            (0.25, 0.106),
            (0.5, 0.32),
            (0.75, 0.61),
            (0.9, 0.82),
            (1.0, 0.97),
        ] {
            assert!(
                (shown(real) - want).abs() < 0.005,
                "{real} → {}",
                shown(real)
            );
        }
        // Accelerating: each equal step of real work adds more to the bar than the one before.
        let rises: Vec<f32> = (0..4)
            .map(|i| shown((i + 1) as f32 * 0.25) - shown(i as f32 * 0.25))
            .collect();
        assert!(rises.windows(2).all(|w| w[1] > w[0]), "{rises:?}");
        // Monotonic over the whole range, and never 1.0.
        let mut last = -1.0;
        for i in 0..=100 {
            let v = shown(i as f32 / 100.0);
            assert!(v >= last && v <= 0.97 + f32::EPSILON);
            last = v;
        }
    }

    #[test]
    fn real_fraction_counts_finished_steps_and_caps_the_current_one() {
        // Source 10 + cli 60 done of 165 total, 45 s into the 90 s app step: (70 + 45) / 165.
        let f = real_fraction(&[10, 60], 90, 45, 165);
        assert!((f - 115.0 / 165.0).abs() < 0.001, "{f}");
        // A step that overruns counts as 95 % done, not more: (70 + 85.5) / 165.
        let f = real_fraction(&[10, 60], 90, 500, 165);
        assert!((f - 155.5 / 165.0).abs() < 0.001, "{f}");
        assert_eq!(real_fraction(&[], 10, 0, 165), 0.0);
        assert!(
            real_fraction(&[], 0, 5, 0) <= 1.0,
            "zeros don't divide by zero"
        );
    }

    #[test]
    fn eta_words() {
        // 70 s done, 20 s into the 90 s app step, total 165: 75 s left.
        assert_eq!(eta(true, 70, 90, 20, 165), "less than a minute left");
        assert_eq!(eta(true, 0, 60, 10, 165), "about 3 min left"); // 155 s left, rounded up
        assert_eq!(eta(true, 70, 90, 90, 165), String::new()); // 5 s left: nothing
        assert_eq!(eta(true, 70, 90, 140, 165), "taking longer than usual"); // 140 > 1.5 × 90
        assert_eq!(eta(false, 0, 60, 10, 165), String::new()); // no history, no time
    }

    #[test]
    fn current_step_reads_the_last_marker_of_the_log() {
        let d = tmp("step");
        let log = d.join("update.log");
        assert_eq!(current_step(&log), None, "no file");
        std::fs::write(
            &log,
            "building…\n▸ step 1/4 source\n⏱ source: 0m03s\n▸ step 2/4 cli\ncompiling a\n",
        )
        .unwrap();
        assert_eq!(
            current_step(&log),
            Some(Step {
                n: 2,
                of: 4,
                name: "cli"
            })
        );
        std::fs::write(&log, "▸ step 3/4 app\nlots of output\n").unwrap();
        let s = current_step(&log).unwrap();
        assert_eq!((s.n, s.of, label(s.name)), (3, 4, "Building the Mac app"));
        std::fs::write(&log, "▸ step 1/2 source\n▸ step 2/2 cli\n").unwrap();
        assert_eq!(current_step(&log).unwrap().of, 2);
        std::fs::write(&log, "▸ step 9/4 nonsense\n").unwrap();
        assert_eq!(current_step(&log), None, "an unknown name is not a step");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn history_round_trips_and_garbage_gives_the_defaults() {
        let d = tmp("hist");
        let path = d.join("update-times.txt");
        assert_eq!(
            read_history(&path),
            History {
                secs: DEFAULT_TIMES,
                known: false
            }
        );
        write_history(&path, [12, 41, 70, 3]).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "source=12\ncli=41\napp=70\nswap=3\n"
        );
        assert_eq!(
            read_history(&path),
            History {
                secs: [12, 41, 70, 3],
                known: true
            }
        );
        assert!(
            !d.join("update-times.txt.tmp").exists(),
            "the temp file was renamed over"
        );
        for bad in [
            "",
            "garbage",
            "source=12\ncli=x\napp=70\nswap=3\n",
            "source=12\ncli=41\n",
        ] {
            std::fs::write(&path, bad).unwrap();
            assert_eq!(
                read_history(&path),
                History {
                    secs: DEFAULT_TIMES,
                    known: false
                },
                "{bad:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn timings_come_from_the_four_clock_lines_of_a_whole_run() {
        let log = "▸ step 1/4 source\n⏱ source: 0m12s\n▸ step 2/4 cli\n⏱ cli: 0m41s\n▸ step 3/4 app\n⏱ app build: 1m10s\n▸ step 4/4 swap\n⏱ swap: 0m03s\n⏱ total: 2m06s\n";
        assert_eq!(parse_timings(log), Some([12, 41, 70, 3]));
        assert_eq!(
            parse_timings("⏱ cli: 0m41s\n"),
            None,
            "a partial run teaches nothing"
        );
        assert_eq!(parse_timings("nothing"), None);
    }

    #[test]
    fn the_bar_never_goes_backwards_even_when_a_step_estimate_would_drop_it() {
        let d = tmp("mono");
        let log = d.join("update.log");
        // History says the first two steps are fast and the app step is huge: entering step 3 would drop the real fraction.
        write_history(&times_path(&log), [1, 1, 600, 1]).unwrap();
        let state: Shared = Arc::default();
        *lock(&state) = UpdateState::Running {
            to: "v0.2.8".into(),
            started: Instant::now(),
        };
        assert_eq!(
            progress(&Shared::default(), &log),
            None,
            "not running → no bar"
        );
        let mut last = 0;
        let mut seen = Vec::new();
        for text in [
            "",
            "▸ step 1/4 source\n",
            "▸ step 1/4 source\n▸ step 2/4 cli\n",
            "▸ step 1/4 source\n▸ step 2/4 cli\n▸ step 3/4 app\n",
            "▸ step 4/4 swap\n",
        ] {
            std::fs::write(&log, text).unwrap();
            let p = progress(&state, &log).unwrap();
            assert!(p.pct >= last, "{} after {last} at {text:?}", p.pct);
            assert!(p.pct <= 97);
            last = p.pct;
            seen.push((p.n, p.pct));
        }
        // The furthest shown is kept: forced to 90 %, the next poll (real progress ≈ 0) still says 90.
        std::fs::write(&log, "").unwrap();
        state.track.lock().unwrap().max_shown = 0.9;
        assert_eq!(progress(&state, &log).unwrap().pct, 90);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_whole_run_teaches_the_next_estimate_and_a_failed_one_does_not() {
        let body = "echo '▸ step 1/4 source'\necho '⏱ source: 0m12s'\necho '▸ step 2/4 cli'\necho '⏱ cli: 0m41s'\necho '▸ step 3/4 app'\necho '⏱ app build: 1m10s'\necho '▸ step 4/4 swap'\necho '⏱ swap: 0m03s'\n";
        let (state, log) = run_fake("times", body, Duration::from_secs(30));
        wait_until("done", || settled(&state) && !busy(&state));
        assert_eq!(snapshot(&state), UpdateState::Done("v0.2.7".into()));
        assert_eq!(
            read_history(&times_path(&log)),
            History {
                secs: [12, 41, 70, 3],
                known: true
            }
        );
        let dir = log.parent().unwrap().to_path_buf();
        let _ = std::fs::remove_dir_all(&dir);

        let (state, log) = run_fake(
            "times-fail",
            &format!("{body}exit 3\n"),
            Duration::from_secs(30),
        );
        wait_until("failed", || settled(&state) && !busy(&state));
        assert!(matches!(snapshot(&state), UpdateState::Failed { .. }));
        assert!(!times_path(&log).exists(), "a failed run writes no history");
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }
}
