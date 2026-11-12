use rusqlite::Connection;
use std::path::Path;

/// Ordered; index + 1 = the `user_version` after applying it. Append only, never edit a shipped one.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_author.sql"),
];

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
    }
    prepare(Connection::open(path)?)
}

pub fn open_in_memory() -> rusqlite::Result<Connection> {
    prepare(Connection::open_in_memory()?)
}

fn prepare(conn: Connection) -> rusqlite::Result<Connection> {
    // Off by default in SQLite and per connection: without it ON DELETE CASCADE / SET NULL do nothing.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // The CLI writes while the server holds its own connection: wait for the lock, don't fail.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let done: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(done as usize) {
        conn.execute_batch(&format!(
            "BEGIN;\n{sql}\nPRAGMA user_version = {};\nCOMMIT;",
            i + 1
        ))?;
    }
    Ok(())
}
