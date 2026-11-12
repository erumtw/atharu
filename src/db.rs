use crate::domain::tags;
use rusqlite::Connection;
use std::path::Path;

/// Ordered; index + 1 = the `user_version` after applying it. Append only, never edit a shipped one.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_author.sql"),
    include_str!("../migrations/003_jira.sql"),
    include_str!("../migrations/004_settings.sql"),
    include_str!("../migrations/005_hardening.sql"),
    include_str!("../migrations/006_tags.sql"),
    include_str!("../migrations/007_meeting_start.sql"),
    include_str!("../migrations/008_line_notes.sql"),
    include_str!("../migrations/009_meeting_transcripts.sql"),
];

/// `user_version` after 006: the step that also fills `taggings` from text written before tags existed.
const TAGS_VERSION: usize = 6;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
    }
    let conn = Connection::open(path)?;
    // WAL: the CLI (and `sqlite3` reads) don't block the server's readers, and vice versa.
    // File DBs only; in-memory ones can't use it. Persistent, so this is a no-op after the first open.
    let _mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    prepare(conn)
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
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        if i + 1 == TAGS_VERSION {
            backfill_tags(&tx)?;
        }
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// What `#tag` parsing can't do in SQL: one pass over every line and note, inside the 006 transaction.
fn backfill_tags(conn: &Connection) -> rusqlite::Result<()> {
    for (sql, kind) in [
        ("SELECT id, text FROM entries", "entry"),
        ("SELECT id, body FROM notes", "note"),
    ] {
        let rows: Vec<(i64, String)> = conn
            .prepare(sql)?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (id, text) in rows {
            retag(conn, kind, &id.to_string(), &tags::tags(&text))?;
        }
    }
    Ok(())
}

/// Rebuild one item's rows in `taggings`. Every save of a line, note or tags field calls it inside
/// the save's own transaction, so the lookup can never disagree with the text. Lives here, not in
/// a slice: it maintains a derived index every slice writes through, like the migrations do.
pub fn retag(
    conn: &Connection,
    item_type: &str,
    item_id: &str,
    tags: &[String],
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM taggings WHERE item_type = ?1 AND item_id = ?2",
        [item_type, item_id],
    )?;
    let mut ins = conn.prepare_cached(
        "INSERT OR IGNORE INTO taggings(tag, item_type, item_id) VALUES (?1, ?2, ?3)",
    )?;
    for t in tags {
        ins.execute([t.as_str(), item_type, item_id])?;
    }
    Ok(())
}
