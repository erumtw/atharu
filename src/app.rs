use crate::config::FileConfig;
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
use std::{
    future::Future,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

#[derive(Clone)]
pub struct AppState {
    // ponytail: one global lock on one connection; fine for a single local user, pool if that ever changes.
    db: Arc<Mutex<Connection>>,
    /// Tests pin "today" so date logic (due, flags, presets) is deterministic.
    pub fixed_today: Option<NaiveDate>,
    /// `data/config.toml`, read once at start (edit + restart to change).
    pub settings: Arc<FileConfig>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        AppState {
            db: Arc::new(Mutex::new(conn)),
            fixed_today: None,
            settings: Arc::default(),
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
        .merge(crate::calendar::routes())
        .merge(crate::epics::routes())
        .merge(crate::report::routes())
        .merge(crate::meetings::routes())
        .merge(crate::learn::routes())
        .merge(crate::notes::routes())
        .merge(crate::jira::routes())
        .merge(crate::settings::routes())
        .route("/static/{*path}", get(static_file))
        .with_state(state)
}

/// Run `job` now and then every `period`, forever. Shared by every read-only integration
/// (Google iCal, Jira titles): each brings its own `refresh`, this owns the timing.
pub fn every<F, Fut>(period: Duration, mut job: F)
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
{
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(period);
        // A laptop waking from sleep should fetch once, not replay every missed tick.
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await; // first tick is immediate = "fetch on start"
            job().await;
        }
    });
}

/// Background refreshers for whatever `data/config.toml` turns on. Needs a tokio runtime.
pub fn spawn_refreshers(state: &AppState) {
    const PERIOD: Duration = Duration::from_secs(15 * 60);
    if state.settings.ical_url.is_some() {
        let st = state.clone();
        every(PERIOD, move || {
            let st = st.clone();
            async move {
                if let Err(e) = crate::calendar::refresh(&st).await {
                    eprintln!("calendar refresh: {e}");
                }
            }
        });
    }
    if state.settings.jira().is_some() {
        let st = state.clone();
        every(PERIOD, move || {
            let st = st.clone();
            async move {
                if let Err(e) = crate::jira::refresh(&st).await {
                    eprintln!("jira refresh: {e}");
                }
            }
        });
    }
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
