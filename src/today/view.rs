use crate::{
    domain::{dates, entry::Kind},
    layout::Layout,
};
use askama::Template;
use chrono::NaiveDate;

pub struct Row {
    pub id: i64,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
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
    pub line: String,
    pub day: String,
}

pub struct Preview {
    pub kind: &'static str,
    pub key: Option<String>,
    pub text: String,
    pub due: Option<String>,
}

#[derive(Template)]
#[template(path = "today/page.html")]
pub struct Page {
    pub layout: Layout,
    pub day: Day,
    pub groups: Vec<Group>,
    pub keys: Vec<String>,
    /// Kept in the box after a parse error.
    pub line: String,
    pub error: Option<String>,
    pub preview: Option<Preview>,
    pub saved: Option<Saved>,
    pub deleted: Option<Deleted>,
}

/// htmx answer to save/delete: the log, plus the toast swapped out-of-band.
#[derive(Template)]
#[template(path = "today/swap.html")]
pub struct Swap {
    pub day: Day,
    pub groups: Vec<Group>,
    pub saved: Option<Saved>,
    pub deleted: Option<Deleted>,
}

#[derive(Template)]
#[template(path = "partials/parse_hint.html")]
pub struct Hint {
    pub error: Option<String>,
    pub preview: Option<Preview>,
}
