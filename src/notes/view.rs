use crate::{domain::entry::Kind, layout::Layout};
use askama::Template;
use serde::Serialize;

pub struct SideNote {
    pub id: i64,
    pub title: String,
}

/// Sidebar groups: mine apart from agent-written ones (spec: never mixed).
pub struct Side {
    pub mine: Vec<SideNote>,
    pub agents: Vec<SideNote>,
    /// Note to highlight, 0 = none.
    pub on: i64,
}

pub struct ListRow {
    pub id: i64,
    pub title: String,
    /// Agent slug for the badge; `None` = mine.
    pub agent: Option<String>,
    /// Snippet split around the first search hit, so the hit can be bold.
    pub before: String,
    pub hit: String,
    pub after: String,
    /// `2h ago`.
    pub ago: String,
    /// Kind of the first linked line, drawn as its badge (D49).
    pub kind: Option<Kind>,
    /// `to-do`, `decision`: the badge's "Linked to a … line".
    pub kind_name: &'static str,
    pub lines: i64,
}

#[derive(Template)]
#[template(path = "notes/list.html")]
pub struct List {
    pub layout: Layout,
    pub side: Side,
    pub q: String,
    pub filter: crate::layout::TagFilter,
    /// `?linked=1`: only notes some line links to.
    pub linked: bool,
    /// The page with the linked filter switched on / off (other filters kept).
    pub linked_on: String,
    pub linked_off: String,
    pub mine: Vec<ListRow>,
    pub agents: Vec<ListRow>,
}

/// One `[[X]]` in the note: where it goes, or `None` when it resolves to nothing.
pub struct Link {
    pub label: String,
    pub href: Option<String>,
}

#[derive(Template)]
#[template(path = "notes/page.html")]
pub struct Page {
    pub layout: Layout,
    pub side: Side,
    pub id: i64,
    pub title: String,
    /// Markdown as stored; the editor loads it, no-JS edits it in the textarea.
    pub body: String,
    /// Server-rendered body: the read-only view until the editor takes over.
    pub html: String,
    pub agent: Option<String>,
    pub links: Vec<Link>,
    pub ago: String,
    pub head: Lines,
}

/// One line in the note's header.
pub struct HeadLine {
    pub id: i64,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    pub epic_key: Option<String>,
    /// To-dos: `open` / `done` / `dropped`; `None` for other kinds.
    pub plan_state: Option<String>,
    /// To-dos: the date field's value.
    pub due: String,
    /// `done Thu 12 Nov` / `dropped Thu 12 Nov`, once it left `open`.
    pub closed: String,
    pub day: String,
    /// `today` or `Thu 12 Nov`.
    pub day_label: String,
}

/// The header on /notes/<id> (D49): the lines this note belongs to. Re-fetched after a check or due
/// change (`plans-changed`), so the row the plan handler would swap is never needed here.
#[derive(Template)]
#[template(path = "notes/lines.html")]
pub struct Lines {
    pub id: i64,
    /// Deleted while their Undo lives: (text, snapshot).
    pub gone: Vec<(String, String)>,
    /// Newest five; the rest fold into `more`.
    pub shown: Vec<HeadLine>,
    pub more: Vec<HeadLine>,
}

impl Lines {
    pub fn count(&self) -> usize {
        self.gone.len() + self.shown.len() + self.more.len()
    }
}

#[derive(Template)]
#[template(path = "notes/saved.html")]
pub struct Saved;

#[derive(Serialize)]
pub struct PageItem {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
}

#[derive(Serialize)]
pub struct PagesData {
    pub pages: Vec<PageItem>,
}

/// `{"data": {"pages": [...]}}`, the collection envelope.
#[derive(Serialize)]
pub struct Pages {
    pub data: PagesData,
}
