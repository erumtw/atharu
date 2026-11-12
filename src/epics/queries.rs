use crate::domain::entry::Kind;
use rusqlite::{Connection, OptionalExtension, params};

pub struct EpicRow {
    pub key: String,
    pub title: String,
    pub role: String,
    pub okr: String,
    pub status: String,
    pub start: Option<String>,
    pub target: Option<String>,
    pub done_on: Option<String>,
    pub metric: String,
    pub before: String,
    pub after: String,
    /// Latest `entries.day` on this epic, if any.
    pub last_log: Option<String>,
    /// Open plans on this epic (the "at risk" rule).
    pub open_plans: i64,
}

const SELECT_EPIC: &str =
    "SELECT ep.key, ep.title, ep.role, ep.okr, ep.status, ep.start, ep.target, ep.done_on,
        ep.metric, ep.before, ep.after, (SELECT MAX(e.day) FROM entries e WHERE e.epic_key = ep.key),
        (SELECT COUNT(*) FROM entries e WHERE e.epic_key = ep.key AND e.plan_state = 'open')
 FROM epics ep";

fn epic_row(r: &rusqlite::Row) -> rusqlite::Result<EpicRow> {
    Ok(EpicRow {
        key: r.get(0)?,
        title: r.get(1)?,
        role: r.get(2)?,
        okr: r.get(3)?,
        status: r.get(4)?,
        start: r.get(5)?,
        target: r.get(6)?,
        done_on: r.get(7)?,
        metric: r.get(8)?,
        before: r.get(9)?,
        after: r.get(10)?,
        last_log: r.get(11)?,
        open_plans: r.get(12)?,
    })
}

/// Named OKRs alphabetically, then "No OKR"; keys in order inside each.
pub fn all_epics(conn: &Connection) -> rusqlite::Result<Vec<EpicRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_EPIC} ORDER BY ep.okr = '', ep.okr, ep.key"
    ))?;
    stmt.query_map([], epic_row)?.collect()
}

pub fn epic(conn: &Connection, key: &str) -> rusqlite::Result<Option<EpicRow>> {
    conn.query_row(&format!("{SELECT_EPIC} WHERE ep.key = ?1"), [key], epic_row)
        .optional()
}

/// Columns a property form may write; the handler only passes names from this list.
pub const EDITABLE: [&str; 10] = [
    "title", "role", "okr", "status", "start", "target", "done_on", "metric", "before", "after",
];

/// Writes only the given columns. `None` value = SQL NULL (cleared date).
/// Returns false when the key does not exist.
pub fn update_fields(
    conn: &Connection,
    key: &str,
    fields: &[(&'static str, Option<String>)],
) -> rusqlite::Result<bool> {
    let exists = conn
        .query_row("SELECT 1 FROM epics WHERE key = ?1", [key], |_| Ok(()))
        .optional()?
        .is_some();
    for (col, val) in fields {
        debug_assert!(EDITABLE.contains(col));
        conn.execute(
            &format!("UPDATE epics SET {col} = ?1 WHERE key = ?2"),
            params![val, key],
        )?;
    }
    Ok(exists)
}

pub struct EvidenceRow {
    pub id: i64,
    pub url: String,
    pub note: String,
}

pub fn evidence(conn: &Connection, key: &str) -> rusqlite::Result<Vec<EvidenceRow>> {
    let mut stmt =
        conn.prepare("SELECT id, url, note FROM evidence WHERE epic_key = ?1 ORDER BY id")?;
    stmt.query_map([key], |r| {
        Ok(EvidenceRow {
            id: r.get(0)?,
            url: r.get(1)?,
            note: r.get(2)?,
        })
    })?
    .collect()
}

pub fn add_evidence(conn: &Connection, key: &str, url: &str, note: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO evidence(epic_key, url, note) VALUES (?1, ?2, ?3)",
        params![key, url, note],
    )?;
    Ok(())
}

/// Scoped by key so a stale form on one epic page can't delete another epic's evidence.
pub fn delete_evidence(conn: &Connection, key: &str, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute(
        "DELETE FROM evidence WHERE id = ?1 AND epic_key = ?2",
        params![id, key],
    )? > 0)
}

pub struct TimelineRow {
    pub day: String,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
}

/// Newest day first, newest line first inside a day (same order as Today).
pub fn timeline(conn: &Connection, key: &str) -> rusqlite::Result<Vec<TimelineRow>> {
    let mut stmt = conn.prepare(
        "SELECT day, kind, topic, text FROM entries WHERE epic_key = ?1 ORDER BY day DESC, id DESC",
    )?;
    stmt.query_map([key], |r| {
        let kind: String = r.get(1)?;
        Ok(TimelineRow {
            day: r.get(0)?,
            kind: Kind::parse(&kind).unwrap_or(Kind::Done),
            topic: r.get(2)?,
            text: r.get(3)?,
        })
    })?
    .collect()
}

/// Days of every line on the epic since `from`, for the activity sparkline.
pub fn entry_days_since(conn: &Connection, key: &str, from: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT day FROM entries WHERE epic_key = ?1 AND day >= ?2")?;
    stmt.query_map(params![key, from], |r| r.get(0))?.collect()
}
