//! Meeting recording (T53, DESIGN.md §6) as the server sees it. The server never records: `AppState.mac`
//! is `Some` only inside Atharu.app, which captures + transcribes on the Mac and posts segments back
//! over HTTP. Shared like `layout`: the meeting banner on Today and on the meeting pages reads it.
mod queries;

use rusqlite::Connection;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

/// The one model (D27): Thai + English verbatim with a glossary prompt; base/small fail on that mix.
pub const MODEL_FILE: &str = "ggml-large-v3-turbo-q5_0.bin";
/// What the transcript meta line calls it.
pub const MODEL_NAME: &str = "large-v3-turbo";
pub const MODEL_SHA256: &str = "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2";
pub const MODEL_BYTES: u64 = 574_041_195;
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";

/// Transcript language setting: Thai by default (D27: `auto` can turn a Thai window into English).
pub const LANGS: [(&str, &str); 3] = [("auto", "Auto"), ("th", "Thai"), ("en", "English")];

/// A panicked capture callback poisons the lock; the levels / state inside are still fine to read.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the Mac app shares with the pages: the speech model on disk, live level meters, permissions.
pub struct Mac {
    /// `<app data>/models`; the model is downloaded here on demand, never bundled.
    pub dir: PathBuf,
    /// Tests point these at a local `file://` and its own hash; the app keeps the pinned ones.
    pub url: String,
    pub sha256: String,
    dl: Mutex<Dl>,
    /// Me (mic), Others (system audio): 0–6 lit bars, written by the capture callbacks.
    pub levels: Mutex<[u8; 2]>,
    /// macOS grants as last checked by the app: (microphone, screen & system audio).
    pub perms: Mutex<(bool, bool)>,
    /// The last Record couldn't start capture (a permission is off): the banner shows the steps.
    pub blocked: Mutex<bool>,
}

enum Dl {
    Idle,
    Running { child: Child, started: Instant },
    Checking,
    Failed,
}

pub enum Model {
    Missing,
    Downloading {
        got_mb: u64,
        total_mb: u64,
        left: String,
    },
    Checking,
    Ready,
    Failed,
}

impl Mac {
    pub fn new(dir: PathBuf) -> Self {
        Mac {
            dir,
            url: MODEL_URL.into(),
            sha256: MODEL_SHA256.into(),
            dl: Mutex::new(Dl::Idle),
            levels: Mutex::default(),
            perms: Mutex::default(),
            blocked: Mutex::default(),
        }
    }

    pub fn model_path(&self) -> PathBuf {
        self.dir.join(MODEL_FILE)
    }

    fn part(&self) -> PathBuf {
        self.dir.join(format!("{MODEL_FILE}.part"))
    }

    pub fn ready(&self) -> bool {
        // Only a checked file is ever renamed to this name, so existing = verified.
        self.model_path().is_file()
    }

    pub fn model(&self) -> Model {
        match &*lock(&self.dl) {
            Dl::Running { started, .. } => {
                let got = std::fs::metadata(self.part()).map_or(0, |m| m.len());
                let secs = started.elapsed().as_secs_f64();
                let left = if got == 0 || secs < 2.0 {
                    String::new()
                } else {
                    let rest = (MODEL_BYTES.saturating_sub(got)) as f64 / (got as f64 / secs);
                    format!(" · about {} left", about(rest))
                };
                Model::Downloading {
                    got_mb: got / 1_000_000,
                    total_mb: MODEL_BYTES / 1_000_000,
                    left,
                }
            }
            Dl::Checking => Model::Checking,
            Dl::Failed => Model::Failed,
            Dl::Idle if self.ready() => Model::Ready,
            Dl::Idle => Model::Missing,
        }
    }

    /// curl + shasum ship with macOS: no HTTP client or hash crate in the app for one 574 MB file.
    pub fn download(self: &Arc<Self>) {
        let mut dl = lock(&self.dl);
        if matches!(*dl, Dl::Running { .. } | Dl::Checking) || self.ready() {
            return;
        }
        if std::fs::create_dir_all(&self.dir).is_err() {
            *dl = Dl::Failed;
            return;
        }
        let child = Command::new("curl")
            .args(["-sSfL", "--retry", "2", "-o"])
            .arg(self.part())
            .arg(&self.url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        *dl = match child {
            Ok(child) => Dl::Running {
                child,
                started: Instant::now(),
            },
            Err(_) => Dl::Failed,
        };
        let me = Arc::clone(self);
        std::thread::spawn(move || me.finish_download());
    }

    fn finish_download(&self) {
        let ok = loop {
            std::thread::sleep(Duration::from_millis(300));
            let mut dl = lock(&self.dl);
            let Dl::Running { child, .. } = &mut *dl else {
                return; // cancelled: cancel() already removed the partial
            };
            match child.try_wait() {
                Ok(None) => continue,
                Ok(Some(st)) => {
                    let ok = st.success();
                    *dl = Dl::Checking;
                    break ok;
                }
                Err(_) => {
                    *dl = Dl::Checking;
                    break false;
                }
            }
        };
        let good = ok && sha256(&self.part()).as_deref() == Some(self.sha256.as_str());
        if good && std::fs::rename(self.part(), self.model_path()).is_ok() {
            *lock(&self.dl) = Dl::Idle;
        } else {
            // A wrong or half file is never kept (Settings.Transcription "Nothing was kept").
            let _ = std::fs::remove_file(self.part());
            *lock(&self.dl) = Dl::Failed;
        }
    }

    pub fn cancel(&self) {
        let mut dl = lock(&self.dl);
        if let Dl::Running { child, .. } = &mut *dl {
            let _ = child.kill();
            let _ = child.wait();
        }
        if !matches!(*dl, Dl::Checking) {
            *dl = Dl::Idle;
            let _ = std::fs::remove_file(self.part());
        }
    }

    pub fn delete(&self) {
        self.cancel();
        let _ = std::fs::remove_file(self.model_path());
    }
}

fn sha256(path: &std::path::Path) -> Option<String> {
    let out = Command::new("shasum")
        .args(["-a", "256", "-b"])
        .arg(path)
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    s.split_whitespace().next().map(str::to_string)
}

/// `about 3 min` / `about 40 s`: estimates, so whole units.
pub fn about(secs: f64) -> String {
    if secs < 60.0 {
        format!("{} s", (secs.max(1.0) / 10.0).ceil() as u64 * 10)
    } else {
        format!("{} min", (secs / 60.0).ceil() as u64)
    }
}

/// `00:01:02`: transcript timestamps and the interrupted callout.
pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// `02:58` under an hour, `1:02:58` after: the banner's elapsed time.
pub fn elapsed(secs: i64) -> String {
    let s = secs.max(0);
    if s < 3600 {
        format!("{:02}:{:02}", s / 60, s % 60)
    } else {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    }
}

/// `1 h 02 min` / `48 min`: how long a recording ran.
pub fn span(secs: i64) -> String {
    let m = (secs.max(0) + 30) / 60;
    if m < 60 {
        format!("{m} min")
    } else {
        format!("{} h {:02} min", m / 60, m % 60)
    }
}

/// Seconds between two local `YYYY-MM-DD HH:MM:SS` stamps (as `meetings` stores them).
pub fn secs_between(from: &str, to: &str) -> i64 {
    let p = |s: &str| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok();
    match (p(from), p(to)) {
        (Some(a), Some(b)) => (b - a).num_seconds(),
        _ => 0,
    }
}

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// The meeting banner's recording part; Today and the meetings pages both carry one.
#[derive(Default)]
pub struct Rec {
    /// Inside Atharu.app: the Record switch and Stop show; the browser only watches.
    pub mac: bool,
    pub model_ready: bool,
    /// The running meeting's recording, if one is live.
    pub live: Option<Live>,
    /// Stopped, the tail is still being transcribed.
    pub finishing: bool,
    /// Capture couldn't start: "not recording yet" + the permission steps.
    pub blocked: bool,
    /// Microphone already allowed (step 1 ticked).
    pub mic_ok: bool,
}

pub struct Live {
    pub elapsed: String,
    /// Six bars per meter: lit or not (`.lvl i.on`).
    pub me: [bool; 6],
    pub others: [bool; 6],
    pub me_n: u8,
    pub others_n: u8,
}

impl Rec {
    pub fn load(conn: &Connection, mac: Option<&Mac>) -> rusqlite::Result<Rec> {
        let mut rec = Rec {
            mac: mac.is_some(),
            model_ready: mac.is_some_and(Mac::ready),
            ..Rec::default()
        };
        let Some((status, started)) = queries::running(conn)? else {
            return Ok(rec);
        };
        if status == "finishing" {
            rec.finishing = true;
            return Ok(rec);
        }
        let [me, others] = mac.map_or([0, 0], |m| *lock(&m.levels));
        let bars = |n: u8| std::array::from_fn(|i| (i as u8) < n);
        rec.live = Some(Live {
            elapsed: elapsed(secs_between(&started, &now())),
            me: bars(me),
            others: bars(others),
            me_n: me,
            others_n: others,
        });
        if let Some(m) = mac {
            rec.blocked = *lock(&m.blocked);
            rec.mic_ok = lock(&m.perms).0;
        }
        Ok(rec)
    }
}

/// The transcript language setting.
pub fn lang(conn: &Connection) -> rusqlite::Result<String> {
    Ok(queries::setting(conn, "transcribe_lang")?
        .filter(|l| LANGS.iter().any(|(v, _)| v == l))
        .unwrap_or_else(|| "th".into()))
}

/// What the Mac app's recorder should be doing: the newest recording still live or finishing.
pub struct Want {
    pub tid: i64,
    pub live: bool,
    pub lang: String,
    /// Glossary `initial_prompt` (D27): without it turbo drops short English lines and writes
    /// tech terms in Thai script.
    pub prompt: String,
}

pub fn wanted(conn: &Connection) -> rusqlite::Result<Option<Want>> {
    let Some((tid, status, lang, meeting)) = queries::want(conn)? else {
        return Ok(None);
    };
    let epics = queries::active_epics(conn)?;
    Ok(Some(Want {
        tid,
        live: status == "live",
        lang,
        prompt: glossary(&meeting, &epics),
    }))
}

/// `Glossary: oms-sync, OPS-101 Checkout latency, ….` On mixed.wav (T53) this form kept every English line
/// and wrote Thai in Thai; a bare list made whisper romanise Thai and echo the keys. Whisper still echoes
/// the "Glossary:" word now and then: the recorder strips it. Only the last 224 tokens count, so cap it.
fn glossary(meeting: &str, epics: &[(String, String)]) -> String {
    let mut p = format!("Glossary: {meeting}");
    for (key, title) in epics {
        let item = format!(", {key} {title}");
        if p.chars().count() + item.chars().count() > 600 {
            break;
        }
        p.push_str(item.trim_end());
    }
    p.push('.');
    p
}

/// At app launch: a recording left live or finishing has no process behind it any more (crash or
/// quit). What was transcribed stays as `interrupted`; one with no lines at all is dropped.
pub fn interrupt_orphans(conn: &Connection) -> rusqlite::Result<()> {
    queries::interrupt_orphans(conn, &now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(clock(62.4), "00:01:02");
        assert_eq!(clock(3725.0), "01:02:05");
        assert_eq!(elapsed(178), "02:58");
        assert_eq!(elapsed(3778), "1:02:58");
        assert_eq!(span(3720), "1 h 02 min");
        assert_eq!(span(2880), "48 min");
        assert_eq!(about(25.0), "30 s");
        assert_eq!(about(150.0), "3 min");
        assert_eq!(
            secs_between("2026-11-12 10:00:00", "2026-11-12 11:02:00"),
            3720
        );
    }

    #[test]
    fn glossary_names_meeting_and_epics() {
        let e = vec![
            ("OPS-101".to_string(), "Checkout latency".to_string()),
            ("OPS-7".to_string(), String::new()),
        ];
        assert_eq!(
            glossary("oms-sync", &e),
            "Glossary: oms-sync, OPS-101 Checkout latency, OPS-7."
        );
    }
}
