use crate::layout::Layout;
use askama::Template;

pub struct Item {
    /// `Impact` / `Decision` / `Blocker`.
    pub label: &'static str,
    pub text: String,
}

pub struct Section {
    /// `OPS-101 · Checkout latency`, `OPS-140`, or `Not filed under an epic`.
    pub heading: String,
    pub items: Vec<Item>,
}

/// The copyable part of the report: shown as HTML and rendered again as markdown.
pub struct Note {
    pub title: String,
    pub shipped: Vec<String>,
    pub sections: Vec<Section>,
}

#[derive(Template)]
#[template(path = "report/note.md")]
pub struct NoteMd<'a> {
    pub note: &'a Note,
}

pub struct Seg {
    pub flex: i64,
    pub color: &'static str,
}

pub struct OkrBar {
    pub okr: String,
    /// `2 active, 1 done`.
    pub summary: String,
    pub segs: Vec<Seg>,
}

pub struct WeekBars {
    /// Bar heights in % of the busiest week.
    pub pct: Vec<i64>,
    /// `13 Oct · 1`, first and last week.
    pub first: String,
    pub last: String,
}

#[derive(Template)]
#[template(path = "report/page.html")]
pub struct Page {
    pub layout: Layout,
    /// `week` / `month` / `quarter`, or `""` for a hand-picked range.
    pub preset: &'static str,
    pub from: String,
    pub to: String,
    pub epics_done: usize,
    pub impact_lines: usize,
    pub decisions: usize,
    pub note: Note,
    pub markdown: String,
    /// Empty-range nudge: (preset, label) of the next wider range; none past a quarter.
    pub wider: Option<(&'static str, &'static str)>,
    pub okrs: Vec<OkrBar>,
    pub weeks: WeekBars,
}

impl Page {
    pub fn is_empty(&self) -> bool {
        self.note.shipped.is_empty() && self.note.sections.is_empty()
    }
}
