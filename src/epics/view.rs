use crate::{
    domain::{entry::LineRow, plan::Plan},
    layout::Layout,
};
use askama::Template;
use chrono::NaiveDate;

/// `14 Nov`, the board's short date.
pub fn short(d: NaiveDate) -> String {
    d.format("%-d %b").to_string()
}

pub struct ListRow {
    pub key: String,
    pub title: String,
    pub role: String,
    /// `14 Nov`, empty when no target.
    pub target: String,
    /// `· 3d`, `· today`, or `· 6d late` (then `late` is set and the board bolds it).
    pub target_note: String,
    pub late: bool,
    /// `today`, `9d ago`, or `—`.
    pub last_log: String,
    pub flag: Option<Flag>,
}

/// A flag as the epic pages show it.
pub struct Flag {
    pub class: &'static str,
    pub label: &'static str,
    pub reason: String,
}

pub struct OkrGroup {
    /// The OKR text, or "No OKR".
    pub okr: String,
    pub rows: Vec<ListRow>,
}

pub struct Closed {
    pub key: String,
    pub title: String,
    pub done: bool,
    /// `done 20 Oct` or `dropped`.
    pub chip: String,
    pub jira: Option<String>,
}

#[derive(Template)]
#[template(path = "epics/list.html")]
pub struct List {
    pub layout: Layout,
    /// (key, Jira `summary · status` if cached).
    pub untitled: Vec<(String, Option<String>)>,
    /// Jira configured: show the Refresh button.
    pub jira_on: bool,
    pub jira_failed: bool,
    pub groups: Vec<OkrGroup>,
    pub closed: Vec<Closed>,
}

/// htmx answer to the inline title box: the row, now named.
#[derive(Template)]
#[template(path = "epics/named.html")]
pub struct Named {
    pub key: String,
    pub title: String,
}

/// Property values as the inputs want them (`""` = empty date).
pub struct EpicView {
    pub key: String,
    pub title: String,
    pub role: String,
    pub okr: String,
    pub status: String,
    pub start: String,
    pub target: String,
    pub done_on: String,
    /// Jira `summary · status`, shown while `title` is empty.
    pub jira: Option<String>,
}

pub struct Evidence {
    pub id: i64,
    pub url: String,
    pub note: String,
    pub author: String,
}

/// Impact line + evidence: the part of the epic page that changes when a property or evidence does.
pub struct Live {
    pub key: String,
    pub metric: String,
    pub before: String,
    pub after: String,
    pub impact: Option<String>,
    pub evidence: Vec<Evidence>,
    /// Kept in the evidence box after a rejected add.
    pub ev_line: String,
    pub ev_error: Option<String>,
}

/// A timeline row: to-dos draw as `plan_row.html` (check, Pin, Drop), every other line as `entry_row.html`.
pub enum Item {
    Line(LineRow),
    Plan(Plan),
}

pub struct DayGroup {
    pub label: String,
    pub rows: Vec<Item>,
}

#[derive(Template)]
#[template(path = "epics/page.html")]
pub struct Page {
    pub layout: Layout,
    pub field: crate::layout::TagField,
    pub epic: EpicView,
    pub flag: Option<Flag>,
    /// Entries per week, last 8 weeks, as % of the busiest.
    pub activity: Vec<i64>,
    pub live: Live,
    pub timeline: Vec<DayGroup>,
    /// For the to-do rows' due / late labels.
    pub today: NaiveDate,
    /// Top-bar status after a plain (no-JS) save failed: the reason.
    pub save_error: Option<&'static str>,
    /// Notes linking here with `[[…]]`: (id, title).
    pub refs: Vec<crate::layout::Ref>,
    /// The `[[…]]` name the empty Linked references line suggests.
    pub ref_name: String,
    /// A plain (no-JS) delete was refused.
    pub delete_error: Option<&'static str>,
}

/// htmx answer to a property / evidence change: the live block, plus the top-bar status out-of-band.
#[derive(Template)]
#[template(path = "epics/swap.html")]
pub struct Swap {
    pub live: Live,
    pub save_error: Option<&'static str>,
    /// The Tags prop, swapped out-of-band: a chip edit changes it.
    pub field: Option<crate::layout::TagField>,
    /// The "done on" input, out-of-band, when this save filled the date in.
    pub done_on: Option<String>,
}
