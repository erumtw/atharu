use crate::config::FileConfig;
use axum::{
    Router,
    extract::{Path, Request, State},
    http::{Method, StatusCode, header},
    middleware::{self, Next},
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
    /// `ATHARU_HOST`: a Host header naming it passes the guard as well as the loopback names.
    pub bind_host: String,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        AppState {
            db: Arc::new(Mutex::new(conn)),
            fixed_today: None,
            settings: Arc::default(),
            bind_host: "127.0.0.1".into(),
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
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

/// DNS rebinding: a page on evil.com re-pointed at 127.0.0.1 still sends `Host: evil.com`, so only
/// loopback names (any port) and the bind host get in. No Host at all = not a browser, let it be.
/// CSRF: browsers mark a write from another site `Sec-Fetch-Site: cross-site`; there are no cookies
/// to steal, but the form posts alone could log and delete lines.
async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    if !allowed(&req, &st.bind_host) {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(req).await
}

fn allowed(req: &Request, bind_host: &str) -> bool {
    let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
    if let Some(host) = header("host") {
        let name = match host.strip_prefix('[') {
            Some(v6) => v6.split(']').next().unwrap_or(""),
            None => host.split(':').next().unwrap_or(""),
        };
        if !matches!(name, "127.0.0.1" | "localhost" | "::1") && name != bind_host {
            return false;
        }
    }
    let read = matches!(*req.method(), Method::GET | Method::HEAD);
    read || header("sec-fetch-site") != Some("cross-site")
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
