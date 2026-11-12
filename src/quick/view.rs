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
    /// Recording state for the card's Record audio / Stop (Mac app only; the browser shows neither).
    pub rec: crate::recording::Rec,
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
    /// The box is a quick note (D53): first line = title, the rest = body.
    pub note_mode: bool,
    /// The checked kind tile (index into KINDS).
    pub kind: usize,
    /// Prefill for the box that shows.
    pub text: String,
    /// The note form's `back`: here, on the same kind tile.
    pub back_note: String,
    pub error: Option<&'static str>,
    /// Just saved: "✓ Saved note · Open".
    pub saved_note: Option<i64>,
    pub running: Option<Running>,
    /// Start section's Record audio switch: shown in the app only, enabled once the model is on disk.
    pub rec_app: bool,
    pub model_ready: bool,
    /// Just ended from here: (id, name) for "Ended X · Open".
    pub ended: Option<(i64, String)>,
    pub series: Vec<String>,
    pub todos: Vec<Todo>,
    pub overdue: usize,
    pub due_today: usize,
    /// Open to-dos not listed here (later, undated, past the cap).
    pub more: i64,
}
