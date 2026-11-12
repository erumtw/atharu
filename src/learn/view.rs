use crate::layout::Layout;
use askama::Template;

/// Meter + label for one depth; `depth = 0` = not rated.
pub struct Depth {
    pub depth: u8,
    pub label: &'static str,
}

pub struct NewTopic {
    pub slug: String,
    pub lines: i64,
}

pub struct ListRow {
    pub slug: String,
    pub depth: Depth,
    /// `2 Oct`, empty if never rated.
    pub last_rated: String,
    /// `40 days ago · re-rate`, `not rated yet`, or empty when fresh.
    pub stale: String,
    pub lines: i64,
}

#[derive(Template)]
#[template(path = "learn/list.html")]
pub struct List {
    pub layout: Layout,
    pub side: Vec<String>,
    pub new_topics: Vec<NewTopic>,
    /// Days of learn lines without a topic, newest first (`YYYY-MM-DD`).
    pub untopic: Vec<String>,
    pub rows: Vec<ListRow>,
}

impl List {
    pub fn inbox_count(&self) -> usize {
        self.new_topics.len() + self.untopic.len()
    }
}

pub struct Hist {
    /// `12 Sep`.
    pub day: String,
    pub depth: Depth,
}

/// Depth now + history; swapped out-of-band when a rating changes.
pub struct DepthNow {
    pub now: Depth,
    /// `rated 8 Nov`, empty if never rated.
    pub rated: String,
    pub history: Vec<Hist>,
}

/// Rate buttons + "Rated 3 today · Undo".
pub struct Rate {
    pub slug: String,
    /// Today's rating, 0 = none yet.
    pub today: u8,
    /// All four depths, for the buttons.
    pub scale: Vec<Depth>,
    pub depth_now: DepthNow,
}

#[derive(Template)]
#[template(path = "learn/rate.html")]
pub struct RateSwap {
    pub rate: Rate,
}

#[derive(Template)]
#[template(path = "learn/page.html")]
pub struct Page {
    pub field: crate::layout::TagField,
    pub layout: Layout,
    pub side: Vec<String>,
    pub slug: String,
    pub summary: String,
    pub own_words: String,
    pub resources: String,
    pub rate: Rate,
    pub lines: Vec<crate::domain::entry::LineRow>,
    /// Notes linking here with `[[…]]`: (id, title).
    pub mentions: Vec<(i64, String)>,
}

#[derive(Template)]
#[template(path = "learn/saved.html")]
pub struct Saved;

/// The Tags prop, its own form: htmx swaps it after each chip edit.
#[derive(Template)]
#[template(path = "learn/tags.html")]
pub struct Tags {
    pub slug: String,
    pub field: crate::layout::TagField,
}
