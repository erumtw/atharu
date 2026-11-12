//! `/tags` list and `/tags/{tag}` hub (D29): one page per tag gathering its lines, to-dos, epics and notes.
mod handlers;
// `atharu tags` lists through the same query as the page.
pub(crate) mod queries;
mod view;

use crate::app::AppState;
use axum::{Router, routing::get};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tags", get(handlers::list))
        .route("/tags/{tag}", get(handlers::hub))
}
