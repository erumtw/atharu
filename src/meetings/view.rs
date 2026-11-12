use crate::{
    domain::{entry::Kind, plan::Plan},
    layout::Layout,
};
use askama::Template;
use chrono::NaiveDate;

pub struct Active {
    pub id: i64,
    pub name: String,
}

/// Start box or running banner (`partials/meeting_banner.html`); Today renders the same partial.
pub struct Banner {
    pub active: Option<Active>,
    pub series: Vec<String>,
    /// Kept in the box after a rejected start.
    pub name: String,
    pub error: Option<String>,
    /// Page a plain POST returns to.
    pub back: String,
}

/// htmx answer to start / end.
#[derive(Template)]
#[template(path = "meetings/banner.html")]
pub struct BannerSwap {
    pub banner: Banner,
}

pub struct SideSeries {
    pub name: String,
    pub count: i64,
}

pub struct ListRow {
    pub id: i64,
    pub day: String,
    pub decisions: i64,
    pub learned: i64,
    /// `1 open · 1 done`, or `—`.
    pub actions: String,
    /// First line of the notes, cut short, or `—`.
    pub notes: String,
    pub running: bool,
}

pub struct SeriesGroup {
    pub name: String,
    pub rows: Vec<ListRow>,
}

#[derive(Template)]
#[template(path = "meetings/list.html")]
pub struct List {
    pub layout: Layout,
    pub side: Vec<SideSeries>,
    pub banner: Banner,
    pub groups: Vec<SeriesGroup>,
    /// Set when `?name=` narrows the list to one series.
    pub filter: Option<String>,
}

pub struct Line {
    pub kind: Kind,
    pub epic_key: Option<String>,
    pub topic: Option<String>,
    pub text: String,
}

pub struct KindGroup {
    pub title: &'static str,
    pub lines: Vec<Line>,
}

#[derive(Template)]
#[template(path = "meetings/page.html")]
pub struct Page {
    pub layout: Layout,
    pub side: Vec<SideSeries>,
    pub id: i64,
    pub name: String,
    /// `Wed 11 Nov 2026`.
    pub day: String,
    /// `None` = still running; else `HH:MM` it ended.
    pub ended: Option<String>,
    /// Up to two earlier/later meetings of the series: (id, label).
    pub siblings: Vec<(i64, String)>,
    pub series_count: usize,
    pub plans: Vec<Plan>,
    pub groups: Vec<KindGroup>,
    pub notes: String,
    pub today: NaiveDate,
    pub back: String,
    /// Notes linking here with `[[…]]`: (id, title).
    pub mentions: Vec<(i64, String)>,
}

#[derive(Template)]
#[template(path = "meetings/saved.html")]
pub struct Saved;
