use crate::{
    domain::{dates, entry::Kind, plan::Plan},
    layout::Layout,
};
use askama::Template;
use chrono::NaiveDate;

pub struct Row {
    pub id: i64,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    pub meeting: Option<String>,
    pub author: String,
}

pub struct Group {
    pub key: Option<String>,
    pub title: String,
    pub rows: Vec<Row>,
}

/// The page's day plus its neighbours, all pre-formatted for the day nav.
pub struct Day {
    pub iso: String,
    pub label: String,
    pub prev: String,
    pub prev_label: String,
    pub next: String,
    pub next_label: String,
    pub is_today: bool,
}

impl Day {
    pub fn new(d: NaiveDate, today: NaiveDate) -> Day {
        let prev = d.pred_opt().unwrap_or(d);
        let next = d.succ_opt().unwrap_or(d);
        Day {
            iso: dates::iso(d),
            label: dates::label(d),
            prev: dates::iso(prev),
            prev_label: prev.format("%a %-d").to_string(),
            next: dates::iso(next),
            next_label: next.format("%a %-d").to_string(),
            is_today: d == today,
        }
    }
}

pub struct Saved {
    pub id: i64,
    pub kind: &'static str,
    pub key: Option<String>,
    pub new_epic: bool,
}

pub struct Deleted {
    /// The whole row as JSON, posted back by Undo.
    pub snap: String,
}

pub struct Preview {
    pub kind: &'static str,
    pub key: Option<String>,
    pub text: String,
    pub due: Option<String>,
}

/// "This week" (pinned) and "Other open plans"; always about the real today, not the backfill day.
pub struct Plans {
    pub pinned: Vec<Plan>,
    pub other: Vec<Plan>,
}

pub struct ActiveMeeting {
    pub id: i64,
    pub name: String,
}

/// Fields `partials/meeting_banner.html` reads; start/end themselves live in the meetings slice.
pub struct Banner {
    pub active: Option<ActiveMeeting>,
    pub series: Vec<String>,
    pub name: String,
    pub error: Option<String>,
    pub back: String,
}

/// One "Needs you" line: a flagged epic or an overdue to-do.
pub struct Need {
    /// `flag over` / `flag risk` / `flag stale`.
    pub class: &'static str,
    pub label: &'static str,
    pub key: Option<String>,
    /// `Log retention · 6 days past target`.
    pub text: String,
    pub href: String,
}

#[derive(Template)]
#[template(path = "today/page.html")]
pub struct Page {
    pub layout: Layout,
    pub banner: Banner,
    pub today: NaiveDate,
    pub plans: Plans,
    pub needs: Vec<Need>,
    pub day: Day,
    pub groups: Vec<Group>,
    pub keys: Vec<String>,
    /// Kept in the box after a parse error.
    pub line: String,
    pub error: Option<String>,
    pub preview: Option<Preview>,
    pub saved: Option<Saved>,
    pub deleted: Option<Deleted>,
    /// The log line open in its edit box (`/?edit=ID` without JS).
    pub edit: Option<Edit>,
}

/// Inline edit box of one log line.
#[derive(Template)]
#[template(path = "partials/entry_edit.html")]
pub struct Edit {
    pub id: i64,
    pub day: String,
    pub line: String,
    pub error: Option<String>,
}

/// htmx answer to save/delete: the log, plus the toast swapped out-of-band.
#[derive(Template)]
#[template(path = "today/swap.html")]
pub struct Swap {
    pub today: NaiveDate,
    /// Swapped out-of-band: a `>` line or its undo changes the open plans too.
    pub plans: Plans,
    pub day: Day,
    pub groups: Vec<Group>,
    pub saved: Option<Saved>,
    pub deleted: Option<Deleted>,
    pub edit: Option<Edit>,
}

#[derive(Template)]
#[template(path = "partials/parse_hint.html")]
pub struct Hint {
    pub error: Option<String>,
    pub preview: Option<Preview>,
}
