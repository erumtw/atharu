//! `/calendar`: week/month of what we already have (to-dos due, epic targets, meetings, logged days)
//! plus own events and Google events read from the secret iCal address (read-only).
mod handlers;
mod ical;
mod queries;
mod view;

pub use ical::{import, refresh};
pub use queries::{MacEvent, mac_settings, replace_mac_events};

use crate::app::AppState;
use axum::{
    Router,
    routing::{get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/calendar", get(handlers::page))
        .route("/calendar/events", post(handlers::add))
        .route("/calendar/events/{id}/delete", post(handlers::delete))
        .route("/calendar/refresh", post(handlers::refresh))
}
