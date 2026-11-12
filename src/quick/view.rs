use crate::{domain::entry::NoteRef, layout::Layout};
use askama::Template;

/// The six kind tiles: (posted prefix, key mark, key class, word). `""` = done, no prefix.
pub const KINDS: [(&str, &str, &str, &str); 6] = [
    ("", "·", "k", "done"),
    ("!", "!", "k", "decision"),
    ("x", "x", "k bl", "blocker"),
    ("+", "+", "k imp", "impact"),
    ("?", "?", "k", "learned"),
    (">", ">", "k", "to-do"),
];

pub struct Saved {
    pub id: i64,
    pub kind: String,
    pub key: Option<String>,
}

pub struct Running {
    pub id: i64,
    pub name: String,
    /// `since 09:17 · 24 min`; empty when the start time is unknown.
    pub since: String,
}

pub struct Todo {
    pub id: i64,
    pub text: String,
    /// `today`, `Tue`, `3 Nov`.
    pub due: String,
    pub late: bool,
    pub note: Option<NoteRef>,
}

#[derive(Template)]
#[template(path = "quick/page.html")]
pub struct Page {
    pub layout: Layout,
    /// `Thu 12 Nov`.
    pub date: String,
    pub kinds: &'static [(&'static str, &'static str, &'static str, &'static str)],
    pub saved: Option<Saved>,
    pub running: Option<Running>,
    pub series: Vec<String>,
    pub todos: Vec<Todo>,
    pub overdue: usize,
    pub due_today: usize,
    /// Open to-dos not listed here (later, undated, past the cap).
    pub more: i64,
}
