use super::{queries, view};
use crate::{
    app::AppState,
    domain::{
        dates,
        entry::Kind,
        tags::{self, parse_list},
    },
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
use serde::Deserialize;

/// Recent lines shown before the fold.
const RECENT: usize = 8;

#[derive(Deserialize)]
pub struct ListQuery {
    q: Option<String>,
    sort: Option<String>,
}

pub async fn list(State(st): State<AppState>, Query(lq): Query<ListQuery>) -> AppResult<Response> {
    let conn = st.conn();
    let mut rows = queries::all(&conn)?;
    let total = rows.len();
    let q =
        lq.q.unwrap_or_default()
            .trim()
            .trim_start_matches('#')
            .to_lowercase();
    rows.retain(|r| r.tag.contains(&q));
    let sort = lq
        .sort
        .as_deref()
        .filter(|s| matches!(*s, "recent" | "name"))
        .unwrap_or("used");
    match sort {
        "recent" => rows.sort_by(|a, b| b.last_used.cmp(&a.last_used).then(a.tag.cmp(&b.tag))),
        "name" => rows.sort_by(|a, b| a.tag.cmp(&b.tag)),
        _ => {}
    }
    web::html(&view::List {
        layout: Layout::load(&conn, "tags", "Tags", vec![])?,
        rows,
        total,
        q,
        sort: sort.into(),
        today: st.today(),
    })
}

/// `today`, else `Wed 11` (the hub's short day).
fn when(day: &str, today: NaiveDate) -> String {
    match NaiveDate::parse_from_str(day, "%Y-%m-%d") {
        Ok(d) if d == today => "today".into(),
        Ok(d) => d.format("%a %-d").to_string(),
        Err(_) => day.into(),
    }
}

#[derive(Deserialize, Default)]
pub struct HubQuery {
    /// `rename` · `merge` · `delete`: the ⋯ menu's confirm step, shown above the page.
    act: Option<String>,
}

fn n(k: usize, one: &str, many: &str) -> String {
    format!("{k} {}", if k == 1 { one } else { many })
}

/// `Fri 13 · 14:00` from a stored event start (`YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`).
fn event_when(at: &str) -> String {
    let day = at
        .get(..10)
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .map(|d| d.format("%a %-d").to_string())
        .unwrap_or_else(|| at.into());
    match at.get(11..16) {
        Some(t) => format!("{day} · {t}"),
        None => day,
    }
}

/// How far ahead "Coming up" looks.
const AHEAD: u64 = 14;

fn render_hub(
    conn: &rusqlite::Connection,
    tag: String,
    today: NaiveDate,
    act: Option<String>,
    error: Option<String>,
) -> AppResult<view::Hub> {
    let topics = queries::topics(conn, &tag)?;
    let series = queries::series(conn, &tag)?;
    // One name, one hub: a same-named topic / series brings its own lines (D29).
    let mut all = queries::lines(conn, &tag)?;
    for l in queries::merged_lines(conn, &tag)? {
        if !all.iter().any(|a| a.id == l.id) {
            all.push(l);
        }
    }
    all.sort_by(|a, b| b.day.cmp(&a.day).then(b.id.cmp(&a.id)));
    let last_line = all.first().map(|l| l.day.clone());
    let total = all.len();
    let (mut todos, mut hi, mut recent) = (Vec::new(), Vec::new(), Vec::new());
    for l in all {
        let due = l
            .due
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        let open = l.plan_state.as_deref() == Some("open");
        let w = when(&l.day, today);
        let line = view::Line {
            id: l.id,
            kind: l.kind,
            topic: l.topic,
            text: l.text,
            when: match l.note {
                Some(n) => format!("{w} · {n}"),
                None => w,
            },
            via: l.via,
            via_epic: l.via_epic,
            author: l.author,
            due: due.map(dates::label),
            late: open && due.is_some_and(|d| d < today),
        };
        match l.kind {
            Kind::Plan if open => todos.push(line),
            Kind::Decision | Kind::Blocker | Kind::Impact => hi.push(line),
            _ => recent.push(line),
        }
    }
    let older = recent.split_off(recent.len().min(RECENT));
    let notes_raw = queries::notes(conn, &tag)?;
    let last_note = notes_raw.iter().map(|x| x.3.clone()).max();
    let notes = notes_raw
        .into_iter()
        .map(|(id, title, author, day)| (id, title, author, when(&day, today)))
        .collect::<Vec<_>>();
    let same_series = series.iter().find(|s| s.0 == tag).cloned();
    let mut used = Vec::new();
    if total > 0 {
        used.push(n(total, "line", "lines"));
    }
    if !todos.is_empty() {
        used.push(n(todos.len(), "open to-do", "open to-dos"));
    }
    if !notes.is_empty() {
        used.push(n(notes.len(), "note", "notes"));
    }
    if let Some(s) = &same_series {
        used.push(n(s.1 as usize, "meeting", "meetings"));
    }
    if let Some(d) = last_line.max(last_note) {
        used.push(format!("last used {}", when(&d, today)));
    }
    let from = dates::iso(today);
    let to = dates::iso(today + chrono::Days::new(AHEAD));
    let coming = queries::upcoming(conn, &tag, &from, &to)?
        .into_iter()
        // Google events are never tagged: a title word naming the tag is enough to list them.
        .filter(|(_, title, own)| {
            *own || title
                .split_whitespace()
                .any(|w| tags::same_word(w.trim_matches(|c: char| !c.is_alphanumeric()), &tag))
        })
        .map(|(at, title, own)| (event_when(&at), title, own))
        .collect();
    let mut meetings: Vec<view::HubMeeting> = series
        .iter()
        .map(|(name, k, day, tags_it)| view::HubMeeting {
            name: name.clone(),
            text: format!(
                "series · {}{}, last {}",
                if *tags_it { "tags it · " } else { "" },
                n(*k as usize, "meeting", "meetings"),
                NaiveDate::parse_from_str(day, "%Y-%m-%d")
                    .map(dates::label)
                    .unwrap_or(day.clone())
            ),
        })
        .collect();
    for (_, name, day, k) in queries::meetings(conn, &tag)? {
        let d = NaiveDate::parse_from_str(&day, "%Y-%m-%d")
            .map(dates::label)
            .unwrap_or(day);
        meetings.push(view::HubMeeting {
            name,
            text: format!(
                "{d} · {} tagged in the notes",
                n(k as usize, "line", "lines")
            ),
        });
    }
    let topic = topics
        .iter()
        .find(|t| t.slug == tag)
        .map(|t| view::SameTopic {
            depth: t.depth,
            own_words: t.own_words.clone(),
        });
    let title = format!("#{tag}");
    Ok(view::Hub {
        layout: Layout::load(conn, "tags", title, vec![crumb("Tags", "/tags")])?,
        used: used.join(" · "),
        tagged_by: queries::tagged_by(conn, &tag)?,
        todos,
        hi,
        recent,
        older,
        epics: queries::epics(conn, &tag)?,
        notes,
        meetings,
        topics,
        topic,
        series: same_series.map(|s| {
            let d = NaiveDate::parse_from_str(&s.2, "%Y-%m-%d")
                .map(dates::label)
                .unwrap_or(s.2);
            format!("{} · last {d}", n(s.1 as usize, "meeting", "meetings"))
        }),
        coming,
        act: act.filter(|a| matches!(a.as_str(), "rename" | "merge" | "delete")),
        error,
        known: queries::all(conn)?
            .into_iter()
            .map(|r| r.tag)
            .filter(|t| *t != tag)
            .collect(),
        tag,
    })
}

/// Only a well-formed, folded name has a hub; `/tags/Expense` or `/tags/a_b` is not a page.
fn check(tag: &str) -> AppResult<()> {
    if parse_list(tag) != [tag] {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn hub(
    State(st): State<AppState>,
    Path(tag): Path<String>,
    Query(q): Query<HubQuery>,
) -> AppResult<Response> {
    check(&tag)?;
    web::html(&render_hub(&st.conn(), tag, st.today(), q.act, None)?)
}

#[derive(Deserialize)]
pub struct RenameForm {
    to: String,
}

/// Rename, or merge when the new name is already a tag: `#old` → `#new` in every line, note,
/// own event and tags field, one transaction (queries::rewrite).
pub async fn rename(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(tag): Path<String>,
    Form(f): Form<RenameForm>,
) -> AppResult<Response> {
    check(&tag)?;
    let to = parse_list(&f.to);
    let mut conn = st.conn();
    let merge = queries::one(&conn, to.first().map_or("", String::as_str))?.is_some();
    let act = if merge { "merge" } else { "rename" };
    let new = match to.as_slice() {
        [x] if *x != tag => x.clone(),
        _ => {
            let msg = if to.len() > 1 {
                "One tag name: letters, digits and dashes."
            } else if to.first() == Some(&tag) {
                "That is already its name."
            } else {
                "Type a tag name: letters, digits and dashes."
            };
            let hub = render_hub(&conn, tag, st.today(), Some(act.into()), Some(msg.into()))?;
            return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &hub);
        }
    };
    let (k, snap) = queries::rewrite(&mut conn, &tag, Some(&new))?;
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/tags/{new}")));
    }
    let what = if merge {
        format!("Merged #{tag} into #{new}")
    } else {
        format!("Renamed #{tag} to #{new}")
    };
    web::html(&view::Done {
        what,
        changed: k,
        snap,
        back: format!("/tags/{tag}"),
        open: Some(new),
    })
}

/// Delete keeps every word: only the `#` goes (and the name leaves the tags fields).
pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(tag): Path<String>,
) -> AppResult<Response> {
    check(&tag)?;
    let (k, snap) = queries::rewrite(&mut st.conn(), &tag, None)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back("/tags"));
    }
    web::html(&view::Done {
        what: format!("Deleted #{tag}"),
        changed: k,
        snap,
        back: format!("/tags/{tag}"),
        open: None,
    })
}

#[derive(Deserialize)]
pub struct UndoForm {
    snap: String,
    back: Option<String>,
    /// Set by a toast that stays where it is (Today's capture bar): answer with the toast, not a redirect.
    inplace: Option<String>,
}

pub async fn undo(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<UndoForm>,
) -> AppResult<Response> {
    let k = queries::undo(&mut st.conn(), &f.snap)?;
    let back = web::safe_back(f.back.as_deref(), "/tags").to_string();
    if f.inplace.is_some() && web::is_htmx(&headers) {
        let t = view::Toast {
            text: format!("Undone · {}", n(k, "line", "lines")),
            tag: None,
            snap: None,
            back,
        };
        return Ok(([("hx-trigger", "tagged")], web::html(&t)?).into_response());
    }
    Ok(web::restored(&headers, &back))
}

#[derive(Deserialize)]
pub struct OlderForm {
    /// The line just tagged by hand (already done).
    except: i64,
    back: Option<String>,
}

/// "Tag them" (Make tag's toast): the word becomes `#tag` in every other line naming it; one Undo.
pub async fn older(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(tag): Path<String>,
    Form(f): Form<OlderForm>,
) -> AppResult<Response> {
    check(&tag)?;
    let (k, snap) = queries::tag_older(&mut st.conn(), &tag, f.except)?;
    let back = web::safe_back(f.back.as_deref(), "/").to_string();
    if !web::is_htmx(&headers) {
        return Ok(web::back(&back));
    }
    let t = view::Toast {
        text: format!("Tagged {}", n(k, "older line", "older lines")),
        tag: Some(tag),
        snap: Some(snap),
        back,
    };
    // Today's log listens for `tagged` and refetches itself: some of those lines may be on screen.
    Ok(([("hx-trigger", "tagged")], web::html(&t)?).into_response())
}
