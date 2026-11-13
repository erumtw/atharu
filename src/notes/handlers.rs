use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, entry::Kind, links, markdown},
    error::{AppError, AppResult},
    layout::{Layout, TagFilter, crumb},
    web,
};
use axum::{
    Json,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{NaiveDate, NaiveDateTime};
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
    /// `1` = only notes linked to lines (D49).
    linked: Option<String>,
}

/// `/notes?…` from the filters that are set (empty ones left out).
fn list_url(q: &str, tags: &[String], linked: bool) -> String {
    let mut parts = Vec::new();
    if !q.is_empty() {
        parts.push(format!("q={}", encode(q)));
    }
    if !tags.is_empty() {
        parts.push(format!("tag={}", tags.join(",")));
    }
    if linked {
        parts.push("linked=1".into());
    }
    if parts.is_empty() {
        "/notes".into()
    } else {
        format!("/notes?{}", parts.join("&"))
    }
}

/// The list badge's "Linked to a … line": the word the capture bar's kind tiles use.
fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Plan => "to-do",
        k => k.tooltip(),
    }
}

pub async fn list(State(st): State<AppState>, Query(lq): Query<ListQuery>) -> AppResult<Response> {
    let q = lq.q.unwrap_or_default().trim().to_string();
    let conn = st.conn();
    let all = queries::list(&conn, "", &[], false)?;
    let tags = TagFilter::pick(lq.tag.as_deref(), lq.add.as_deref());
    let linked = lq.linked.as_deref() == Some("1");
    let (mut mine, mut agents) = (Vec::new(), Vec::new());
    for n in queries::list(&conn, &q, &tags, linked)? {
        let (before, hit, after) = snippet(&markdown::to_text(&n.body), &q);
        let row = view::ListRow {
            id: n.id,
            title: title_or_untitled(&n.title),
            agent: agent(&n.author),
            before,
            hit,
            after,
            ago: ago(&n.updated_at),
            kind: n.first_kind,
            kind_name: n.first_kind.map_or("", kind_name),
            lines: n.lines,
        };
        if row.agent.is_none() {
            mine.push(row);
        } else {
            agents.push(row);
        }
    }
    let mut keep = Vec::new();
    if !q.is_empty() {
        keep.push(("q", q.clone()));
    }
    if linked {
        keep.push(("linked", "1".to_string()));
    }
    let base = list_url(&q, &[], linked);
    web::html(&view::List {
        layout: Layout::load(&conn, "notes", "Notes", vec![])?,
        side: side(&all, 0),
        linked_on: list_url(&q, &tags, true),
        linked_off: list_url(&q, &tags, false),
        linked,
        filter: TagFilter {
            path: if base.contains('?') {
                format!("{base}&")
            } else {
                format!("{base}?")
            },
            tags,
            keep,
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

#[derive(Deserialize)]
pub struct TextForm {
    #[serde(default)]
    text: String,
    /// The popover's line box forwarded by `POST /entries` (307) when it starts `n ` (D55): the rest is the text.
    line: Option<String>,
    back: Option<String>,
}

/// One text box → the owner's note (the quick popover, D53). With `back` it answers there with `note=<id>` (the
/// popover's "✓ Saved note · Open"), else on the new note.
pub async fn create_text(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<TextForm>,
) -> AppResult<Response> {
    let back = f.back.as_deref().map(|b| web::safe_back(Some(b), "/notes"));
    let sep = |to: &str| if to.contains('?') { '&' } else { '?' };
    let text = f
        .line
        .as_deref()
        .and_then(markdown::quick_note)
        .unwrap_or(&f.text);
    let Some((title, body)) = markdown::split_note(text) else {
        if web::is_htmx(&headers) {
            // htmx keeps the text (it only resets on 2xx); only the hint changes, as for a rejected line.
            let retarget = [("hx-retarget", "#hint"), ("hx-reswap", "innerHTML")];
            let hint = view::HintError {
                error: markdown::NO_TITLE,
            };
            return Ok((
                retarget,
                web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &hint)?,
            )
                .into_response());
        }
        // Nothing but whitespace was typed, so dropping it loses nothing: back to the box, in note mode.
        let to = back.unwrap_or("/notes");
        return Ok(web::back(&format!("{to}{}mode=note&err=1", sep(to))));
    };
    let id = queries::insert(&st.conn(), title, body, "me")?;
    Ok(match back {
        Some(to) => web::back(&format!("{to}{}note={id}", sep(to))),
        None => web::back(&format!("/notes/{id}")),
    })
}

fn resolver(conn: &Connection) -> rusqlite::Result<impl Fn(&str) -> Option<String>> {
    let (epics, topics, meetings, tags) = queries::known_pages(conn)?;
    Ok(move |x: &str| {
        links::href(
            x,
            |s| epics.contains(s),
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
        side: side(&queries::list(&conn, "", &[], false)?, id),
        head: head(&st, &conn, id)?,
        html: markdown::to_html(&links::rewrite(&n.body, &resolve)),
        agent: agent(&n.author),
        meeting: queries::meeting_of(&conn, id)?.map(|(mid, name, day)| {
            let day = chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d")
                .map(crate::domain::dates::label)
                .unwrap_or(day);
            (mid, format!("{name} · {day}"))
        }),
        ago: ago(&n.updated_at),
        id,
        title: n.title,
        body: n.body,
        links,
    })
}

/// The header's lines: newest five shown, the rest folded; lines deleted in their Undo window first.
fn head(st: &AppState, conn: &Connection, id: i64) -> rusqlite::Result<view::Lines> {
    let today = st.today();
    let label = |d: &str| match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
        Ok(d) if d == today => "today".to_string(),
        Ok(d) => dates::label(d),
        Err(_) => d.to_string(),
    };
    let mut shown: Vec<view::HeadLine> = queries::lines(conn, id)?
        .into_iter()
        .map(|l| view::HeadLine {
            closed: match (l.plan_state.as_deref(), l.closed_day.as_deref()) {
                (Some(s @ ("done" | "dropped")), Some(d)) => format!("{s} {}", label(d)),
                (Some(s @ ("done" | "dropped")), None) => s.to_string(),
                _ => String::new(),
            },
            day_label: label(&l.day),
            id: l.id,
            kind: l.kind,
            topic: l.topic,
            text: l.text,
            epic_key: l.epic_key,
            plan_state: l.plan_state,
            due: l.due.unwrap_or_default(),
            day: l.day,
        })
        .collect();
    let live: Vec<i64> = shown.iter().map(|l| l.id).collect();
    let more = shown.split_off(shown.len().min(HEAD_LINES));
    Ok(view::Lines {
        id,
        // A line restored under its old id is listed live again; don't show it twice.
        gone: st
            .gone_lines(id)
            .into_iter()
            .filter(|g| !live.contains(&g.line))
            .map(|g| (g.text, g.snap))
            .collect(),
        shown,
        more,
    })
}

/// Header lines shown before "N more lines".
const HEAD_LINES: usize = 5;

/// The header alone: re-fetched when a to-do in it is ticked or re-dated.
pub async fn lines(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Response> {
    let conn = st.conn();
    queries::note(&conn, id)?.ok_or(AppError::NotFound)?;
    web::html(&head(&st, &conn, id)?)
}

/// A line's note button (D49): its note, made first if it has none. htmx follows HX-Redirect,
/// a plain form the 303; either way you land on the note, where you type next.
pub async fn for_line(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let note = queries::note_for_line(&st.conn(), id)?.ok_or(AppError::NotFound)?;
    Ok(web::restored(&headers, &format!("/notes/{note}")))
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
