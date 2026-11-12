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
}
