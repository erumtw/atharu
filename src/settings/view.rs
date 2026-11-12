use crate::recording::Model;
use crate::{domain::look::Look, layout::Layout};
use askama::Template;

#[derive(Template)]
#[template(path = "settings/page.html")]
pub struct Page {
    pub layout: Layout,
    pub looks: &'static [Look],
    /// The picked look: its AA floor is where the slider stops (and the hatched part ends).
    pub cur: &'static Look,
    /// Connections rows: on/off only, from data/config.toml; never the URL or token.
    pub google: bool,
    pub jira: bool,
    /// About rows: the release tag, or `0.1.0 · dev build`.
    pub version: String,
    pub sha: &'static str,
    pub built: &'static str,
    pub db: Db,
    pub tr: Transcription,
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
