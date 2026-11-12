//! Meeting mode: start/end (one at a time), `/meetings` by series, `/meetings/{id}` with its lines and notes,
//! and the recordings the Mac app transcribes into it (T53).
mod handlers;
mod queries;
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
        .route("/meetings/banner", get(handlers::banner_get))
        .route("/meetings/{id}/record", post(handlers::record))
        .route("/meetings/{id}/record/stop", post(handlers::stop))
        .route("/meetings/{id}/transcript", get(handlers::transcript))
        .route(
            "/meetings/transcripts/{tid}/segments",
            post(handlers::segments),
        )
        .route("/meetings/{id}/notes", post(handlers::save_notes))
        .route("/meetings/{id}/tags", post(handlers::set_tags))
        .route("/meetings/{id}/delete", post(handlers::delete))
        .route("/meetings/restore", post(handlers::restore))
}
