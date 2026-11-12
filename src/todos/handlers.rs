use super::{queries, view};
use crate::{
    app::AppState,
    domain::{
        dates,
        plan::{Bucket, PIN_CAP, Plan, link_label, parse_plan},
    },
    error::{AppError, AppResult},
    layout::{Layout, TagFilter},
    web,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{Days, NaiveDate};
use rusqlite::Connection;
use serde::Deserialize;

const PIN_FULL: &str = "Top 3 is full. Unpin one first.";

fn lists(
    conn: &Connection,
    today: NaiveDate,
    tags: &[String],
    by_tag: bool,
) -> rusqlite::Result<view::Lists> {
    let plans = queries::open_plans(conn, tags)?;
    let mut groups: Vec<view::Group> = if by_tag {
        by_tag_groups(conn, plans)?
    } else {
        let mut groups: Vec<view::Group> = Bucket::ALL
            .iter()
            .map(|b| view::Group {
                title: b.title().into(),
                tag: None,
                plans: Vec::new(),
            })
            .collect();
        for p in plans {
            let i = Bucket::ALL
                .iter()
                .position(|b| *b == Bucket::of(p.due, today))
                .unwrap_or(0);
            groups[i].plans.push(p);
        }
        groups
    };
    groups.retain(|g| !g.plans.is_empty());
    let week = dates::week_start(today);
    // Filtered: "Done this week" would mix tagged and untagged, so it waits for the full list.
    let done = if tags.is_empty() {
        queries::done_between(conn, &dates::iso(week), &dates::iso(week + Days::new(6)))?
    } else {
        Vec::new()
    };
    Ok(view::Lists { groups, done })
}

/// Group by tag (D29): one group per tag, biggest first, "No tag" last; a to-do with two tags shows
/// under both, inherited ones say where from.
fn by_tag_groups(conn: &Connection, plans: Vec<Plan>) -> rusqlite::Result<Vec<view::Group>> {
    let tagged = queries::open_plan_tags(conn)?;
    let mut out: Vec<view::Group> = Vec::new();
    for p in plans {
        let mine: Vec<&(i64, String, Option<String>)> =
            tagged.iter().filter(|(id, _, _)| *id == p.id).collect();
        let names: Vec<(Option<String>, Option<String>)> = if mine.is_empty() {
            vec![(None, None)]
        } else {
            mine.iter()
                .map(|(_, t, v)| (Some(t.clone()), v.clone()))
                .collect()
        };
        for (tag, via) in names {
            let plan = Plan { via, ..p.clone() };
            match out.iter_mut().find(|g| g.tag == tag) {
                Some(g) => g.plans.push(plan),
                None => out.push(view::Group {
                    title: if tag.is_none() {
                        "No tag".into()
                    } else {
                        String::new()
                    },
                    tag,
                    plans: vec![plan],
                }),
            }
        }
    }
    out.sort_by(|a, b| {
        (a.tag.is_none(), std::cmp::Reverse(a.plans.len()), &a.tag).cmp(&(
            b.tag.is_none(),
            std::cmp::Reverse(b.plans.len()),
            &b.tag,
        ))
    });
    Ok(out)
}

fn render_page(
    st: &AppState,
    tags: Vec<String>,
    by_tag: bool,
    line: String,
    error: Option<String>,
    notice: Option<String>,
) -> AppResult<view::Page> {
    let today = st.today();
    let conn = st.conn();
    let lists = lists(&conn, today, &tags, by_tag)?;
    let shown = queries::open_plans(&conn, &tags)?.len();
    Ok(view::Page {
        layout: Layout::load(&conn, "todos", "To-dos", vec![])?,
        today,
        open_total: queries::open_plans(&conn, &[])?.len(),
        shown,
        by_tag,
        filter: TagFilter {
            tags,
            path: if by_tag {
                "/todos?group=tag&"
            } else {
                "/todos?"
            }
            .into(),
            keep: if by_tag {
                vec![("group", "tag".into())]
            } else {
                Vec::new()
            },
            known: queries::tag_names(&conn)?,
        },
        lists,
        keys: queries::epic_keys(&conn)?,
        line,
        error,
        notice,
    })
}

#[derive(Deserialize)]
pub struct PageQuery {
    /// `tag` = one group per tag instead of by due date.
    group: Option<String>,
    /// Tag filter, comma list (any of them); `add` = the filter row's add box.
    tag: Option<String>,
    add: Option<String>,
}

pub async fn page(State(st): State<AppState>, Query(q): Query<PageQuery>) -> AppResult<Response> {
    let tags = TagFilter::pick(q.tag.as_deref(), q.add.as_deref());
    let by_tag = q.group.as_deref() == Some("tag");
    web::html(&render_page(&st, tags, by_tag, String::new(), None, None)?)
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
            let page = render_page(&st, Vec::new(), false, f.line, Some(e.to_string()), None)?;
            return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &page);
        }
    };
    // Logged today: the day a plan was made, whatever its due date.
    queries::insert_plan(&mut st.conn(), &dates::iso(today), &entry)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back("/todos"));
    }
    let tags = TagFilter::pick(web::page_param(&headers, "tag").as_deref(), None);
    let by_tag = web::page_param(&headers, "group").as_deref() == Some("tag");
    web::html(&view::ListsSwap {
        today,
        lists: lists(&st.conn(), today, &tags, by_tag)?,
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
            let page = render_page(
                &st,
                Vec::new(),
                false,
                String::new(),
                None,
                Some(PIN_FULL.into()),
            )?;
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
