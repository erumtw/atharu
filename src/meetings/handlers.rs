use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, entry::Kind, entry::slugify},
    error::{AppError, AppResult},
    layout::{Layout, crumb},
    web,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Deserialize;

/// Plain POSTs go back where they came from; only same-site paths, never `//host`.
fn safe_back(back: Option<&str>) -> String {
    match back {
        Some(b) if b.starts_with('/') && !b.starts_with("//") => b.to_string(),
        _ => "/meetings".into(),
    }
}

fn banner(
    conn: &Connection,
    back: String,
    name: String,
    error: Option<String>,
) -> rusqlite::Result<view::Banner> {
    Ok(view::Banner {
        active: queries::active(conn)?.map(|(id, name)| view::Active { id, name }),
        series: queries::series(conn)?.into_iter().map(|(n, _)| n).collect(),
        name,
        error,
        back,
    })
}

fn side(conn: &Connection) -> rusqlite::Result<Vec<view::SideSeries>> {
    Ok(queries::series(conn)?
        .into_iter()
        .map(|(name, count)| view::SideSeries { name, count })
        .collect())
}

fn actions(open: i64, done: i64, dropped: i64) -> String {
    let parts: Vec<String> = [(open, "open"), (done, "done"), (dropped, "dropped")]
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, l)| format!("{n} {l}"))
        .collect();
    if parts.is_empty() {
        "—".into()
    } else {
        parts.join(" · ")
    }
}

fn first_line(notes: &str) -> String {
    let line = notes
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    match line.char_indices().nth(40) {
        None if line.is_empty() => "—".into(),
        None => line.to_string(),
        Some((i, _)) => format!("{}…", &line[..i]),
    }
}

fn label(day: &str) -> String {
    NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(dates::label)
        .unwrap_or_else(|_| day.to_string())
}

#[derive(Deserialize)]
pub struct ListQuery {
    name: Option<String>,
}

fn render_list(st: &AppState, filter: Option<String>, b: view::Banner) -> AppResult<view::List> {
    let conn = st.conn();
    let mut groups: Vec<view::SeriesGroup> = Vec::new();
    for m in queries::meetings(&conn, filter.as_deref())? {
        let row = view::ListRow {
            id: m.id,
            day: label(&m.day),
            decisions: m.decisions,
            learned: m.learned,
            actions: actions(m.open, m.done, m.dropped),
            notes: first_line(&m.notes),
            running: m.ended_at.is_none(),
        };
        // Rows come sorted by series, so a new name always means a new group.
        match groups.last_mut() {
            Some(g) if g.name == m.name => g.rows.push(row),
            _ => groups.push(view::SeriesGroup {
                name: m.name,
                rows: vec![row],
            }),
        }
    }
    Ok(view::List {
        layout: Layout::load(&conn, "meetings", "Meetings", vec![])?,
        side: side(&conn)?,
        banner: b,
        groups,
        filter,
    })
}

pub async fn list(State(st): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Response> {
    let b = banner(&st.conn(), "/meetings".into(), String::new(), None)?;
    let filter = q.name.filter(|n| !n.is_empty());
    web::html(&render_list(&st, filter, b)?)
}

#[derive(Deserialize)]
pub struct StartForm {
    name: String,
    back: Option<String>,
}

pub async fn start(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<StartForm>,
) -> AppResult<Response> {
    let back = safe_back(f.back.as_deref());
    let slug = slugify(&f.name);
    let error = if slug.is_empty() {
        // Not on the States board; same voice as the line box's "Type what you did first."
        Some("Type a meeting name first.".to_string())
    } else {
        queries::start(&st.conn(), &dates::iso(st.today()), &slug)?
            .err()
            .map(|running| format!("End {running} first."))
    };
    if error.is_none() && !web::is_htmx(&headers) {
        return Ok(web::back(&back));
    }
    let status = if error.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    let kept = if error.is_some() {
        f.name
    } else {
        String::new()
    };
    let b = banner(&st.conn(), back, kept, error)?;
    if web::is_htmx(&headers) {
        return web::html_status(status, &view::BannerSwap { banner: b });
    }
    // No JS and rejected: the meetings page carries the same box, with the message.
    web::html_status(status, &render_list(&st, None, b)?)
}

#[derive(Deserialize)]
pub struct BackForm {
    back: Option<String>,
}

pub async fn end(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<BackForm>,
) -> AppResult<Response> {
    let back = safe_back(f.back.as_deref());
    let at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    // Already ended (double click, stale tab) is fine; only an unknown id is an error.
    if !queries::end(&st.conn(), id, &at)? && queries::meeting(&st.conn(), id)?.is_none() {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&back));
    }
    let b = banner(&st.conn(), back, String::new(), None)?;
    web::html(&view::BannerSwap { banner: b })
}

pub async fn page(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Response> {
    let conn = st.conn();
    let m = queries::meeting(&conn, id)?.ok_or(AppError::NotFound)?;
    let siblings = queries::siblings(&conn, &m.name, id)?;
    let mut groups: Vec<view::KindGroup> = [
        (Kind::Decision, "Decisions"),
        (Kind::Learn, "Learned"),
        (Kind::Impact, "Impact"),
        (Kind::Blocker, "Blockers"),
        (Kind::Done, "Done"),
    ]
    .iter()
    .map(|(_, title)| view::KindGroup {
        title,
        lines: Vec::new(),
    })
    .collect();
    for l in queries::lines(&conn, id)? {
        let i = match l.kind {
            Kind::Decision => 0,
            Kind::Learn => 1,
            Kind::Impact => 2,
            Kind::Blocker => 3,
            _ => 4,
        };
        groups[i].lines.push(view::Line {
            kind: l.kind,
            epic_key: l.epic_key,
            topic: l.topic,
            text: l.text,
        });
    }
    groups.retain(|g| !g.lines.is_empty());
    let day = NaiveDate::parse_from_str(&m.day, "%Y-%m-%d")
        .map(|d| d.format("%a %-d %b %Y").to_string())
        .unwrap_or_else(|_| m.day.clone());
    web::html(&view::Page {
        layout: Layout::load(
            &conn,
            "meetings",
            label(&m.day),
            vec![
                crumb("Meetings", "/meetings"),
                crumb(m.name.clone(), format!("/meetings?name={}", m.name)),
            ],
        )?,
        side: side(&conn)?,
        id,
        series_count: siblings.len() + 1,
        siblings: siblings
            .iter()
            .take(2)
            .map(|(i, d)| (*i, label(d)))
            .collect(),
        plans: queries::plans(&conn, id)?,
        groups,
        day,
        // Stored as local `YYYY-MM-DD HH:MM:SS`; the page only needs the time.
        ended: m.ended_at.map(|t| t.get(11..16).unwrap_or(&t).to_string()),
        mentions: queries::mentions(&conn, &m.name)?,
        name: m.name,
        notes: m.notes,
        today: st.today(),
        back: format!("/meetings/{id}"),
    })
}

#[derive(Deserialize)]
pub struct NotesForm {
    notes: String,
}

/// Autosave: htmx posts as you type; without JS the same form posts via Save.
pub async fn save_notes(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<NotesForm>,
) -> AppResult<Response> {
    if !queries::set_notes(&st.conn(), id, &f.notes)? {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/meetings/{id}")));
    }
    web::html(&view::Saved)
}
