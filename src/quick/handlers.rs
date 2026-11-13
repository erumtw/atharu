use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, markdown, plan},
    error::AppResult,
    layout::Layout,
    recording::MeetingState,
    web,
};
use axum::{
    extract::{Query, State},
    response::Response,
};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Deserialize;

/// Recent series offered as one-click pills (and in the tray menu).
pub const SERIES: usize = 3;
/// The start box's series menu (T93): enough to find last month's, few enough to scan.
const MENU_SERIES: usize = 12;
/// To-do rows before "+ N more": five rows fit the 480 px window with no inner scroll (0.2.6); past that it's /todos' list.
const TODOS: usize = 5;

#[derive(Deserialize)]
pub struct PageQuery {
    /// Set by `POST /entries` with `back=/quick`: the line to confirm with Undo.
    saved: Option<i64>,
    /// Set by `POST /notes/new`: the note to offer "Open" on (D53).
    note: Option<i64>,
    /// `note` = the box is a quick note (the header's note button without JS is a GET here, carrying the form's fields).
    mode: Option<String>,
    /// The kind to check: the one picked before Note, so the box goes back to it.
    kind: Option<String>,
    /// What was in the line when Note was picked: it moves into the note box.
    line: Option<String>,
    /// A quick note that was only whitespace (plain POST; htmx gets the hint directly).
    err: Option<String>,
    /// Set by End meeting from here: "Ended X · Open".
    ended: Option<i64>,
}

/// `/quick?kind=…` for the note form's `back`: the prefixes are `!x+?>`, and `+` would read as a space.
fn back_to_kind(kind: &str) -> String {
    let enc: String = kind.bytes().map(|b| format!("%{b:02X}")).collect();
    format!("/quick?kind={enc}")
}

pub async fn page(State(st): State<AppState>, Query(q): Query<PageQuery>) -> AppResult<Response> {
    let today = st.today();
    let conn = st.conn();
    let saved = match q.saved {
        Some(id) => queries::entry(&conn, id)?.map(|(kind, key)| view::Saved { id, kind, key }),
        None => None,
    };
    let now = chrono::Local::now().naive_local();
    let mut meeting = MeetingState::load(&conn, st.mac.as_deref())?;
    meeting.series.truncate(MENU_SERIES);
    let since = meeting
        .running
        .as_ref()
        .and_then(|r| r.started_at.as_deref())
        .map_or_else(String::new, |s| since(s, now));
    let all = queries::due_plans(&conn, &dates::iso(today))?;
    let overdue = all.iter().filter(|p| p.due < dates::iso(today)).count();
    let layout = Layout::load(&conn, "quick", "Quick", vec![])?;
    let todos: Vec<view::Todo> = all
        .iter()
        .take(TODOS)
        .map(|p| {
            let d = NaiveDate::parse_from_str(&p.due, "%Y-%m-%d").unwrap_or(today);
            view::Todo {
                id: p.id,
                text: p.text.clone(),
                // The to-do row's quiet due date, as on Today and To-dos (`Plan::due_quiet` is this).
                due: plan::quiet(d, today),
                late: d < today,
                today: d == today,
                note: p.note.clone(),
            }
        })
        .collect();
    let kind = view::KINDS
        .iter()
        .position(|(v, ..)| q.kind.as_deref() == Some(v))
        .unwrap_or(0);
    web::html(&view::Page {
        date: dates::label(today),
        kinds: &view::KINDS,
        note_mode: q.mode.as_deref() == Some("note"),
        back_note: back_to_kind(view::KINDS[kind].0),
        kind,
        text: q.line.unwrap_or_default(),
        error: q.err.is_some().then_some(markdown::NO_TITLE),
        saved_note: q.note,
        saved,
        meeting,
        since,
        ended: match q.ended {
            Some(id) => queries::meeting_name(&conn, id)?.map(|n| (id, n)),
            None => None,
        },
        overdue,
        due_today: all.len() - overdue,
        more: layout.open_plans - todos.len() as i64,
        todos,
        layout,
    })
}

/// `24 min`: minutes, not a ticking clock (the window reloads each time it opens).
fn since(started: &str, now: NaiveDateTime) -> String {
    match NaiveDateTime::parse_from_str(started, "%Y-%m-%d %H:%M:%S") {
        Ok(t) => format!("{} min", (now - t).num_minutes().max(0)),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        let now =
            NaiveDateTime::parse_from_str("2026-11-12 09:41:00", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(since("2026-11-12 09:17:30", now), "23 min");
        assert_eq!(since("junk", now), "");
        assert_eq!(back_to_kind("+"), "/quick?kind=%2B");
        assert_eq!(back_to_kind(""), "/quick?kind=");
    }
}
