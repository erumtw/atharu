//! `/settings`: the look picker (spec "Looks"), the glass controls (D17) and the launch animation + its `/launch` page.
//! Stored in the `settings` table, read by the layout.
mod handlers;
pub(crate) mod queries;
mod view;

use crate::app::AppState;
use axum::{
    Router,
    routing::{get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(handlers::page).post(handlers::save))
        .route("/settings/glass", post(handlers::save_glass))
        .route("/settings/launch", post(handlers::save_launch))
        .route("/launch", get(handlers::launch))
        .route("/settings/transcription", get(handlers::transcription_get))
        .route("/settings/transcription/model", post(handlers::model))
        .route("/settings/transcription/lang", post(handlers::lang))
        .route("/settings/transcription/people", post(handlers::people))
        .route(
            "/settings/maccal",
            get(handlers::mac_cal_get).post(handlers::save_mac_cal),
        )
        .route("/settings/db", post(handlers::save_db))
        .route("/settings/db/note", post(handlers::db_note_seen))
}
