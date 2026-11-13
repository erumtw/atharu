use crate::layout::Layout;
use askama::Template;

/// One chip in a day cell.
pub struct Item {
    /// `meet` · `todo` · `tgt` · `own` (the board's `.ev` classes).
    pub class: &'static str,
    pub time: Option<String>,
    pub text: String,
    /// Under the text, e.g. `ended 10:42`.
    pub sub: Option<String>,
    /// Link items (to-do, target, logged meeting). `None` = an event: click starts meeting mode.
    pub href: Option<String>,
    /// Open to-do past its due day.
    pub late: bool,
    /// Own event id, for its delete button.
    pub own_id: Option<i64>,
}

pub struct Day {
    pub iso: String,
    /// `Sun` · `8`: the header shows both, the month cell only the number.
    pub wd: String,
    pub num: String,
    pub now: bool,
    /// Month view: leading/trailing days of the neighbouring months.
    pub outside: bool,
    pub logged: i64,
    /// Due dates, epic targets, date-only events, meetings with no start time.
    pub all_day: Vec<Item>,
    /// In start order.
    pub timed: Vec<Timed>,
}

impl Day {
    /// Month cell: timed first (by time), then the all-day ones.
    pub fn items(&self) -> Vec<&Item> {
        self.timed
            .iter()
            .map(|t| &t.item)
            .chain(&self.all_day)
            .collect()
    }
}

impl Timed {
    pub fn i(&self) -> &Item {
        &self.item
    }
}

/// An item on the hour grid; minutes count from the grid's first hour.
pub struct Timed {
    pub item: Item,
    pub top: u32,
    pub len: u32,
    /// Overlap layout (`domain::timegrid::columns`): column and column count of its cluster.
    pub col: usize,
    pub cols: usize,
}

/// The `#cal` section: the grid plus the last action's message. Every action swaps it whole.
pub struct Cal {
    /// `day` · `days` (N days, picked by width) · `week` · `month`.
    pub view: &'static str,
    pub n: u32,
    /// Where the range is pointed, carried by every form so it answers for the same range.
    pub d: String,
    /// The tag filter (comma list), carried by every form like `d`.
    pub tag: String,
    /// The hour grid's first and last hour: 07–21, widened to fit the range's items.
    pub h0: u32,
    pub h1: u32,
    /// Minutes from `h0` to now, when today is in the range.
    pub now_min: Option<u32>,
    pub days: Vec<Day>,
    pub notice: Option<Notice>,
}

pub struct Notice {
    pub text: String,
    pub error: bool,
}

impl Cal {
    pub fn month(&self) -> bool {
        self.view == "month"
    }

    pub fn hours(&self) -> std::ops::Range<u32> {
        self.h0..self.h1
    }

    /// How many of `n` items a cell shows: all of them in a one-day view or when ≤ 3, else 2 + "+N more".
    pub fn cap(&self, n: usize) -> usize {
        if self.days.len() == 1 || n <= 3 { n } else { 2 }
    }

    /// A day opens as the Day view (same tag filter).
    pub fn day_href(&self, d: &Day) -> String {
        let tag = if self.tag.is_empty() {
            String::new()
        } else {
            format!("&tag={}", self.tag)
        };
        format!("/calendar?d={}&view=day{tag}", d.iso)
    }
}

#[derive(Template)]
#[template(path = "calendar/page.html")]
pub struct Page {
    pub layout: Layout,
    pub title: String,
    pub prev: String,
    pub next: String,
    /// `week`, `month`, `day` or `5 days`: the ‹ › labels.
    pub unit: String,
    /// The current view around today.
    pub today: String,
    pub views: Vec<ViewLink>,
    pub cal: Cal,
    pub has_ical: bool,
    pub filter: crate::layout::TagFilter,
    /// "+ Event" form, kept after a rejected add (no-JS path).
    pub form: EventForm,
}

/// One button of the Day · N days · Week · Month switch.
pub struct ViewLink {
    pub label: &'static str,
    pub href: String,
    pub on: bool,
}

#[derive(Default)]
pub struct EventForm {
    pub title: String,
    pub date: String,
    pub time: String,
}

/// htmx answer to add / delete / refresh.
#[derive(Template)]
#[template(path = "calendar/grid.html")]
pub struct Swap {
    pub cal: Cal,
}
