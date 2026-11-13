use crate::domain::{
    entry::{Kind, NoteRef},
    plan::Plan,
    tags,
    transcript::TxStatus,
};
use chrono::NaiveDate;
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
    include_str!("../migrations/010_meeting_notes.sql"),
    include_str!("../migrations/011_series_archive.sql"),
    include_str!("../migrations/012_mac_events.sql"),
    include_str!("../migrations/013_jira_fields.sql"),
];

/// `user_version` after 006: the step that also fills `taggings` from text written before tags existed.
const TAGS_VERSION: usize = 6;
/// `user_version` after 010: the step that moves each meeting's notes text into a note.
const MEETING_NOTES_VERSION: usize = 10;

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
        if i + 1 == MEETING_NOTES_VERSION {
            backfill_meeting_notes(&tx)?;
        }
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// 010: each meeting with notes text gets a note "<series> · <day>" holding it (plus `[[series]]`, so the note
/// links back), written by the owner, tagged from its text. Per row in Rust: SQL can't pair each new note id
/// with its meeting, and the tags need the parser.
fn backfill_meeting_notes(conn: &Connection) -> rusqlite::Result<()> {
    let rows: Vec<(i64, String, String, String)> = conn
        .prepare("SELECT id, day, name, notes FROM meetings WHERE trim(notes) <> '' ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;
    for (id, day, name, text) in rows {
        let body = format!("{}\n\n[[{name}]]", text.trim_end());
        conn.execute(
            "INSERT INTO notes(title, body, author) VALUES (?1, ?2, 'me')",
            rusqlite::params![format!("{name} · {day}"), body],
        )?;
        let note = conn.last_insert_rowid();
        conn.execute("UPDATE meetings SET note_id = ?1 WHERE id = ?2", [note, id])?;
        retag(conn, "note", &note.to_string(), &tags::tags(&body))?;
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

/// Whether `[[x]]` still points somewhere once the `renamed` kind (`"tag"` / `"series"`) no longer has `x`
/// (`links::href` order). A rename rewrites `[[old]]` only when this is false, so a link that now lands on a
/// topic, epic, tag or series of the same name keeps its target (0.2.0 gate P4). A failed read counts as
/// "no", which falls back to following the rename.
pub fn still_resolves(conn: &Connection, x: &str, renamed: &str) -> bool {
    let has = |sql: &str, s: &str| {
        conn.query_row(sql, [s], |r| r.get::<_, bool>(0))
            .unwrap_or(false)
    };
    crate::domain::links::href(
        x,
        |s| has("SELECT EXISTS (SELECT 1 FROM epics WHERE key = ?1)", s),
        |s| has("SELECT EXISTS (SELECT 1 FROM topics WHERE slug = ?1)", s),
        |s| renamed != "series" && has("SELECT EXISTS (SELECT 1 FROM meetings WHERE name = ?1)", s),
        |s| renamed != "tag" && has("SELECT EXISTS (SELECT 1 FROM taggings WHERE tag = ?1)", s),
    )
    .is_some()
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

/// A to-do row's columns (`FROM entries`, unaliased: callers append WHERE / ORDER BY), mapped by `plan`. Shared like
/// `retag`: every list drawing `plan_row.html` (To-dos, Today, a meeting, an epic, a tag hub) loads the same row.
/// Column 10 = the Jira chip label, the cached issue whose key ends a path/param segment of the link.
pub const SELECT_PLAN: &str =
    "SELECT id, day, epic_key, text, link, due, pinned, plan_state, closed_day,
         (SELECT j.key || ' · ' || j.summary FROM jira_issues j
           WHERE (link || '#') GLOB '*[/=]' || j.key || '[^0-9]*' LIMIT 1),
         author, note_id, (SELECT title FROM notes WHERE notes.id = entries.note_id)
         FROM entries";

/// `entries.kind` at column `i`; the CHECK constraint guarantees one of the six, so the fallback never fires.
pub fn kind(r: &rusqlite::Row, i: usize) -> rusqlite::Result<Kind> {
    let s: String = r.get(i)?;
    Ok(Kind::parse(&s).unwrap_or(Kind::Done))
}

/// `meeting_transcripts.status` at column `i`; the CHECK constraint guarantees one of the four, so an unknown
/// value is an error, not a guess.
pub fn tx_status(r: &rusqlite::Row, i: usize) -> rusqlite::Result<TxStatus> {
    let s: String = r.get(i)?;
    TxStatus::parse(&s).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(i, "status".into(), rusqlite::types::Type::Text)
    })
}

pub fn plan(r: &rusqlite::Row) -> rusqlite::Result<Plan> {
    let date = |s: Option<String>| s.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());
    Ok(Plan {
        id: r.get(0)?,
        day: date(r.get(1)?).unwrap_or_default(),
        epic_key: r.get(2)?,
        text: r.get(3)?,
        link: r.get(4)?,
        due: date(r.get(5)?),
        pinned: r.get(6)?,
        state: r.get(7)?,
        closed_day: date(r.get(8)?),
        jira: r.get(9)?,
        author: r.get(10)?,
        note: NoteRef::from_cols(r.get(11)?, r.get(12)?),
        // Only To-dos' filtered list selects it (14th column).
        via: match r.get(13) {
            Err(rusqlite::Error::InvalidColumnIndex(_)) => None,
            v => v?,
        },
    })
}
