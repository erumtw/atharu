//! Meeting mode: start/end (one at a time), `/meetings` by series, `/meetings/{id}` with its lines and notes,
//! and the recordings the Mac app transcribes into it (T53).
mod handlers;
pub(crate) mod queries;
pub mod shots;
mod view;

use crate::app::AppState;
use axum::{
    Router,
    routing::{get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/meetings", get(handlers::list).post(handlers::start))
        .route("/meetings/{id}", get(handlers::page))
        .route("/meetings/{id}/end", post(handlers::end))
        .route("/meetings/{id}/rename", post(handlers::rename))
        .route("/meetings/banner", get(handlers::banner_get))
        .route("/meetings/permissions", post(handlers::permissions))
        .route("/meetings/{id}/record", post(handlers::record))
        .route("/meetings/{id}/record/stop", post(handlers::stop))
        .route("/meetings/{id}/transcript", get(handlers::transcript))
        .route(
            "/meetings/transcripts/{tid}/segments",
            post(handlers::segments),
        )
        .route(
            "/meetings/{id}/transcripts/{tid}/speakers",
            post(handlers::rename_speaker),
        )
        .route("/meetings/{id}/notes", post(handlers::save_notes))
        .route("/meetings/{id}/note", post(handlers::start_note))
        .route("/meetings/{id}/tags", post(handlers::set_tags))
        .route("/meetings/{id}/delete", post(handlers::delete))
        .route("/meetings/restore", post(handlers::restore))
        .route("/meetings/{id}/shots", get(handlers::shots))
        .route(
            "/attachments/meetings/{id}/{file}",
            get(handlers::shot_file),
        )
}

/// The Mac app's screenshot button (T89): store a PNG it captured for meeting `id`. A plain function, not a
/// route: the app holds the state, and an HTTP upload would only add a way in for any local page.
pub fn add_shot(st: &AppState, id: i64, png: &[u8]) -> Result<String, String> {
    let conn = st.conn();
    queries::meeting(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or("No such meeting.")?;
    let root = shots::root(&conn).ok_or("No folder next to the database.")?;
    let now = chrono::Local::now().format("%H%M%S").to_string();
    shots::save(&root, id, png, &now)
}
