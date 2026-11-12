//! `/epics` list grouped by OKR and `/epics/{key}`: properties (autosave), evidence, impact line, timeline.
mod handlers;
mod queries;
mod view;

use crate::app::AppState;
use axum::{
    Router,
    routing::{delete, get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/epics", get(handlers::list))
        .route(
            "/epics/{key}",
            get(handlers::page)
                .patch(handlers::update)
                .post(handlers::update),
        )
        .route("/epics/{key}/title", post(handlers::set_title))
        .route("/epics/{key}/evidence", post(handlers::add_evidence))
        .route(
            "/epics/{key}/evidence/{id}",
            delete(handlers::delete_evidence),
        )
        .route(
            "/epics/{key}/evidence/{id}/delete",
            post(handlers::delete_evidence),
        )
}
