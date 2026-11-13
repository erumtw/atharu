use super::{queries, view};
use crate::{
    app::AppState,
    domain::{
        dates, depth,
        entry::{LineRow, is_slug},
    },
    error::{AppError, AppResult},
    layout::{Layout, crumb},
    web,
};
use axum::{
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashMap;

fn short(d: NaiveDate) -> String {
    d.format("%-d %b").to_string()
}

fn depth_view(d: Option<u8>) -> view::Depth {
    match d {
        Some(d) => view::Depth {
            depth: d,
            label: depth::label(d),
        },
        None => view::Depth {
            depth: 0,
            label: "not rated yet",
        },
    }
}

pub async fn list(State(st): State<AppState>) -> AppResult<Response> {
    let today = st.today();
    let conn = st.conn();
    let rows = queries::topics(&conn)?
        .into_iter()
        .map(|t| {
            let stale = match t.last_rated {
                None => "not rated yet".to_string(),
                Some(d) if depth::is_stale(Some(d), today) => {
                    format!("{} days ago · re-rate", (today - d).num_days())
                }
                Some(_) => String::new(),
            };
            view::ListRow {
                slug: t.slug,
                depth: depth_view(t.depth),
                last_rated: t.last_rated.map(short).unwrap_or_default(),
                stale,
                lines: t.lines,
            }
        })
        .collect();
    web::html(&view::List {
        layout: Layout::load(&conn, "learn", "Learn", vec![])?,
        side: queries::slugs(&conn)?,
        new_topics: queries::new_topics(&conn)?
            .into_iter()
            .map(|(slug, lines)| view::NewTopic { slug, lines })
            .collect(),
        untopic: queries::untopic_days(&conn)?,
        rows,
    })
}

#[derive(Deserialize)]
pub struct MakeForm {
    slug: String,
}

/// Plain POST either way: the new page is where you go next (write own words, rate).
pub async fn make_topic(
    State(st): State<AppState>,
    Form(f): Form<MakeForm>,
) -> AppResult<Response> {
    let slug = f.slug.trim();
    if !is_slug(slug) {
        return Ok(web::back("/learn"));
    }
    queries::make_topic(&st.conn(), slug)?;
    Ok(web::back(&format!("/learn/{slug}")))
}

fn rate_view(conn: &Connection, slug: &str, today: NaiveDate) -> rusqlite::Result<view::Rate> {
    let ratings = queries::ratings(conn, slug)?;
    let last = ratings.last();
    Ok(view::Rate {
        slug: slug.to_string(),
        today: last.filter(|r| r.day == today).map_or(0, |r| r.depth),
        scale: (1..=4).map(|d| depth_view(Some(d))).collect(),
        depth_now: view::DepthNow {
            now: depth_view(last.map(|r| r.depth)),
            rated: last
                .map(|r| format!("rated {}", short(r.day)))
                .unwrap_or_default(),
            history: ratings
                .iter()
                .map(|r| view::Hist {
                    day: short(r.day),
                    depth: depth_view(Some(r.depth)),
                })
                .collect(),
        },
    })
}

pub async fn page(State(st): State<AppState>, Path(slug): Path<String>) -> AppResult<Response> {
    let conn = st.conn();
    let t = queries::topic(&conn, &slug)?.ok_or(AppError::NotFound)?;
    let lines = queries::lines(&conn, &slug)?
        .into_iter()
        .map(|l| LineRow {
            when: NaiveDate::parse_from_str(&l.day, "%Y-%m-%d")
                .map(short)
                .unwrap_or_else(|_| l.day.clone()),
            ..l
        })
        .collect();
    web::html(&view::Page {
        layout: Layout::load(&conn, "learn", slug.clone(), vec![crumb("Learn", "/learn")])?,
        side: queries::slugs(&conn)?,
        rate: rate_view(&conn, &slug, st.today())?,
        summary: t.summary,
        own_words: t.own_words,
        resources: t.resources,
        refs: crate::layout::refs(&conn, Some(&slug), Some(crate::layout::Lines::Topic(&slug)))?,
        ref_name: slug.clone(),
        lines,
        field: tag_field(&conn, &slug)?,
        slug,
    })
}

/// Autosave: the page's two forms post on change; only the fields a form carries are written.
pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Form(form): Form<HashMap<String, String>>,
) -> AppResult<Response> {
    let fields: Vec<(&str, &str)> = queries::EDITABLE
        .iter()
        .filter_map(|c| form.get(*c).map(|v| (*c, v.trim())))
        .collect();
    if !queries::update_fields(&st.conn(), &slug, &fields)? {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/learn/{slug}")));
    }
    web::html(&view::Saved)
}

fn tag_field(conn: &rusqlite::Connection, slug: &str) -> rusqlite::Result<crate::layout::TagField> {
    Ok(crate::layout::TagField {
        id: "topic-tags",
        tags: crate::domain::tags::parse_list(&queries::tags(conn, slug)?),
        known: queries::tag_names(conn)?,
    })
}

#[derive(Deserialize)]
pub struct TagsForm {
    tags: Option<String>,
    tags_add: Option<String>,
    tags_drop: Option<String>,
}

/// The Tags prop: a chip edit saves at once and answers with the prop (htmx) or the page.
pub async fn set_tags(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Form(f): Form<TagsForm>,
) -> AppResult<Response> {
    let list = crate::domain::tags::edit_list(
        f.tags.as_deref().unwrap_or(""),
        f.tags_add.as_deref(),
        f.tags_drop.as_deref(),
    );
    let conn = st.conn();
    if !queries::set_tags(&conn, &slug, &list)? {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/learn/{slug}")));
    }
    web::html(&view::Tags {
        field: tag_field(&conn, &slug)?,
        slug,
    })
}

#[derive(Deserialize)]
pub struct RateForm {
    depth: u8,
}

fn rate_answer(st: &AppState, headers: &HeaderMap, slug: &str) -> AppResult<Response> {
    if !web::is_htmx(headers) {
        return Ok(web::back(&format!("/learn/{slug}")));
    }
    let rate = rate_view(&st.conn(), slug, st.today())?;
    web::html(&view::RateSwap { rate })
}

pub async fn rate(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Form(f): Form<RateForm>,
) -> AppResult<Response> {
    if !(1..=4).contains(&f.depth) {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, "Depth is 1 to 4.").into_response());
    }
    if queries::topic(&st.conn(), &slug)?.is_none() {
        return Err(AppError::NotFound);
    }
    queries::rate(&st.conn(), &slug, &dates::iso(st.today()), f.depth)?;
    rate_answer(&st, &headers, &slug)
}

/// Undo = drop today's rating; earlier days are history and stay.
pub async fn unrate(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    queries::unrate(&st.conn(), &slug, &dates::iso(st.today()))?;
    rate_answer(&st, &headers, &slug)
}

/// Delete + Undo toast; the topic's `?` lines stay and show in the Learn inbox again.
pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let snap = queries::delete(&mut st.conn(), &slug)?.ok_or(AppError::NotFound)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back("/learn"));
    }
    web::html(&deleted(&slug, snap, None))
}

fn deleted(slug: &str, snap: String, error: Option<String>) -> web::Deleted {
    web::Deleted {
        what: format!("Topic {slug}"),
        // Under the slug, so a topic named "restore" can't collide with a `/learn/{slug}` route.
        restore: format!("/learn/{slug}/restore"),
        snap,
        back: "/learn",
        back_label: "All topics",
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
        Ok(slug) => Ok(web::restored(&headers, &format!("/learn/{slug}"))),
        Err(slug) => web::html_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            &deleted(
                &slug,
                f.snap,
                Some(format!(
                    "There's a {slug} topic again, so the old one can't come back."
                )),
            ),
        ),
    }
}
