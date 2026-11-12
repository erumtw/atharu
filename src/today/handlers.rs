use super::{queries, view};
use crate::{
    app::AppState,
    domain::{
        dates,
        flags::{self, Flag},
        parse::parse,
    },
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
            meeting: r.meeting,
            author: r.author,
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

fn plans(conn: &Connection, today: NaiveDate) -> rusqlite::Result<view::Plans> {
    let (pinned, other) = queries::plans(conn, &dates::iso(dates::week_start(today)))?
        .into_iter()
        .partition(|p| p.pinned);
    Ok(view::Plans { pinned, other })
}

/// Flagged epics (worst first), then overdue to-dos.
fn needs(
    conn: &Connection,
    plans: &view::Plans,
    today: NaiveDate,
) -> rusqlite::Result<Vec<view::Need>> {
    let mut out: Vec<(Flag, view::Need)> = Vec::new();
    for e in queries::active_epic_facts(conn)? {
        let facts = flags::EpicFacts {
            status: "active",
            target: e.target,
            last_activity: e.last_activity,
            open_plans: e.open_plans,
        };
        if let Some(f) = flags::flag(&facts, today) {
            let text = if e.title.is_empty() {
                f.reason
            } else {
                format!("{} · {}", e.title, f.reason)
            };
            out.push((
                f.flag,
                view::Need {
                    class: f.flag.class(),
                    label: f.flag.label(),
                    href: format!("/epics/{}", e.key),
                    key: Some(e.key),
                    text,
                },
            ));
        }
    }
    out.sort_by_key(|(f, _)| *f as u8);
    let mut needs: Vec<view::Need> = out.into_iter().map(|(_, n)| n).collect();
    for p in plans.pinned.iter().chain(&plans.other) {
        if p.is_late(&today) {
            needs.push(view::Need {
                class: Flag::Overdue.class(),
                label: "overdue",
                key: p.epic_key.clone(),
                text: format!("{} · was due {}", p.text, p.due_label()),
                href: "/todos".into(),
            });
        }
    }
    Ok(needs)
}

fn render_page(
    st: &AppState,
    day: NaiveDate,
    line: String,
    error: Option<String>,
) -> AppResult<view::Page> {
    let conn = st.conn();
    let plans = plans(&conn, st.today())?;
    Ok(view::Page {
        layout: Layout::load(&conn, "today", "Today", vec![])?,
        banner: view::Banner {
            active: queries::active_meeting(&conn)?
                .map(|(id, name)| view::ActiveMeeting { id, name }),
            series: queries::meeting_series(&conn)?,
            name: String::new(),
            error: None,
            back: format!("/?day={}", dates::iso(day)),
        },
        today: st.today(),
        needs: needs(&conn, &plans, st.today())?,
        plans,
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

    let (id, new_epic) = queries::insert_entry(&mut st.conn(), &dates::iso(day), &entry, "me")?;
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/?day={}", dates::iso(day))));
    }
    // One guard per statement: two `st.conn()` temporaries in one expression deadlock the mutex.
    let conn = st.conn();
    let swap = view::Swap {
        today,
        plans: plans(&conn, today)?,
        day: view::Day::new(day, today),
        groups: groups(&conn, day)?,
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
    let conn = st.conn();
    let swap = view::Swap {
        today: st.today(),
        plans: plans(&conn, st.today())?,
        day: view::Day::new(d, st.today()),
        groups: groups(&conn, d)?,
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
