use rusqlite::{Connection, OptionalExtension};

pub struct Running {
    pub id: i64,
    pub name: String,
    /// `YYYY-MM-DD HH:MM:SS` local; NULL for meetings started before migration 007.
    pub started_at: Option<String>,
}

pub fn running(conn: &Connection) -> rusqlite::Result<Option<Running>> {
    conn.query_row(
        "SELECT id, name, started_at FROM meetings WHERE ended_at IS NULL",
        [],
        |r| {
            Ok(Running {
                id: r.get(0)?,
                name: r.get(1)?,
                started_at: r.get(2)?,
            })
        },
    )
    .optional()
}

/// Series names, most recently started first (ids only grow, so the newest meeting of a series ranks it).
pub fn recent_series(conn: &Connection, n: usize) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM meetings GROUP BY name ORDER BY MAX(id) DESC LIMIT ?1")?;
    stmt.query_map([n as i64], |r| r.get(0))?.collect()
}

/// Open to-dos due today or earlier, overdue first.
pub fn due_plans(conn: &Connection, today: &str) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, text, due FROM entries
         WHERE kind = 'plan' AND plan_state = 'open' AND due <= ?1
         ORDER BY due, id",
    )?;
    stmt.query_map([today], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect()
}

/// The line just saved, for "✓ Saved as … · Undo": (kind, epic key).
pub fn entry(conn: &Connection, id: i64) -> rusqlite::Result<Option<(String, Option<String>)>> {
    conn.query_row(
        "SELECT kind, epic_key FROM entries WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}
