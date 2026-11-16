//! `/notes` list + search, `/notes/{id}` Tiptap editor (markdown autosave), `/api/pages` for the `[[` picker.
mod handlers;
pub(crate) mod images;
// The agent CLI writes its own notes through these (spec "Agent access").
pub(crate) mod queries;
mod view;

use crate::app::AppState;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/notes", get(handlers::list).post(handlers::create))
        .route("/notes/new", post(handlers::create_text))
        .route("/notes/{id}", get(handlers::page).post(handlers::update))
        .route("/notes/{id}/delete", post(handlers::delete))
        // axum's default body cap is 2 MB; a Retina screenshot is more, so only this route takes the full 25 MB.
        .route(
            "/notes/{id}/images",
            post(handlers::upload_image).layer(DefaultBodyLimit::max(images::MAX_BYTES)),
        )
        .route("/attachments/notes/{id}/{file}", get(handlers::image_file))
        .route("/notes/{id}/lines", get(handlers::lines))
        .route("/entries/{id}/note", post(handlers::for_line))
        .route("/notes/restore", post(handlers::restore))
        .route("/api/pages", get(handlers::pages))
}
