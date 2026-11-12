use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, flags, impact::impact_line, links, tags},
    error::{AppError, AppResult},
    layout::{Layout, crumb},
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
use std::collections::HashMap;

fn date(s: Option<&str>) -> Option<NaiveDate> {
    s.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

fn flag(e: &queries::EpicRow, today: NaiveDate) -> Option<view::Flag> {
    let facts = flags::EpicFacts {
        status: &e.status,
        target: date(e.target.as_deref()),
        last_activity: date(e.last_log.as_deref()),
        open_plans: e.open_plans,
    };
    flags::flag(&facts, today).map(|f| view::Flag {
        class: f.flag.class(),
        label: f.flag.label(),
        reason: f.reason,
    })
}

fn list_row(e: queries::EpicRow, today: NaiveDate) -> view::ListRow {
    let flag = flag(&e, today);
    let (target, target_note, late) = match date(e.target.as_deref()) {
        None => (String::new(), String::new(), false),
        Some(t) => {
            let days = (t - today).num_days();
            let note = match days {
                0 => "· today".to_string(),
                d if d < 0 => format!("· {}d late", -d),
                d => format!("· {d}d"),
            };
            (view::short(t), note, days < 0)
        }
    };
    let last_log = match date(e.last_log.as_deref()).map(|d| (today - d).num_days()) {
        None => "—".to_string(),
        Some(0) => "today".to_string(),
        Some(n) => format!("{n}d ago"),
    };
    view::ListRow {
        key: e.key,
        title: e.title,
        role: e.role,
        target,
        target_note,
        late,
        last_log,
        flag,
    }
}

#[derive(Deserialize)]
pub struct ListQuery {
    /// `failed` after the Jira Refresh button couldn't reach Jira.
    jira: Option<String>,
}

pub async fn list(State(st): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Response> {
    let today = st.today();
    let conn = st.conn();
    let (mut untitled, mut groups, mut closed) =
        (Vec::new(), Vec::<view::OkrGroup>::new(), Vec::new());
    for e in queries::all_epics(&conn)? {
        if e.status != "active" {
            let done = e.status == "done";
            closed.push(view::Closed {
                chip: match date(e.done_on.as_deref()) {
                    Some(d) if done => format!("done {}", view::short(d)),
                    _ => e.status.clone(),
                },
                key: e.key,
                jira: e.jira.filter(|_| e.title.is_empty()),
                title: e.title,
                done,
            });
        } else if e.title.is_empty() {
            untitled.push((e.key, e.jira));
        } else {
            let okr = if e.okr.is_empty() {
                "No OKR".to_string()
            } else {
                e.okr.clone()
            };
            let row = list_row(e, today);
            // Query sorts by OKR, so a new OKR always means a new group.
            match groups.last_mut() {
                Some(g) if g.okr == okr => g.rows.push(row),
                _ => groups.push(view::OkrGroup {
                    okr,
                    rows: vec![row],
                }),
            }
        }
    }
    web::html(&view::List {
        layout: Layout::load(&conn, "epics", "Epics", vec![])?,
        untitled,
        groups,
        closed,
        jira_on: st.settings.jira().is_some(),
        jira_failed: q.jira.as_deref() == Some("failed"),
    })
}

#[derive(Deserialize)]
pub struct TitleForm {
    title: String,
}

/// Inline "Type a title, press Enter" on `/epics`.
pub async fn set_title(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Form(f): Form<TitleForm>,
) -> AppResult<Response> {
    let title = f.title.trim().to_string();
    if !title.is_empty()
        && !queries::update_fields(&st.conn(), &key, &[("title", Some(title.clone()))])?
    {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back("/epics"));
    }
    web::html(&view::Named { key, title })
}

fn live(
    conn: &Connection,
    e: &queries::EpicRow,
    ev_line: String,
    ev_error: Option<String>,
) -> AppResult<view::Live> {
    let evidence: Vec<view::Evidence> = queries::evidence(conn, &e.key)?
        .into_iter()
        .map(|r| view::Evidence {
            id: r.id,
            url: r.url,
            note: r.note,
            author: r.author,
        })
        .collect();
    let urls: Vec<String> = evidence.iter().map(|v| v.url.clone()).collect();
    let title = if e.title.is_empty() { &e.key } else { &e.title };
    Ok(view::Live {
        key: e.key.clone(),
        impact: impact_line(title, &e.metric, &e.before, &e.after, &urls),
        metric: e.metric.clone(),
        before: e.before.clone(),
        after: e.after.clone(),
        evidence,
        ev_line,
        ev_error,
    })
}

fn load(conn: &Connection, key: &str) -> AppResult<queries::EpicRow> {
    queries::epic(conn, key)?.ok_or(AppError::NotFound)
}

fn render_page(
    st: &AppState,
    key: &str,
    save_error: Option<&'static str>,
    ev_line: String,
    ev_error: Option<String>,
) -> AppResult<view::Page> {
    let today = st.today();
    let conn = st.conn();
    let e = load(&conn, key)?;
    let epic_tags = tags::parse_list(&e.tags);
    let mut timeline: Vec<view::DayGroup> = Vec::new();
    let mut last_day = String::new();
    for r in queries::timeline(&conn, key)? {
        if r.day != last_day {
            let label = match date(Some(&r.day)) {
                Some(d) if d == today => "Today".to_string(),
                Some(d) => dates::label(d),
                None => r.day.clone(),
            };
            timeline.push(view::DayGroup {
                label,
                rows: Vec::new(),
            });
            last_day = r.day.clone();
        }
        if let Some(g) = timeline.last_mut() {
            let own = tags::tags(&r.text);
            g.rows.push(view::TimelineRow {
                kind: r.kind,
                topic: r.topic,
                inh: epic_tags
                    .iter()
                    .filter(|t| !own.contains(t))
                    .cloned()
                    .collect(),
                text: r.text,
                author: r.author,
            });
        }
    }
    let live = live(&conn, &e, ev_line, ev_error)?;
    let since = dates::week_start(today) - chrono::Days::new(49);
    let days: Vec<NaiveDate> = queries::entry_days_since(&conn, key, &dates::iso(since))?
        .iter()
        .filter_map(|d| date(Some(d)))
        .collect();
    let weeks = flags::weekly_counts(&days, today);
    let max = weeks.iter().copied().max().unwrap_or(0).max(1);
    let title = if e.title.is_empty() {
        e.key.clone()
    } else {
        e.title.clone()
    };
    Ok(view::Page {
        layout: Layout::load(&conn, "epics", title, vec![crumb("Epics", "/epics")])?,
        flag: flag(&e, today),
        activity: weeks.iter().map(|n| n * 100 / max).collect(),
        epic: view::EpicView {
            key: e.key,
            jira: e.jira.filter(|_| e.title.is_empty()),
            title: e.title,
            role: e.role,
            okr: e.okr,
            status: e.status,
            start: e.start.unwrap_or_default(),
            target: e.target.unwrap_or_default(),
            done_on: e.done_on.unwrap_or_default(),
        },
        field: tag_field(&conn, epic_tags)?,
        mentions: queries::mentions(&conn, key)?,
        live,
        timeline,
        save_error,
        delete_error: None,
    })
}

fn tag_field(conn: &Connection, tags: Vec<String>) -> rusqlite::Result<crate::layout::TagField> {
    Ok(crate::layout::TagField {
        id: "tags",
        tags,
        known: queries::tag_names(conn)?,
    })
}

pub async fn page(State(st): State<AppState>, Path(key): Path<String>) -> AppResult<Response> {
    web::html(&render_page(&st, &key, None, String::new(), None)?)
}

/// Turns submitted fields into column writes; `None` = some value is invalid (form can't produce it).
fn fields(form: &HashMap<String, String>) -> Option<Vec<(&'static str, Option<String>)>> {
    let mut out = Vec::new();
    for col in queries::EDITABLE {
        let Some(raw) = form.get(col) else { continue };
        let v = raw.trim();
        let val = match col {
            "role" if !["owner", "contributor"].contains(&v) => return None,
            "status" if !["active", "done", "dropped"].contains(&v) => return None,
            "start" | "target" | "done_on" if v.is_empty() => None,
            "start" | "target" | "done_on" => {
                NaiveDate::parse_from_str(v, "%Y-%m-%d").ok()?;
                Some(v.to_string())
            }
            // Free typing (`#Expense, hiring`) is folded to the stored `expense,hiring`; junk is dropped.
            "tags" => Some(crate::domain::tags::parse_list(v).join(",")),
            _ => Some(v.to_string()),
        };
        out.push((col, val));
    }
    Some(out)
}

const SAVE_FAILED: &str = "Couldn't save. Try again.";
const TARGET_BEFORE_START: &str = "Target can't be before the start.";

/// Target before start after this save (a field not in the form keeps its stored value). The date inputs
/// carry min/max so the picker greys those days, but a typed or scripted value still lands here.
fn backwards(e: &queries::EpicRow, f: &[(&'static str, Option<String>)]) -> bool {
    let pick = |col: &str, stored: &Option<String>| {
        f.iter()
            .find(|(c, _)| *c == col)
            .map_or(stored.clone(), |(_, v)| v.clone())
    };
    // ISO dates compare as strings.
    matches!((pick("start", &e.start), pick("target", &e.target)), (Some(s), Some(t)) if t < s)
}

/// Autosave: htmx PATCHes the form on every change; without JS the same form POSTs.
pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Form(mut form): Form<HashMap<String, String>>,
) -> AppResult<Response> {
    // A chip edit (× or the add box) folds into the plain `tags` field before validation.
    if form.contains_key("tags_add") || form.contains_key("tags_drop") {
        let cur = form.get("tags").cloned().unwrap_or_default();
        let list = tags::edit_list(
            &cur,
            form.get("tags_add").map(String::as_str),
            form.get("tags_drop").map(String::as_str),
        );
        form.insert("tags".into(), list.join(","));
    }
    let error = match fields(&form) {
        Some(f) => {
            let conn = st.conn();
            if backwards(&load(&conn, &key)?, &f) {
                Some(TARGET_BEFORE_START)
            } else if !queries::update_fields(&conn, &key, &f)? {
                return Err(AppError::NotFound);
            } else {
                None
            }
        }
        None => Some(SAVE_FAILED),
    };
    let ok = error.is_none();
    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    if !web::is_htmx(&headers) {
        if ok {
            return Ok(web::back(&format!("/epics/{key}")));
        }
        return web::html_status(status, &render_page(&st, &key, error, String::new(), None)?);
    }
    let conn = st.conn();
    let e = load(&conn, &key)?;
    web::html_status(
        status,
        &view::Swap {
            live: live(&conn, &e, String::new(), None)?,
            save_error: error,
            field: Some(tag_field(&conn, tags::parse_list(&e.tags))?),
        },
    )
}

#[derive(Deserialize)]
pub struct EvidenceForm {
    line: String,
}

/// "paste a link, then a note after a space".
pub async fn add_evidence(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Form(f): Form<EvidenceForm>,
) -> AppResult<Response> {
    let line = f.line.trim();
    let (url, note) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let error = (!links::is_http(url)).then(|| links::NOT_HTTP.to_string());
    if error.is_none() {
        queries::add_evidence(&st.conn(), &key, url, note.trim(), "me")?;
    }
    let (status, kept) = match &error {
        Some(_) => (StatusCode::UNPROCESSABLE_ENTITY, line.to_string()),
        None => (StatusCode::OK, String::new()),
    };
    if !web::is_htmx(&headers) {
        if error.is_none() {
            return Ok(web::back(&format!("/epics/{key}")));
        }
        return web::html_status(status, &render_page(&st, &key, None, kept, error)?);
    }
    let conn = st.conn();
    let e = load(&conn, &key)?;
    web::html_status(
        status,
        &view::Swap {
            live: live(&conn, &e, kept, error)?,
            save_error: None,
            field: None,
        },
    )
}

pub async fn delete_evidence(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((key, id)): Path<(String, i64)>,
) -> AppResult<Response> {
    if !queries::delete_evidence(&st.conn(), &key, id)? {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/epics/{key}")));
    }
    let conn = st.conn();
    let e = load(&conn, &key)?;
    web::html(&view::Swap {
        live: live(&conn, &e, String::new(), None)?,
        save_error: None,
        field: None,
    })
}

const HAS_LINES: &str = "Mark it Done or Dropped instead.";

/// Only an epic with no lines can go (lines need their key); evidence goes with it, Undo brings both back.
pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> AppResult<Response> {
    let res = queries::delete(&mut st.conn(), &key)?;
    match res {
        queries::Deleted::Missing => Err(AppError::NotFound),
        queries::Deleted::HasLines if web::is_htmx(&headers) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            [("hx-retarget", "#epic-del"), ("hx-reswap", "innerHTML")],
            HAS_LINES,
        )
            .into_response()),
        queries::Deleted::HasLines => {
            let mut page = render_page(&st, &key, None, String::new(), None)?;
            page.delete_error = Some(HAS_LINES);
            web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &page)
        }
        queries::Deleted::Gone(_) if !web::is_htmx(&headers) => Ok(web::back("/epics")),
        queries::Deleted::Gone(snap) => web::html(&deleted(&key, snap, None)),
    }
}

fn deleted(key: &str, snap: String, error: Option<String>) -> web::Deleted {
    web::Deleted {
        what: format!("Epic {key}"),
        restore: format!("/epics/{key}/restore"),
        snap,
        back: "/epics",
        back_label: "All epics",
        error,
    }
}

#[derive(Deserialize)]
pub struct SnapForm {
    snap: String,
}

pub async fn restore(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<SnapForm>,
) -> AppResult<Response> {
    let res = queries::restore(&mut st.conn(), &f.snap)?;
    match res {
        Ok(key) => Ok(web::restored(&headers, &format!("/epics/{key}"))),
        Err(key) => web::html_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            &deleted(
                &key,
                f.snap,
                Some(format!(
                    "{key} is back from a new line, so the old one can't return."
                )),
            ),
        ),
    }
}
