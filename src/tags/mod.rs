//! `/tags` list and `/tags/{tag}` hub (D29): one page per tag gathering its lines, to-dos, epics and notes.
mod handlers;
// `atharu tags` lists through the same query as the page.
pub(crate) mod queries;
mod view;

use crate::app::AppState;
use axum::{
    Router,
    routing::{get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tags", get(handlers::list))
        .route("/tags/{tag}", get(handlers::hub))
        .route("/tags/{tag}/rename", post(handlers::rename))
        .route("/tags/{tag}/delete", post(handlers::delete))
        .route("/tags/{tag}/older", post(handlers::older))
        // A tag can't be named `undo/…`, but `/tags/undo` would be the hub of #undo: keep it off that path.
        .route("/tags-undo", post(handlers::undo))
}
