use rusqlite::{Connection, OptionalExtension, params};

pub fn set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .optional()
}

/// The open DB's file; empty for an in-memory one.
pub fn db_file(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row(
        "SELECT file FROM pragma_database_list WHERE name = 'main'",
        [],
        |r| r.get(0),
    )
}

/// `OPS-101 Checkout latency`: the Vocabulary row's example of what the glossary passes.
pub fn first_epic(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT trim(key || ' ' || title) FROM epics WHERE status = 'active' ORDER BY key LIMIT 1",
        [],
        |r| r.get(0),
    )
    .optional()
}
