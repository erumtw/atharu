//! `/` Today: capture a line, see the day's log grouped by epic, delete/undo, live parse hint.
mod handlers;
// The agent CLI files lines through the same queries (one validation path).
pub(crate) mod queries;
mod view;

use crate::app::AppState;
use axum::{
    Router,
    routing::{delete, get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::page))
        .route("/entries", post(handlers::create))
        .route(
            "/entries/{id}",
            delete(handlers::delete).post(handlers::edit),
        )
        .route("/entries/restore", post(handlers::restore))
        .route("/entries/{id}/edit", get(handlers::edit_form))
        .route("/entries/{id}/delete", post(handlers::delete))
        .route("/parse-preview", post(handlers::preview))
}
