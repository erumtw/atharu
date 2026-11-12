//! `/todos`: every open `>` line grouped by due date, plus the plan actions (done · drop · pin · due) every page posts to.
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
        .route("/todos", get(handlers::page).post(handlers::create))
        .route("/todos/preview", post(handlers::preview))
        .route(
            "/plans/{id}",
            post(handlers::update).patch(handlers::update),
        )
}
