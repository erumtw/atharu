//! Page sync (0.2.13): a page polls `/api/sync` and learns whether something changed since its last look,
//! made by another process (CLI, agents, the other server: SQLite's `data_version`) or by another tab or
//! device on this server (`AppState::writes_since`). The page then re-fetches its live parts or offers a reload.
mod queries;

use crate::{app::AppState, error::AppResult};
use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/sync", get(sync))
}

#[derive(Deserialize)]
pub struct SyncQuery {
    /// `seq` from the page's last answer; absent on its first look.
    since: Option<u64>,
    /// The asking page's id (`X-Atharu-Tab` on its writes): its own writes don't count as "other".
    tab: Option<String>,
    /// On a note page: also answer that note's `updatedAt`, so only a change to it asks for a reload.
    note: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sync {
    version: i64,
    seq: u64,
    other: bool,
    note: Option<NoteStamp>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteStamp {
    id: i64,
    updated_at: String,
}

async fn sync(State(st): State<AppState>, Query(q): Query<SyncQuery>) -> AppResult<Json<Sync>> {
    let (version, note) = {
        let conn = st.conn();
        let note = match q.note {
            Some(id) => {
                queries::note_updated(&conn, id)?.map(|updated_at| NoteStamp { id, updated_at })
            }
            None => None,
        };
        (queries::data_version(&conn)?, note)
    };
    let (seq, other) = st.writes_since(q.since.unwrap_or(u64::MAX), q.tab.as_deref().unwrap_or(""));
    Ok(Json(Sync {
        version,
        seq,
        other,
        note,
    }))
}
