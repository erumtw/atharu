use crate::{domain::entry::NoteRef, layout::Layout};
use askama::Template;

/// The kind menu's six rows: (posted prefix, mark, word, the key that picks it by typing). `""` = done, no prefix.
pub const KINDS: [(&str, &str, &str, &str); 6] = [
    ("", "·", "done", ""),
    ("!", "!", "decision", "!"),
    ("x", "x", "blocker", "x␣"),
    ("+", "+", "impact", "+"),
    ("?", "?", "learned", "?"),
    (">", ">", "to-do", ">"),
];

pub struct Saved {
    pub id: i64,
    pub kind: String,
    pub key: Option<String>,
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
    /// The checked kind (index into KINDS).
    pub kind: usize,
    /// Its mark and word, on the kind button.
    pub kind_mark: &'static str,
    pub kind_word: &'static str,
    /// Prefill for the box that shows.
    pub text: String,
    /// The note form's `back`: here, on the same kind.
    pub back_note: String,
    pub error: Option<&'static str>,
    /// Just saved: "✓ Saved note · Open".
    pub saved_note: Option<i64>,
    /// The meeting card / start box (series capped to the menu's length). Record audio and Stop show in the app
    /// only (`rec.mac`), enabled once the model is on disk.
    pub meeting: crate::recording::MeetingState,
    /// `24 min` of the running meeting; empty when none or its start time is unknown.
    pub since: String,
    /// Just ended from here: (id, name) for "Ended X · Open".
    pub ended: Option<(i64, String)>,
    pub todos: Vec<Todo>,
    pub overdue: usize,
    pub due_today: usize,
    /// Open to-dos not listed here (later, undated, past the cap).
    pub more: i64,
}
