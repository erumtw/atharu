use rusqlite::{Connection, OptionalExtension};

/// Changes when another connection (another process) commits; never for this server's own writes.
pub fn data_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA data_version", [], |r| r.get(0))
}

pub fn note_updated(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT updated_at FROM notes WHERE id = ?1", [id], |r| {
        r.get(0)
    })
    .optional()
}
