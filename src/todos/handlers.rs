use super::{queries, view};
use crate::{
    app::AppState,
    domain::{
        dates,
        plan::{Bucket, PIN_CAP, link_label, parse_plan},
    },
    error::{AppError, AppResult},
    layout::Layout,
    web,
};
use axum::{
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{Days, NaiveDate};
use rusqlite::Connection;
use serde::Deserialize;

const PIN_FULL: &str = "Top 3 is full. Unpin one first.";

fn lists(conn: &Connection, today: NaiveDate) -> rusqlite::Result<view::Lists> {
    let mut groups: Vec<view::Group> = Bucket::ALL
        .iter()
        .map(|b| view::Group {
            title: b.title(),
            plans: Vec::new(),
        })
        .collect();
    for p in queries::open_plans(conn)? {
        let i = Bucket::ALL
            .iter()
            .position(|b| *b == Bucket::of(p.due, today))
            .unwrap_or(0);
        groups[i].plans.push(p);
    }
    groups.retain(|g| !g.plans.is_empty());
    let week = dates::week_start(today);
    let done = queries::done_between(conn, &dates::iso(week), &dates::iso(week + Days::new(6)))?;
    Ok(view::Lists { groups, done })
}

fn render_page(
    st: &AppState,
    line: String,
    error: Option<String>,
    notice: Option<String>,
) -> AppResult<view::Page> {
    let today = st.today();
    let conn = st.conn();
    Ok(view::Page {
        layout: Layout::load(&conn, "todos", "To-dos", vec![])?,
        today,
        lists: lists(&conn, today)?,
        keys: queries::epic_keys(&conn)?,
        line,
        error,
        notice,
    })
}

pub async fn page(State(st): State<AppState>) -> AppResult<Response> {
    web::html(&render_page(&st, String::new(), None, None)?)
}

#[derive(Deserialize)]
pub struct LineForm {
    line: String,
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<LineForm>,
) -> AppResult<Response> {
    let today = st.today();
    let entry = match parse_plan(&f.line, today) {
        Ok(e) => e,
        Err(e) if web::is_htmx(&headers) => {
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
            let page = render_page(&st, f.line, Some(e.to_string()), None)?;
            return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &page);
        }
    };
    // Logged today: the day a plan was made, whatever its due date.
    queries::insert_plan(&mut st.conn(), &dates::iso(today), &entry)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back("/todos"));
    }
    web::html(&view::ListsSwap {
        today,
        lists: lists(&st.conn(), today)?,
    })
}

pub async fn preview(State(st): State<AppState>, Form(f): Form<LineForm>) -> AppResult<Response> {
    let hint = if f.line.trim().is_empty() {
        view::Hint {
            error: None,
            preview: None,
        }
    } else {
        match parse_plan(&f.line, st.today()) {
            Ok(e) => view::Hint {
                error: None,
                preview: Some(view::Preview {
                    key: e.epic_key,
                    link: e.link.as_deref().map(link_label),
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

/// Fields a row form may send; only the ones present are written (same idea as epic autosave).
#[derive(Deserialize)]
pub struct PlanForm {
    state: Option<String>,
    pinned: Option<String>,
    due: Option<String>,
    back: Option<String>,
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<PlanForm>,
) -> AppResult<Response> {
    let today = st.today();
    let back = web::safe_back(f.back.as_deref(), "/todos").to_string();
    let conn = st.conn();
    let p = queries::plan_by_id(&conn, id)?.ok_or(AppError::NotFound)?;

    let state = match f.state.as_deref() {
        None => None,
        Some(s @ ("open" | "done" | "dropped")) => Some(s),
        Some(_) => return Ok(StatusCode::UNPROCESSABLE_ENTITY.into_response()),
    };
    let due = match f.due.as_deref().map(str::trim) {
        None => None,
        Some("") => Some(None),
        Some(d) => match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(_) => Some(Some(d)),
            Err(_) => return Ok(StatusCode::UNPROCESSABLE_ENTITY.into_response()),
        },
    };
    let pin = f.pinned.as_deref().map(|v| v == "1");
    // Only open plans take a pin; the cap counts open pins only, so finished ones never block new work.
    let pin_full = pin == Some(true) && !p.pinned && queries::pinned_open(&conn, id)? >= PIN_CAP;

    if let Some(s) = state {
        queries::set_state(&conn, id, s, &dates::iso(today))?;
    }
    if let Some(d) = due {
        queries::set_due(&conn, id, d)?;
    }
    if let Some(on) = pin
        && !pin_full
        && (!on || p.is_open())
    {
        queries::set_pinned(&conn, id, on)?;
    }

    if !web::is_htmx(&headers) {
        if pin_full {
            drop(conn);
            let page = render_page(&st, String::new(), None, Some(PIN_FULL.into()))?;
            return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &page);
        }
        return Ok(web::back(&back));
    }
    let p = queries::plan_by_id(&conn, id)?.ok_or(AppError::NotFound)?;
    let row = view::Row {
        today,
        p,
        full: back == "/todos",
        back,
        pin_error: pin_full,
    };
    let status = if pin_full {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    web::html_status(status, &row)
}
