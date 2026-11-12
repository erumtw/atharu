use super::queries::{HubEpic, HubTopic, TagRow};
use crate::{
    domain::entry::{Kind, NoteRef},
    layout::Layout,
};
use askama::Template;

#[derive(Template)]
#[template(path = "tags/list.html")]
pub struct List {
    pub layout: Layout,
    pub rows: Vec<TagRow>,
    /// All tags in use, before the find box narrows `rows`.
    pub total: usize,
    pub q: String,
    /// `used` · `recent` · `name`.
    pub sort: String,
    pub today: chrono::NaiveDate,
}

impl List {
    /// `today` or `Wed 11 Nov`.
    pub fn last(&self, r: &TagRow) -> String {
        let d = r
            .last_used
            .as_deref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        match d {
            Some(d) if d == self.today => "today".into(),
            Some(d) => crate::domain::dates::label(d),
            None => String::new(),
        }
    }
}

pub struct Line {
    pub id: i64,
    /// `YYYY-MM-DD` it was logged: a to-do's no-JS edit link opens that day.
    pub day: String,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    /// `today`, `Wed 11`.
    pub when: String,
    pub via: Option<String>,
    pub via_epic: bool,
    pub author: String,
    /// To-dos: `Fri 13 Nov`, and whether that is past.
    pub due: Option<String>,
    pub late: bool,
    pub note: Option<NoteRef>,
}

#[derive(Template)]
#[template(path = "tags/hub.html")]
pub struct Hub {
    pub layout: Layout,
    pub tag: String,
    /// `23 lines · 3 open to-dos · 2 notes · last used today`.
    pub used: String,
    /// (item_type, id): epics, series and topics whose tags field holds it.
    pub tagged_by: Vec<(String, String)>,
    pub todos: Vec<Line>,
    /// Decisions, blockers, impact: the highlighted block.
    pub hi: Vec<Line>,
    pub recent: Vec<Line>,
    /// Past the first page of recent lines, folded.
    pub older: Vec<Line>,
    pub epics: Vec<HubEpic>,
    /// (id, title, author, `9 Nov` / `today`).
    pub notes: Vec<(i64, String, String, String)>,
    pub meetings: Vec<HubMeeting>,
    pub topics: Vec<HubTopic>,
    /// The topic of the same name (one name, one hub).
    pub topic: Option<SameTopic>,
    /// The meeting series of the same name: `3 meetings · last Wed 11 Nov`.
    pub series: Option<String>,
    /// Coming up: (`Fri 13 · 14:00`, title, own event).
    pub coming: Vec<(String, String, bool)>,
    /// The ⋯ menu's confirm step: `rename` · `merge` · `delete`.
    pub act: Option<String>,
    pub error: Option<String>,
    /// Other tags, for the merge box.
    pub known: Vec<String>,
}

pub struct HubMeeting {
    pub name: String,
    /// `series · tags it · 3 meetings, last Tue 10 Nov` / `Wed 11 Nov · 1 line tagged in the notes`.
    pub text: String,
}

pub struct SameTopic {
    pub depth: u8,
    pub own_words: String,
}

impl SameTopic {
    pub fn label(&self) -> &'static str {
        if self.depth == 0 {
            "not rated yet"
        } else {
            crate::domain::depth::label(self.depth)
        }
    }
}

/// After rename / merge / delete: the page's `<main>` becomes this, with Undo.
#[derive(Template)]
#[template(path = "tags/done.html")]
pub struct Done {
    pub what: String,
    /// Sources rewritten.
    pub changed: usize,
    pub snap: String,
    /// Where Undo lands (the old hub).
    pub back: String,
    /// The tag it now lives under.
    pub open: Option<String>,
}

/// Toast body for the capture bar's `#toast` ("Tag them" and its Undo).
#[derive(Template)]
#[template(path = "tags/toast.html")]
pub struct Toast {
    pub text: String,
    pub tag: Option<String>,
    /// Set = offer Undo.
    pub snap: Option<String>,
    pub back: String,
}

impl Hub {
    pub fn back(&self) -> String {
        format!("/tags/{}", self.tag)
    }

    /// The callout and props when a topic or series shares the name.
    pub fn merged(&self) -> bool {
        self.topic.is_some() || self.series.is_some()
    }

    /// Nothing anywhere carries or shares the name yet.
    pub fn is_empty(&self) -> bool {
        self.used.is_empty() && self.tagged_by.is_empty() && !self.merged()
    }
}
