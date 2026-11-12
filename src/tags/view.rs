use super::queries::{HubEpic, TagRow};
use crate::{domain::entry::Kind, layout::Layout};
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
}

#[derive(Template)]
#[template(path = "tags/hub.html")]
pub struct Hub {
    pub layout: Layout,
    pub tag: String,
    /// `23 lines · 3 open to-dos · 2 notes · last used today`.
    pub used: String,
    pub tagged_by: Vec<String>,
    pub todos: Vec<Line>,
    /// Decisions, blockers, impact: the highlighted block.
    pub hi: Vec<Line>,
    pub recent: Vec<Line>,
    /// Past the first page of recent lines, folded.
    pub older: Vec<Line>,
    pub epics: Vec<HubEpic>,
    /// (id, title, author, `9 Nov` / `today`).
    pub notes: Vec<(i64, String, String, String)>,
    /// (name, `Wed 11 Nov`, lines).
    pub meetings: Vec<(String, String, i64)>,
}

impl Hub {
    pub fn back(&self) -> String {
        format!("/tags/{}", self.tag)
    }
}
