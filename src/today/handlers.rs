use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, parse::parse},
    error::{AppError, AppResult},
    layout::Layout,
    web,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DayQuery {
    day: Option<String>,
}

#[derive(Deserialize)]
pub struct LineForm {
    line: String,
    day: Option<String>,
}

/// A bad or missing `day` means today: the date picker never produces one, a hand-typed URL might.
fn pick_day(day: Option<&str>, today: NaiveDate) -> NaiveDate {
    day.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or(today)
}

fn groups(conn: &Connection, day: NaiveDate) -> rusqlite::Result<Vec<view::Group>> {
    let mut out: Vec<view::Group> = Vec::new();
    for r in queries::day_entries(conn, &dates::iso(day))? {
        let row = view::Row {
            id: r.id,
            kind: r.kind,
            topic: r.topic,
            text: r.text,
        };
        // Rows arrive sorted by group, so a new key always means a new group.
        match out.last_mut() {
            Some(g) if g.key == r.epic_key => g.rows.push(row),
            _ => out.push(view::Group {
                key: r.epic_key,
                title: r.epic_title,
                rows: vec![row],
            }),
        }
    }
    Ok(out)
}

fn render_page(
    st: &AppState,
    day: NaiveDate,
    line: String,
    error: Option<String>,
) -> AppResult<view::Page> {
    let conn = st.conn();
    Ok(view::Page {
        layout: Layout::load(&conn, "today", "Today", vec![])?,
        day: view::Day::new(day, st.today()),
        groups: groups(&conn, day)?,
        keys: queries::epic_keys(&conn)?,
        line,
        error,
        preview: None,
        saved: None,
        deleted: None,
    })
}

pub async fn page(State(st): State<AppState>, Query(q): Query<DayQuery>) -> AppResult<Response> {
    let day = pick_day(q.day.as_deref(), st.today());
    web::html(&render_page(&st, day, String::new(), None)?)
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<LineForm>,
) -> AppResult<Response> {
    let today = st.today();
    let day = pick_day(f.day.as_deref(), today);
    let entry = match parse(&f.line, today) {
        Ok(e) => e,
        Err(e) if web::is_htmx(&headers) => {
            // Form keeps its text (htmx only resets on 2xx); only the hint under it changes.
            let hint = view::Hint {
                error: Some(e.to_string()),
                preview: None,
            };
            let retarget = [("hx-retarget", "#hint"), ("hx-reswap", "innerHTML")];
            return Ok((
                retarget,
                web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &hint)?,
            )
                .into_response());
        }
        Err(e) => {
            let page = render_page(&st, day, f.line, Some(e.to_string()))?;
            return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &page);
        }
    };

    let (id, new_epic) = queries::insert_entry(&mut st.conn(), &dates::iso(day), &entry)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/?day={}", dates::iso(day))));
    }
    let swap = view::Swap {
        day: view::Day::new(day, today),
        groups: groups(&st.conn(), day)?,
        saved: Some(view::Saved {
            id,
            kind: entry.kind.as_str(),
            key: entry.epic_key,
            new_epic,
        }),
        deleted: None,
    };
    web::html(&swap)
}

/// `DELETE /entries/{id}` from htmx, `POST /entries/{id}/delete` from a plain form.
pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let (day, entry) = queries::delete_entry(&st.conn(), id)?.ok_or(AppError::NotFound)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/?day={day}")));
    }
    let d = pick_day(Some(&day), st.today());
    let swap = view::Swap {
        day: view::Day::new(d, st.today()),
        groups: groups(&st.conn(), d)?,
        saved: None,
        deleted: Some(view::Deleted {
            line: entry.to_line(),
            day,
        }),
    };
    web::html(&swap)
}

/// Live "→ will save as …" under the input; blank box = blank hint, not an error.
pub async fn preview(State(st): State<AppState>, Form(f): Form<LineForm>) -> AppResult<Response> {
    let hint = if f.line.trim().is_empty() {
        view::Hint {
            error: None,
            preview: None,
        }
    } else {
        match parse(&f.line, st.today()) {
            Ok(e) => view::Hint {
                error: None,
                preview: Some(view::Preview {
                    kind: e.kind.as_str(),
                    key: e.epic_key,
                    text: e.text,
                    due: e.due.map(dates::label),
                }),
            },
            Err(e) => view::Hint {
                error: Some(e.to_string()),
                preview: None,
            },
        }
    };
    web::html(&hint)
}
