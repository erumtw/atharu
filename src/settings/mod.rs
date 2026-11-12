//! `/settings`: the look picker (spec "Looks") and the glass controls (D17). Stored in the `settings` table, read by the layout.
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
        .route("/settings", get(handlers::page).post(handlers::save))
        .route("/settings/glass", post(handlers::save_glass))
}
