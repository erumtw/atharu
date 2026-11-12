use crate::layout::Layout;
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
}

#[derive(Template)]
#[template(path = "notes/list.html")]
pub struct List {
    pub layout: Layout,
    pub side: Side,
    pub q: String,
    pub filter: crate::layout::TagFilter,
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
