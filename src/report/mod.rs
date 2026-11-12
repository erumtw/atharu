//! `/report`: this week / month / quarter as a paste-ready note (copy as markdown) plus numbers that are not copied.
mod handlers;
mod queries;
mod view;

use crate::app::AppState;
use axum::{Router, routing::get};

pub fn routes() -> Router<AppState> {
    Router::new().route("/report", get(handlers::page))
}
