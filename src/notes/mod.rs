//! `/notes` list + search, `/notes/{id}` Tiptap editor (markdown autosave), `/api/pages` for the `[[` picker.
mod handlers;
// The agent CLI writes its own notes through these (spec "Agent access").
pub(crate) mod queries;
mod view;

use crate::app::AppState;
use axum::{
    Router,
    routing::{get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/notes", get(handlers::list).post(handlers::create))
        .route("/notes/new", post(handlers::create_text))
        .route("/notes/{id}", get(handlers::page).post(handlers::update))
        .route("/notes/{id}/delete", post(handlers::delete))
        .route("/notes/{id}/lines", get(handlers::lines))
        .route("/entries/{id}/note", post(handlers::for_line))
        .route("/notes/restore", post(handlers::restore))
        .route("/api/pages", get(handlers::pages))
}
