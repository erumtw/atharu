use super::{queries, view};
use crate::{
    app::AppState,
    domain::{links, markdown},
    error::{AppError, AppResult},
    layout::{Layout, TagFilter, crumb},
    web,
};
use axum::{
    Json,
    extract::{Form, Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use chrono::NaiveDateTime;
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashMap;

fn title_or_untitled(t: &str) -> String {
    if t.trim().is_empty() {
        "Untitled".into()
    } else {
        t.to_string()
    }
}

fn agent(author: &str) -> Option<String> {
    (author != "me").then(|| author.to_string())
}

/// `updated_at` is UTC from SQLite, so compare against UTC now.
fn ago(updated_at: &str) -> String {
    let Ok(t) = NaiveDateTime::parse_from_str(updated_at, "%Y-%m-%d %H:%M:%S") else {
        return String::new();
    };
    let mins = (chrono::Utc::now().naive_utc() - t).num_minutes();
    match mins {
        ..1 => "just now".into(),
        1..60 => format!("{mins}m ago"),
        60..1440 => format!("{}h ago", mins / 60),
        _ => format!("{}d ago", mins / 1440),
    }
}

/// ~100 chars of body on one line, centred on the first hit of `q` when there is one.
fn snippet(body: &str, q: &str) -> (String, String, String) {
    let flat: Vec<char> = body
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect();
    let lower: String = flat.iter().collect::<String>().to_lowercase();
    let qn = q.chars().count();
    // Char index of the hit; ASCII lowercasing keeps indices aligned for the usual case.
    let at = (!q.is_empty())
        .then(|| lower.find(&q.to_lowercase()))
        .flatten()
        .map(|b| lower[..b].chars().count())
        .filter(|&i| i + qn <= flat.len());
    let s = |a: usize, b: usize| flat[a..b].iter().collect::<String>();
    match at {
        Some(i) => {
            let from = i.saturating_sub(40);
            let to = (i + qn + 60).min(flat.len());
            let lead = if from > 0 { "…" } else { "" };
            let tail = if to < flat.len() { "…" } else { "" };
            (
                format!("{lead}{}", s(from, i)),
                s(i, i + qn),
                format!("{}{tail}", s(i + qn, to)),
            )
        }
        None => {
            let to = flat.len().min(100);
            let tail = if to < flat.len() { "…" } else { "" };
            (format!("{}{tail}", s(0, to)), String::new(), String::new())
        }
    }
}

fn side(rows: &[queries::NoteRow], on: i64) -> view::Side {
    let mut side = view::Side {
        mine: Vec::new(),
        agents: Vec::new(),
        on,
    };
    for n in rows {
        let item = view::SideNote {
            id: n.id,
            title: title_or_untitled(&n.title),
        };
        if n.author == "me" {
            side.mine.push(item);
        } else {
            side.agents.push(item);
        }
    }
    side
}

/// Percent-encodes a query value (the search box) for the filter row's links.
fn encode(v: &str) -> String {
    v.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[derive(Deserialize)]
pub struct ListQuery {
    q: Option<String>,
    /// Tag filter, comma list (any of them); `add` = the filter row's add box.
    tag: Option<String>,
    add: Option<String>,
}

pub async fn list(State(st): State<AppState>, Query(lq): Query<ListQuery>) -> AppResult<Response> {
    let q = lq.q.unwrap_or_default().trim().to_string();
    let conn = st.conn();
    let all = queries::list(&conn, "", &[])?;
    let tags = TagFilter::pick(lq.tag.as_deref(), lq.add.as_deref());
    let (mut mine, mut agents) = (Vec::new(), Vec::new());
    for n in queries::list(&conn, &q, &tags)? {
        let (before, hit, after) = snippet(&markdown::to_text(&n.body), &q);
        let row = view::ListRow {
            id: n.id,
            title: title_or_untitled(&n.title),
            agent: agent(&n.author),
            before,
            hit,
            after,
            ago: ago(&n.updated_at),
        };
        if row.agent.is_none() {
            mine.push(row);
        } else {
            agents.push(row);
        }
    }
    web::html(&view::List {
        layout: Layout::load(&conn, "notes", "Notes", vec![])?,
        side: side(&all, 0),
        filter: TagFilter {
            tags,
            path: if q.is_empty() {
                "/notes?".into()
            } else {
                format!("/notes?q={}&", encode(&q))
            },
            keep: if q.is_empty() {
                Vec::new()
            } else {
                vec![("q", q.clone())]
            },
            known: queries::tag_names(&conn)?,
        },
        q,
        mine,
        agents,
    })
}

/// Plain POST either way: the new note's page is where you type next.
pub async fn create(State(st): State<AppState>) -> AppResult<Response> {
    let id = queries::create(&st.conn())?;
    Ok(web::back(&format!("/notes/{id}")))
}

fn resolver(conn: &Connection) -> rusqlite::Result<impl Fn(&str) -> Option<String>> {
    let (topics, meetings, tags) = queries::known_pages(conn)?;
    Ok(move |x: &str| {
        links::href(
            x,
            |s| topics.contains(s),
            |s| meetings.contains(s),
            |s| tags.contains(s),
        )
    })
}

pub async fn page(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Response> {
    let conn = st.conn();
    let n = queries::note(&conn, id)?.ok_or(AppError::NotFound)?;
    let resolve = resolver(&conn)?;
    let links = links::mentions(&n.body)
        .into_iter()
        .map(|x| view::Link {
            label: x.to_string(),
            href: resolve(x),
        })
        .collect();
    let title = title_or_untitled(&n.title);
    web::html(&view::Page {
        layout: Layout::load(&conn, "notes", title, vec![crumb("Notes", "/notes")])?,
        side: side(&queries::list(&conn, "", &[])?, id),
        html: markdown::to_html(&links::rewrite(&n.body, &resolve)),
        agent: agent(&n.author),
        ago: ago(&n.updated_at),
        id,
        title: n.title,
        body: n.body,
        links,
    })
}

/// Autosave: the editor form posts title + markdown; only the fields it carries are written.
pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(form): Form<HashMap<String, String>>,
) -> AppResult<Response> {
    let fields: Vec<(&str, &str)> = queries::EDITABLE
        .iter()
        .filter_map(|c| form.get(*c).map(|v| (*c, v.as_str())))
        .collect();
    if !queries::update_fields(&st.conn(), id, &fields)? {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/notes/{id}")));
    }
    web::html(&view::Saved)
}

#[derive(Deserialize)]
pub struct PagesQuery {
    q: Option<String>,
}

/// JSON for the editor's `[[` picker.
pub async fn pages(
    State(st): State<AppState>,
    Query(pq): Query<PagesQuery>,
) -> AppResult<Response> {
    let rows = queries::pages(&st.conn(), pq.q.as_deref().unwrap_or("").trim())?;
    let pages = rows
        .into_iter()
        .map(|r| view::PageItem {
            id: r.id,
            name: r.name,
            kind: r.kind,
        })
        .collect();
    Ok(Json(view::Pages {
        data: view::PagesData { pages },
    })
    .into_response())
}

/// Delete + Undo toast (htmx) or back to the list (plain form).
pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let (title, snap) = queries::delete(&st.conn(), id)?.ok_or(AppError::NotFound)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back("/notes"));
    }
    web::html(&web::Deleted {
        what: format!("Note “{}”", title_or_untitled(&title)),
        restore: "/notes/restore".into(),
        snap,
        back: "/notes",
        back_label: "All notes",
        error: None,
    })
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
    let id = queries::restore(&st.conn(), &f.snap)?;
    Ok(web::restored(&headers, &format!("/notes/{id}")))
}
