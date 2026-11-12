use crate::layout::Layout;
use askama::Template;

pub struct Item {
    /// `Impact` / `Decision` / `Blocker`.
    pub label: &'static str,
    pub text: String,
    /// Grouped by tag: the line's epic key, shown in brackets like the meeting.
    pub epic: Option<String>,
    pub meeting: Option<String>,
    /// Agent that filed the line: shown `(via claude)` so the copy stays honest.
    pub via: Option<String>,
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
    /// `vector-search: depth 2 → 3 (can build without AI)`.
    pub learning: Vec<String>,
}

#[derive(Template)]
#[template(path = "report/note.md")]
pub struct NoteMd<'a> {
    pub note: &'a Note,
}

pub struct Seg {
    pub flex: i64,
    /// `.obar` class: active / done / other.
    pub class: &'static str,
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
    pub decisions_in_meetings: usize,
    /// Plans logged in range: hit rate % (`None` = no plans) and the counts behind it.
    pub hit_rate: Option<i64>,
    pub plans_done: i64,
    pub plans_open: i64,
    pub plans_dropped: i64,
    pub meetings: i64,
    pub note: Note,
    pub markdown: String,
    /// Empty-range nudge: (preset, label) of the next wider range; none past a quarter.
    pub wider: Option<(&'static str, &'static str)>,
    pub okrs: Vec<OkrBar>,
    /// `epic` · `tag`: what the note's sections are.
    pub group: &'static str,
    pub filter: crate::layout::TagFilter,
    pub weeks: WeekBars,
    /// `Sep`, one per month touching the range.
    pub heat_months: Vec<String>,
    pub heat: Vec<HeatRow>,
}

pub struct HeatRow {
    pub topic: String,
    /// Depth as of each month end; 0 = not rated yet (blank cell).
    pub cells: Vec<u8>,
}

/// Cell class for a depth (colours live in tokens.css: --heat-1, --heat-2, --accent, --heat-4).
pub fn heat_class(depth: &u8) -> &'static str {
    match depth {
        1 => "heat1",
        2 => "heat2",
        3 => "heat3",
        _ => "heat4",
    }
}

impl Page {
    pub fn is_empty(&self) -> bool {
        self.note.shipped.is_empty()
            && self.note.sections.is_empty()
            && self.note.learning.is_empty()
    }
}
