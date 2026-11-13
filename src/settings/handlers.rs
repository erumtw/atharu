use super::{queries, view};
use crate::{
    app::AppState,
    build_info, config,
    domain::look::{self, LOOKS},
    error::{AppError, AppResult},
    layout::Layout,
    recording::{self, Mac, Model},
    updater::{self, UpdateState},
    web,
};
use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

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
        // A stored failure only speaks while Jira is set up: removing the token from the config silences it.
        jira_dead: st.settings.jira().is_some()
            && queries::get(conn, "jira_auth_failed")?.is_some(),
        token_url: crate::config::JIRA_TOKEN_URL,
        version: build_info::TAG.map_or_else(
            || format!("{} · dev build", build_info::VERSION),
            str::to_string,
        ),
        sha: build_info::SHA,
        built: build_info::BUILT,
        db,
        tr: transcription(st, conn)?,
        mc: mac_cal(st, conn)?,
        upd: update_part(st, conn),
        launch: look::launch(queries::get(conn, "launch")?.as_deref()),
    })
}

fn mac_cal(st: &AppState, conn: &rusqlite::Connection) -> AppResult<view::MacCal> {
    let chosen = queries::get(conn, "mac_cals")?.unwrap_or_default();
    let chosen: Vec<&str> = chosen.split(',').filter(|c| !c.is_empty()).collect();
    let mc = st.mac_cals.as_deref();
    Ok(view::MacCal {
        app: mc.is_some(),
        on: queries::get(conn, "mac_cal")?.as_deref() == Some("1"),
        access: mc.map_or(recording::Access::Unknown, |m| m.access()),
        cals: mc
            .map_or_else(Vec::new, |m| m.list())
            .into_iter()
            .map(|mut c| {
                // It lands in a style attribute: only `#rrggbb` gets there.
                let hex = c.color.len() == 7
                    && c.color.starts_with('#')
                    && c.color[1..].chars().all(|h| h.is_ascii_hexdigit());
                if !hex {
                    c.color = "#8e8e93".into();
                }
                let on = chosen.contains(&c.id.as_str());
                (c, on)
            })
            .collect(),
        synced: queries::get(conn, "mac_cal_synced")?
            .map(|t| t.get(..16).unwrap_or(&t).to_string()),
    })
}

/// The row alone: its poll while the app waits for macOS or the first sync.
pub async fn mac_cal_get(State(st): State<AppState>) -> AppResult<Response> {
    let mc = mac_cal(&st, &st.conn())?;
    web::html(&view::MacCalRow { mc })
}

/// The Mac calendars form autosaves on every change: `on` = the switch, one `cal` per checked calendar.
/// The app then asks macOS for access if it hasn't, and syncs at once.
pub async fn save_mac_cal(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> AppResult<Response> {
    let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(&body).unwrap_or_default();
    let on = pairs.iter().any(|(k, v)| k == "on" && v == "1");
    let cals: Vec<&str> = pairs
        .iter()
        .filter(|(k, v)| k == "cal" && !v.contains(',') && !v.is_empty())
        .map(|(_, v)| v.as_str())
        .collect();
    {
        let conn = st.conn();
        queries::set(&conn, "mac_cal", if on { "1" } else { "0" })?;
        if on {
            // Only the app lists the calendars: a browser post (no boxes shown) leaves the choice alone.
            if st.mac_cals.is_some() {
                queries::set(&conn, "mac_cals", &cals.join(","))?;
            }
        } else {
            // Off (D7): the synced events go, the ticked calendars stay for when it's on again.
            queries::clear_mac_events(&conn)?;
        }
    }
    if let Some(m) = &st.mac_cals {
        m.kick.notify_one();
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back("/settings#maccal"));
    }
    let mc = mac_cal(&st, &st.conn())?;
    web::html(&view::MacCalRow { mc })
}

fn transcription(st: &AppState, conn: &rusqlite::Connection) -> AppResult<view::Transcription> {
    let mac = st.mac.as_deref();
    let model = mac.map_or(Model::Missing, Mac::model);
    Ok(view::Transcription {
        mac: mac.is_some(),
        polling: matches!(model, Model::Downloading { .. } | Model::Checking),
        model,
        langs: &recording::LANGS,
        lang: recording::lang(conn)?,
        hint: queries::first_epic(conn)?.unwrap_or_default(),
        perms: mac.is_some_and(|m| {
            let p = m.perms.lock().unwrap_or_else(|e| e.into_inner());
            p.0 && p.1
        }),
        people: queries::get(conn, "people")?.unwrap_or_default(),
    })
}

fn tr_answer(st: &AppState, headers: &HeaderMap) -> AppResult<Response> {
    if !web::is_htmx(headers) {
        return Ok(web::back("/settings#transcription"));
    }
    web::html(&transcription(st, &st.conn())?)
}

/// The group alone, for the poll while downloading.
pub async fn transcription_get(State(st): State<AppState>) -> AppResult<Response> {
    web::html(&transcription(&st, &st.conn())?)
}

#[derive(Deserialize)]
pub struct ModelForm {
    act: String,
}

/// Download / Cancel (deletes the partial) / Delete the speech model. A browser has no model to manage.
pub async fn model(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<ModelForm>,
) -> AppResult<Response> {
    let Some(mac) = &st.mac else {
        return Err(AppError::NotFound);
    };
    match f.act.as_str() {
        "download" => mac.download(),
        "cancel" => mac.cancel(),
        "delete" => mac.delete(),
        _ => {
            return Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Pick download, cancel or delete.",
            )
                .into_response());
        }
    }
    tr_answer(&st, &headers)
}

#[derive(Deserialize)]
pub struct PeopleForm {
    people: String,
}

/// Names list autosave (no Save button): htmx gets 204, a plain post goes back.
pub async fn people(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<PeopleForm>,
) -> AppResult<Response> {
    // 40 people at ~100 chars each; more would only be cut from the prompt anyway.
    let text: String = f.people.chars().take(4000).collect();
    queries::set(&st.conn(), "people", &text)?;
    Ok(if web::is_htmx(&headers) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        web::back("/settings#transcription")
    })
}

#[derive(Deserialize)]
pub struct LangForm {
    lang: String,
}

pub async fn lang(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<LangForm>,
) -> AppResult<Response> {
    if !recording::LANGS.iter().any(|(v, _)| *v == f.lang) {
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Pick Auto, Thai or English.",
        )
            .into_response());
    }
    queries::set(&st.conn(), "transcribe_lang", &f.lang)?;
    tr_answer(&st, &headers)
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

/// The Mac app's first page: the launch animation, then it replaces itself with Today. `off` skips straight there.
pub async fn launch(State(st): State<AppState>) -> AppResult<Response> {
    let conn = st.conn();
    let variant = look::launch(queries::get(&conn, "launch")?.as_deref());
    if variant == "off" {
        return Ok(web::back("/"));
    }
    web::html(&view::Launch {
        layout: Layout::load(&conn, "today", "Atharu", vec![])?,
        variant,
    })
}

#[derive(Deserialize)]
pub struct LaunchForm {
    launch: String,
}

/// htmx saves on click (204; the preview is already playing); a plain form POST gets a 303 back.
pub async fn save_launch(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<LaunchForm>,
) -> AppResult<Response> {
    if !look::LAUNCH.contains(&f.launch.as_str()) {
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Pick Ink, Liquid Glass or Off.",
        )
            .into_response());
    }
    queries::set(&st.conn(), "launch", &f.launch)?;
    Ok(if web::is_htmx(&headers) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        web::back("/settings")
    })
}

// ---- About › Update, the Mac app's button (0.2.6) ------------------------------------------------------------------

/// `update.log` next to the open DB (a temp file for an in-memory one, i.e. in tests).
fn update_log(conn: &rusqlite::Connection) -> PathBuf {
    let file = queries::db_file(conn).unwrap_or_default();
    Path::new(&file)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(std::env::temp_dir, Path::to_path_buf)
        .join("update.log")
}

fn update_part(st: &AppState, conn: &rusqlite::Connection) -> view::Update {
    let state = updater::snapshot(&st.update);
    let log = update_log(conn);
    // The progress line is the log's tail, read when the row polls (the build writes the file itself, no pipe).
    let last = match state {
        UpdateState::Running { .. } => updater::tail(&log),
        _ => String::new(),
    };
    view::Update::new(&state, log.display().to_string(), last)
}

fn update_html(st: &AppState) -> AppResult<Response> {
    let part = update_part(st, &st.conn());
    web::html(&part)
}

/// The partial, or (a plain form) back to the row: every POST here works without JS.
fn update_answer(st: &AppState, headers: &HeaderMap) -> AppResult<Response> {
    if web::is_htmx(headers) {
        update_html(st)
    } else {
        Ok(web::back("/settings#update"))
    }
}

/// The row alone: its poll while an update runs.
pub async fn update_get(State(st): State<AppState>) -> AppResult<Response> {
    update_html(&st)
}

/// "Check for updates": fetch the tags (a blocking git call, off the runtime) and say whether a newer release exists.
pub async fn update_check(State(st): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if !matches!(updater::snapshot(&st.update), UpdateState::Running { .. }) {
        let repo = updater::repo();
        let found = tokio::task::spawn_blocking(move || updater::newest(Path::new(&repo))).await;
        let next = match found {
            Ok(Ok(Some(tag))) if updater::is_newer(updater::installed(), &tag) => {
                UpdateState::Available(tag)
            }
            Ok(Ok(Some(tag))) => UpdateState::UpToDate(tag),
            Ok(Ok(None)) => UpdateState::Failed {
                to: String::new(),
                why: "No release tags yet: there is nothing to install.".into(),
            },
            Ok(Err(why)) => UpdateState::Failed {
                to: String::new(),
                why,
            },
            Err(_) => UpdateState::Failed {
                to: String::new(),
                why: "The check didn't finish.".into(),
            },
        };
        let mut s = updater::lock(&st.update);
        // A run that started while the check ran keeps its state.
        if !matches!(*s, UpdateState::Running { .. }) {
            *s = next;
        }
    }
    update_answer(&st, &headers)
}

/// "Update": install the tag the last check found, only from Atharu.app (it can restart itself), one at a time. Nothing in the
/// request picks the tag or the command: it is the state's own.
pub async fn update_start(State(st): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let app = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ua| ua.contains("AtharuMac"));
    if !app {
        return Ok((StatusCode::FORBIDDEN, "Update from Terminal: atharu update").into_response());
    }
    if let UpdateState::Available(tag) = updater::snapshot(&st.update) {
        let log = update_log(&st.conn());
        updater::start(&st.update, PathBuf::from(updater::repo()), tag, log);
    }
    update_answer(&st, &headers)
}
