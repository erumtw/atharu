//! `/quick`: the menu-bar popover (T81, D46). Capture with kind tiles, start/end a meeting, today's + overdue to-dos.
//! Every form posts to the owning slice's endpoint with `back=/quick`; this slice only reads.
mod handlers;
mod queries;
mod view;

use crate::app::AppState;
use axum::{Router, routing::get};
use rusqlite::Connection;

pub fn routes() -> Router<AppState> {
    Router::new().route("/quick", get(handlers::page))
}

/// What the tray's right-click menu (Atharu.app) names.
#[derive(Default)]
pub struct TrayMenu {
    /// The running meeting (id, name): the menu offers to end it.
    pub running: Option<(i64, String)>,
    /// Else the recent series to start.
    pub series: Vec<String>,
    /// The running meeting's live recording started at (tray icon state + elapsed time).
    pub recording: Option<String>,
}

pub fn tray_menu(conn: &Connection) -> rusqlite::Result<TrayMenu> {
    let running = queries::running(conn)?.map(|r| (r.id, r.name));
    Ok(TrayMenu {
        recording: match &running {
            Some((id, _)) => queries::recording(conn, *id)?,
            None => None,
        },
        running,
        series: queries::recent_series(conn, handlers::SERIES)?,
    })
}
