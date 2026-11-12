use super::{queries, view};
use crate::{app::AppState, domain::dates, error::AppResult, layout::Layout, web};
use axum::{
    extract::{Query, State},
    response::Response,
};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Deserialize;

/// Recent series offered as one-click pills (and in the tray menu).
pub const SERIES: usize = 3;
/// To-do rows before "+ N more"; the rest of the 480 px window scrolls, but past 8 it's a list for /todos.
const TODOS: usize = 8;

#[derive(Deserialize)]
pub struct PageQuery {
    /// Set by `POST /entries` with `back=/quick`: the line to confirm with Undo.
    saved: Option<i64>,
}

pub async fn page(State(st): State<AppState>, Query(q): Query<PageQuery>) -> AppResult<Response> {
    let today = st.today();
    let conn = st.conn();
    let saved = match q.saved {
        Some(id) => queries::entry(&conn, id)?.map(|(kind, key)| view::Saved { id, kind, key }),
        None => None,
    };
    let now = chrono::Local::now().naive_local();
    let running = queries::running(&conn)?.map(|r| view::Running {
        since: r
            .started_at
            .as_deref()
            .map_or_else(String::new, |s| since(s, now)),
        id: r.id,
        name: r.name,
    });
    let all = queries::due_plans(&conn, &dates::iso(today))?;
    let overdue = all
        .iter()
        .filter(|(_, _, d)| *d < dates::iso(today))
        .count();
    let layout = Layout::load(&conn, "quick", "Quick", vec![])?;
    let todos: Vec<view::Todo> = all
        .iter()
        .take(TODOS)
        .map(|(id, text, due)| {
            let d = NaiveDate::parse_from_str(due, "%Y-%m-%d").unwrap_or(today);
            view::Todo {
                id: *id,
                text: text.clone(),
                due: due_label(d, today),
                late: d < today,
            }
        })
        .collect();
    web::html(&view::Page {
        date: dates::label(today),
        kinds: &view::KINDS,
        saved,
        running,
        series: queries::recent_series(&conn, SERIES)?,
        overdue,
        due_today: all.len() - overdue,
        more: layout.open_plans - todos.len() as i64,
        todos,
        layout,
    })
}

/// `since 09:17 · 24 min`: minutes, not a ticking clock (the window reloads each time it opens).
fn since(started: &str, now: NaiveDateTime) -> String {
    match NaiveDateTime::parse_from_str(started, "%Y-%m-%d %H:%M:%S") {
        Ok(t) => format!(
            "since {} · {} min",
            t.format("%H:%M"),
            (now - t).num_minutes().max(0)
        ),
        Err(_) => String::new(),
    }
}

/// Short, the row is 360 wide: `today`, a weekday within the past week, else `3 Nov`.
fn due_label(d: NaiveDate, today: NaiveDate) -> String {
    match (today - d).num_days() {
        0 => "today".into(),
        1..=6 => d.format("%a").to_string(),
        _ => d.format("%-d %b").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        let t: NaiveDate = "2026-11-12".parse().unwrap();
        assert_eq!(due_label(t, t), "today");
        assert_eq!(due_label("2026-11-10".parse().unwrap(), t), "Tue");
        assert_eq!(due_label("2026-11-03".parse().unwrap(), t), "3 Nov");
        let now =
            NaiveDateTime::parse_from_str("2026-11-12 09:41:00", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(since("2026-11-12 09:17:30", now), "since 09:17 · 23 min");
        assert_eq!(since("junk", now), "");
    }
}
