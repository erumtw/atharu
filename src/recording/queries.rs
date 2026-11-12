use rusqlite::{Connection, OptionalExtension, params};

/// The running meeting's recording that is still live or finishing: (status, started_at).
pub fn running(conn: &Connection) -> rusqlite::Result<Option<(String, String)>> {
    conn.query_row(
        "SELECT t.status, t.started_at FROM meeting_transcripts t
         JOIN meetings m ON m.id = t.meeting_id
         WHERE m.ended_at IS NULL AND t.status IN ('live', 'finishing')
         ORDER BY t.id DESC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// Newest recording live or finishing, any meeting (End also stops): (id, status, lang, meeting name).
pub fn want(conn: &Connection) -> rusqlite::Result<Option<(i64, String, String, String)>> {
    conn.query_row(
        "SELECT t.id, t.status, t.lang, m.name FROM meeting_transcripts t
         JOIN meetings m ON m.id = t.meeting_id
         WHERE t.status IN ('live', 'finishing') ORDER BY t.id DESC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .optional()
}

pub fn active_epics(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt =
        conn.prepare("SELECT key, title FROM epics WHERE status = 'active' ORDER BY key")?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

pub fn setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .optional()
}

pub fn interrupt_orphans(conn: &Connection, at: &str) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM meeting_transcripts
         WHERE status IN ('live', 'finishing') AND json_array_length(segments) = 0",
        [],
    )?;
    tx.execute(
        "UPDATE meeting_transcripts SET status = 'interrupted', ended_at = COALESCE(ended_at, ?1)
         WHERE status IN ('live', 'finishing')",
        params![at],
    )?;
    tx.commit()
}
