use super::{queries, view};
use crate::{
    app::AppState,
    build_info, config,
    domain::look::{self, LOOKS},
    error::AppResult,
    layout::Layout,
    web,
};
use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::path::PathBuf;

pub async fn page(State(st): State<AppState>) -> AppResult<Response> {
    let conn = st.conn();
    let db = db_row(&conn, None, None)?;
    web::html(&full(&st, &conn, db)?)
}

fn full(st: &AppState, conn: &rusqlite::Connection, db: view::Db) -> AppResult<view::Page> {
    let layout = Layout::load(conn, "settings", "Settings", vec![])?;
    Ok(view::Page {
        cur: look::pick(Some(layout.look)),
        layout,
        looks: &LOOKS,
        google: st.settings.ical_url.is_some(),
        jira: st.settings.jira().is_some(),
        version: build_info::TAG.map_or_else(
            || format!("{} · dev build", build_info::VERSION),
            str::to_string,
        ),
        sha: build_info::SHA,
        built: build_info::BUILT,
        db,
    })
}

/// Settings key set once the "old Mac DB isn't merged" note is dismissed.
const DB_NOTE_SEEN: &str = "db_note_seen";

fn pointer() -> Option<PathBuf> {
    std::env::var_os(config::DB_POINTER_ENV).map(PathBuf::from)
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The Database file rows; `input` + `error` carry a rejected path back into the field.
fn db_row(
    conn: &rusqlite::Connection,
    input: Option<String>,
    error: Option<String>,
) -> AppResult<view::Db> {
    let path = queries::db_file(conn)?;
    let path = if path.is_empty() {
        "In memory".into()
    } else {
        path
    };
    let pointer = pointer();
    let next = pointer
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(view::Db {
        change: pointer.is_some(),
        pending: next.as_ref().is_some_and(|n| *n != path),
        note: pointer.is_some() && queries::get(conn, DB_NOTE_SEEN)?.is_none(),
        input: input.or(next).unwrap_or_else(|| path.clone()),
        path,
        error,
    })
}

#[derive(Deserialize)]
pub struct DbForm {
    path: String,
}

/// Mac app only (the pointer env): check the path, write the pointer, show "Restart Atharu to switch".
/// The open connection stays: switching under live requests is what the restart avoids.
pub async fn save_db(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<DbForm>,
) -> AppResult<Response> {
    let Some(ptr) = pointer() else {
        return Ok((
            StatusCode::NOT_FOUND,
            "Only the Mac app can change its database.",
        )
            .into_response());
    };
    let checked = config::check_db_path(&f.path, home().as_deref()).and_then(|db| {
        config::write_db_pointer(&ptr, &db).map_err(|e| format!("Couldn't save the path: {e}."))
    });
    let conn = st.conn();
    let Err(e) = checked else {
        return if web::is_htmx(&headers) {
            web::html(&db_row(&conn, None, None)?)
        } else {
            Ok(web::back("/settings"))
        };
    };
    let row = db_row(&conn, Some(f.path), Some(e))?;
    if web::is_htmx(&headers) {
        return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &row);
    }
    web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &full(&st, &conn, row)?)
}

/// "Got it" on the one-time note: htmx drops the row (empty 200), a plain form goes back.
pub async fn db_note_seen(State(st): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    queries::set(&st.conn(), DB_NOTE_SEEN, "1")?;
    Ok(if web::is_htmx(&headers) {
        "".into_response()
    } else {
        web::back("/settings")
    })
}

#[derive(Deserialize)]
pub struct LookForm {
    look: String,
}

/// Plain form POST, 303 back: the look lives on `<html>`, so the whole page has to reload anyway
/// (an htmx partial couldn't repaint the wallpaper or favicon).
pub async fn save(State(st): State<AppState>, Form(f): Form<LookForm>) -> AppResult<Response> {
    if !LOOKS.iter().any(|l| l.slug == f.look) {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, "Pick one of the looks.").into_response());
    }
    queries::set(&st.conn(), "look", &f.look)?;
    Ok(web::back("/settings"))
}

#[derive(Deserialize)]
pub struct GlassForm {
    /// Checkbox: present = on.
    glass_all: Option<String>,
    glass_opacity: Option<u8>,
    text_blur: Option<String>,
    /// Which preset button submitted the form, if one did.
    preset: Option<String>,
}

/// htmx saves on change (204, the page already previewed it); a plain form POST gets a 303 back.
/// A preset sets tint + alpha together; the slider alone only moves alpha.
pub async fn save_glass(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<GlassForm>,
) -> AppResult<Response> {
    let conn = st.conn();
    let floor = look::pick(queries::get(&conn, "look")?.as_deref()).glass_floor;
    let (tint, alpha) = match f.preset.as_deref() {
        Some("clear") => (Some("clear"), floor),
        Some("tinted") => (Some("tinted"), look::GLASS_TINTED.max(floor)),
        Some(_) => {
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, "Pick Clear or Tinted.").into_response());
        }
        None => (
            None,
            f.glass_opacity.unwrap_or(look::GLASS_DEFAULT).min(100),
        ),
    };
    queries::set(
        &conn,
        "glass_all",
        if f.glass_all.is_some() { "1" } else { "0" },
    )?;
    queries::set(
        &conn,
        "text_blur",
        if f.text_blur.is_some() { "1" } else { "0" },
    )?;
    queries::set(&conn, "glass_opacity", &alpha.to_string())?;
    if let Some(t) = tint {
        queries::set(&conn, "glass_tint", t)?;
    }
    Ok(if web::is_htmx(&headers) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        web::back("/settings")
    })
}
