//! `/learn`: inbox of new topics from `?` lines + topics by staleness; `/learn/{slug}`: own words, depth ratings.
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
        .route("/learn", get(handlers::list).post(handlers::make_topic))
        .route("/learn/{slug}", get(handlers::page).post(handlers::update))
        .route("/learn/{slug}/rate", post(handlers::rate))
        .route("/learn/{slug}/unrate", post(handlers::unrate))
}
