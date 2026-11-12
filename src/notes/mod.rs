//! `/notes` list + search, `/notes/{id}` Tiptap editor (markdown autosave), `/api/pages` for the `[[` picker.
mod handlers;
mod queries;
mod view;

use crate::app::AppState;
use axum::{Router, routing::get};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/notes", get(handlers::list).post(handlers::create))
        .route("/notes/{id}", get(handlers::page).post(handlers::update))
        .route("/api/pages", get(handlers::pages))
}
