use rusqlite::{Connection, params};

/// Open to-dos due in [from, to]: (day, text).
pub fn due_plans(
    conn: &Connection,
    from: &str,
    to: &str,
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT due, text FROM entries
         WHERE plan_state = 'open' AND due BETWEEN ?1 AND ?2 ORDER BY due, id",
    )?;
    stmt.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

/// Active epics whose target falls in [from, to]: (day, key).
pub fn targets(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT target, key FROM epics
         WHERE status = 'active' AND target BETWEEN ?1 AND ?2 ORDER BY target, key",
    )?;
    stmt.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

pub struct MeetingRow {
    pub id: i64,
    pub day: String,
    pub name: String,
    pub ended_at: Option<String>,
}

pub fn meetings(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<Vec<MeetingRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, day, name, ended_at FROM meetings WHERE day BETWEEN ?1 AND ?2 ORDER BY day, id",
    )?;
    stmt.query_map(params![from, to], |r| {
        Ok(MeetingRow {
            id: r.get(0)?,
            day: r.get(1)?,
            name: r.get(2)?,
            ended_at: r.get(3)?,
        })
    })?
    .collect()
}

/// Lines logged per day in [from, to]: (day, count).
pub fn logged(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt =
        conn.prepare("SELECT day, COUNT(*) FROM entries WHERE day BETWEEN ?1 AND ?2 GROUP BY day")?;
    stmt.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

pub struct EventRow {
    pub id: i64,
    pub source: String,
    pub title: String,
    /// `YYYY-MM-DD` (all day) or `YYYY-MM-DDTHH:MM`.
    pub starts_at: String,
}

/// Events starting in [from, to]. Text compare works for both stored forms: `2026-11-11` and
/// `2026-11-11T10:00` both sort before `2026-11-12`, so `< to + 1 day` keeps the index usable.
pub fn events(conn: &Connection, from: &str, to_excl: &str) -> rusqlite::Result<Vec<EventRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, source, title, starts_at FROM events
         WHERE starts_at >= ?1 AND starts_at < ?2 ORDER BY starts_at, id",
    )?;
    stmt.query_map(params![from, to_excl], |r| {
        Ok(EventRow {
            id: r.get(0)?,
            source: r.get(1)?,
            title: r.get(2)?,
            starts_at: r.get(3)?,
        })
    })?
    .collect()
}

pub fn add_own(conn: &Connection, title: &str, starts_at: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO events(source, title, starts_at) VALUES ('own', ?1, ?2)",
        params![title, starts_at],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Only own events: Google ones come back on the next fetch anyway, and we never write back.
pub fn delete_own(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute(
        "DELETE FROM events WHERE id = ?1 AND source = 'own'",
        params![id],
    )? > 0)
}

pub struct GoogleEvent {
    pub ext_id: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: Option<String>,
}

/// Mirror the feed: upsert by (source, ext_id), then drop Google rows the feed no longer has
/// (cancelled/deleted upstream). One transaction, so a page never sees half a refresh.
pub fn sync_google(conn: &mut Connection, events: &[GoogleEvent]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut up = tx.prepare(
            "INSERT INTO events(source, ext_id, title, starts_at, ends_at)
             VALUES ('google', ?1, ?2, ?3, ?4)
             ON CONFLICT(source, ext_id) DO UPDATE SET
               title = excluded.title, starts_at = excluded.starts_at, ends_at = excluded.ends_at",
        )?;
        for e in events {
            up.execute(params![e.ext_id, e.title, e.starts_at, e.ends_at])?;
        }
    }
    let seen =
        serde_json::Value::from(events.iter().map(|e| e.ext_id.as_str()).collect::<Vec<_>>());
    tx.execute(
        "DELETE FROM events WHERE source = 'google'
         AND ext_id NOT IN (SELECT value FROM json_each(?1))",
        params![seen.to_string()],
    )?;
    tx.commit()
}
