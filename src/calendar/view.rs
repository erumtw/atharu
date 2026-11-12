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
    /// `Sun 8` in week view, `9` in month view.
    pub head: String,
    pub now: bool,
    /// Month view: leading/trailing days of the neighbouring months.
    pub outside: bool,
    pub logged: i64,
    pub items: Vec<Item>,
}

/// The `#cal` section: the grid plus the last action's message. Every action swaps it whole.
pub struct Cal {
    pub month: bool,
    /// Anchor date of the shown range, carried by every form so it answers for the same range.
    pub week: String,
    /// The tag filter (comma list), carried by every form like `week`.
    pub tag: String,
    pub days: Vec<Day>,
    pub notice: Option<Notice>,
}

pub struct Notice {
    pub text: String,
    pub error: bool,
}

impl Cal {
    pub fn view(&self) -> &'static str {
        if self.month { "month" } else { "week" }
    }
}

#[derive(Template)]
#[template(path = "calendar/page.html")]
pub struct Page {
    pub layout: Layout,
    pub title: String,
    pub prev: String,
    pub next: String,
    pub cal: Cal,
    pub has_ical: bool,
    pub filter: crate::layout::TagFilter,
    /// "+ Event" form, kept after a rejected add (no-JS path).
    pub form: EventForm,
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
