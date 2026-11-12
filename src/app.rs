use axum::{
    Router,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::NaiveDate;
use rusqlite::Connection;
use rust_embed::RustEmbed;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone)]
pub struct AppState {
    // ponytail: one global lock on one connection; fine for a single local user, pool if that ever changes.
    db: Arc<Mutex<Connection>>,
    /// Tests pin "today" so date logic (due, flags, presets) is deterministic.
    pub fixed_today: Option<NaiveDate>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        AppState {
            db: Arc::new(Mutex::new(conn)),
            fixed_today: None,
        }
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic mid-handler poisons the lock; the connection itself is still fine to use.
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn today(&self) -> NaiveDate {
        self.fixed_today
            .unwrap_or_else(|| chrono::Local::now().date_naive())
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(crate::today::routes())
        .merge(crate::todos::routes())
        .merge(crate::epics::routes())
        .merge(crate::report::routes())
        .merge(crate::meetings::routes())
        .route("/static/{*path}", get(static_file))
        .with_state(state)
}

#[derive(RustEmbed)]
#[folder = "static/"]
struct Static;

async fn static_file(Path(path): Path<String>) -> Response {
    match Static::get(&path) {
        Some(f) => (
            [(header::CONTENT_TYPE, f.metadata.mimetype().to_string())],
            f.data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
