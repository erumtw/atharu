//! Meeting mode: start/end (one at a time), `/meetings` by series, `/meetings/{id}` with its lines and notes.
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
        .route("/meetings/{id}/notes", post(handlers::save_notes))
}
