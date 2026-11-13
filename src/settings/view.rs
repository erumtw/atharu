use crate::recording::Model;
use crate::{domain::look::Look, layout::Layout};
use askama::Template;

#[derive(Template)]
#[template(path = "settings/page.html")]
pub struct Page {
    pub layout: Layout,
    pub looks: &'static [Look],
    /// The picked look: its AA floor is where the hatched part ends (a baseline; the slider goes below it).
    pub cur: &'static Look,
    /// Connections rows: on/off only, from data/config.toml; never the URL or token.
    pub google: bool,
    pub jira: bool,
    /// D9: Jira refused the token on its last call.
    pub jira_dead: bool,
    pub token_url: &'static str,
    /// About rows: the release tag, or `0.1.0 · dev build`.
    pub version: String,
    pub sha: &'static str,
    pub built: &'static str,
    pub db: Db,
    pub tr: Transcription,
    pub mc: MacCal,
    /// About › Update, the Mac app's button (0.2.6).
    pub upd: Update,
    /// The stored launch animation (`domain::look::LAUNCH`).
    pub launch: &'static str,
}

/// `/launch`: the standalone page the Mac app opens first; no sidebar, so it needs only the look and glass fields.
#[derive(Template)]
#[template(path = "settings/launch.html")]
pub struct Launch {
    pub layout: Layout,
    /// `ink` or `glass` (`off` never renders; it redirects).
    pub variant: &'static str,
}

/// Settings › Transcription; polled while the model downloads.
#[derive(Template)]
#[template(path = "settings/transcription.html")]
pub struct Transcription {
    /// Atharu.app; a browser gets the one-line version.
    pub mac: bool,
    pub model: Model,
    pub polling: bool,
    pub langs: &'static [(&'static str, &'static str)],
    pub lang: String,
    /// `OPS-101 Checkout latency`: the first active epic, as an example of what is passed.
    pub hint: String,
    pub perms: bool,
    /// The Names list as typed (`domain::people`).
    pub people: String,
}

/// Settings › About › Database file (T85); also the htmx partial `/settings/db` returns.
#[derive(Template)]
#[template(path = "settings/db.html")]
pub struct Db {
    /// The file this server has open (`In memory` in tests).
    pub path: String,
    /// Mac app only: the pointer file it reads at start, so the path can be changed here.
    pub change: bool,
    /// The pointer names another file than the open one: the switch waits for a restart.
    pub pending: bool,
    /// The one-time "old Mac DB isn't merged" note is still undismissed.
    pub note: bool,
    pub input: String,
    pub error: Option<String>,
}

/// Settings › Connections › Mac calendars (0.2.4). In the Mac app: a switch + one checkbox per calendar;
/// in a browser: status only (only the app can read macOS Calendar).
pub struct MacCal {
    pub app: bool,
    pub on: bool,
    pub access: crate::recording::Access,
    /// Calendar.app's calendars, each with whether it is synced.
    pub cals: Vec<(crate::recording::MacCalendar, bool)>,
    /// Last sync, local `YYYY-MM-DD HH:MM`.
    pub synced: Option<String>,
}

impl MacCal {
    pub fn denied(&self) -> bool {
        self.access == crate::recording::Access::Denied
    }

    pub fn granted(&self) -> bool {
        self.access == crate::recording::Access::Granted
    }

    /// On in the app but not settled yet (macOS hasn't answered, or the first sync hasn't run): the row polls.
    pub fn waiting(&self) -> bool {
        self.app && self.on && !self.denied() && (!self.granted() || self.synced.is_none())
    }
}

/// htmx answer to the Mac calendars form: the row alone.
#[derive(Template)]
#[template(path = "settings/maccal.html")]
pub struct MacCalRow {
    pub mc: MacCal,
}

/// Where the Update button is (`updater::UpdateState`, flattened for the template).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Idle,
    UpToDate,
    Available,
    Running,
    Done,
    Failed,
}

/// About › Update, the Mac app's part: check → Update → progress → Restart. Polls itself only while it runs.
#[derive(Template)]
#[template(path = "settings/update.html")]
pub struct Update {
    pub phase: Phase,
    /// The tag it is about: the newest, the one installing, the one installed.
    pub tag: String,
    /// `45 s` / `2 min` while it runs.
    pub elapsed: String,
    /// The last line `install.sh` printed.
    pub last: String,
    pub why: String,
    /// The failure was the check itself, not an install.
    pub check_failed: bool,
    /// Where the full output of the last run is.
    pub log: String,
}

impl Update {
    pub fn new(s: &crate::updater::UpdateState, log: String) -> Update {
        use crate::updater::UpdateState as S;
        let mut u = Update {
            phase: Phase::Idle,
            tag: String::new(),
            elapsed: String::new(),
            last: String::new(),
            why: String::new(),
            check_failed: false,
            log,
        };
        match s {
            S::Idle => {}
            S::UpToDate(t) => (u.phase, u.tag) = (Phase::UpToDate, t.clone()),
            S::Available(t) => (u.phase, u.tag) = (Phase::Available, t.clone()),
            S::Running { to, started, last } => {
                let secs = started.elapsed().as_secs();
                u.phase = Phase::Running;
                u.tag = to.clone();
                u.last = last.clone();
                u.elapsed = if secs < 60 {
                    format!("{secs} s")
                } else {
                    format!("{} min", secs / 60)
                };
            }
            S::Done(t) => (u.phase, u.tag) = (Phase::Done, t.clone()),
            S::Failed { to, why } => {
                u.phase = Phase::Failed;
                u.check_failed = to.is_empty();
                u.tag = to.clone();
                u.why = why.clone();
            }
        }
        u
    }
}
