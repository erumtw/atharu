use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;

pub struct NoteRow {
    pub id: i64,
    pub title: String,
    pub body: String,
    /// `me` or an agent slug.
    pub author: String,
    /// UTC `YYYY-MM-DD HH:MM:SS` (SQLite `datetime('now')`).
    pub updated_at: String,
}

fn note_row(r: &rusqlite::Row) -> rusqlite::Result<NoteRow> {
    Ok(NoteRow {
        id: r.get(0)?,
        title: r.get(1)?,
        body: r.get(2)?,
        author: r.get(3)?,
        updated_at: r.get(4)?,
    })
}

/// Newest first; `q` non-empty = substring match on title + body (case-insensitive for ASCII).
// ponytail: LIKE scans every note, FTS5 when search gets slow.
pub fn list(conn: &Connection, q: &str) -> rusqlite::Result<Vec<NoteRow>> {
    let pat = format!(
        "%{}%",
        q.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let mut stmt = conn.prepare(
        "SELECT id, title, body, author, updated_at FROM notes
         WHERE ?1 = '' OR title LIKE ?2 ESCAPE '\\' OR body LIKE ?2 ESCAPE '\\'
         ORDER BY updated_at DESC, id DESC",
    )?;
    stmt.query_map(params![q, pat], note_row)?.collect()
}

pub fn note(conn: &Connection, id: i64) -> rusqlite::Result<Option<NoteRow>> {
    conn.query_row(
        "SELECT id, title, body, author, updated_at FROM notes WHERE id = ?1",
        [id],
        note_row,
    )
    .optional()
}

pub fn create(conn: &Connection) -> rusqlite::Result<i64> {
    conn.execute("INSERT INTO notes(title) VALUES ('')", [])?;
    Ok(conn.last_insert_rowid())
}

/// Agent note (CLI). `author` is never `me`: the CLI refuses that name before it gets here.
pub fn agent_create(
    conn: &Connection,
    title: &str,
    body: &str,
    author: &str,
) -> rusqlite::Result<i64> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO notes(title, body, author) VALUES (?1, ?2, ?3)",
        params![title, body, author],
    )?;
    let id = tx.last_insert_rowid();
    retag(&tx, id, body)?;
    tx.commit()?;
    Ok(id)
}

/// Rewrites a note only if `author` wrote it; false = no such note of theirs (mine stay untouched).
pub fn agent_update(
    conn: &Connection,
    id: i64,
    title: &str,
    body: &str,
    author: &str,
) -> rusqlite::Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let ok = tx.execute(
        "UPDATE notes SET title = ?1, body = ?2, updated_at = datetime('now')
         WHERE id = ?3 AND author = ?4",
        params![title, body, id, author],
    )? > 0;
    if ok {
        retag(&tx, id, body)?;
    }
    tx.commit()?;
    Ok(ok)
}

/// Columns the editor form may write; anything else in the form is ignored.
pub const EDITABLE: [&str; 2] = ["title", "body"];

/// Writes only the given columns and bumps `updated_at`; false = unknown id.
/// Author stays as it is: editing an agent's note doesn't make it mine (spec).
pub fn update_fields(conn: &Connection, id: i64, f: &[(&str, &str)]) -> rusqlite::Result<bool> {
    if note(conn, id)?.is_none() {
        return Ok(false);
    }
    let tx = conn.unchecked_transaction()?;
    for (col, v) in f {
        // `col` comes from EDITABLE, never from the request.
        tx.execute(
            &format!("UPDATE notes SET {col} = ?1, updated_at = datetime('now') WHERE id = ?2"),
            params![v, id],
        )?;
        if *col == "body" {
            retag(&tx, id, v)?;
        }
    }
    tx.commit()?;
    Ok(true)
}

/// Existing topic slugs and meeting series, for resolving `[[X]]`.
pub fn known_pages(conn: &Connection) -> rusqlite::Result<(HashSet<String>, HashSet<String>)> {
    let topics = conn
        .prepare("SELECT slug FROM topics")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let meetings = conn
        .prepare("SELECT DISTINCT name FROM meetings")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok((topics, meetings))
}

pub struct PageRow {
    /// `epic` / `topic` / `meeting`.
    pub kind: &'static str,
    pub id: String,
    pub name: String,
}

/// `[[` picker: epics (key or title), topics, meeting series matching `q`; a handful of each.
pub fn pages(conn: &Connection, q: &str) -> rusqlite::Result<Vec<PageRow>> {
    let pat = format!("%{}%", q.replace(['%', '_', '\\'], ""));
    let mut out = Vec::new();
    for (kind, sql) in [
        (
            "epic",
            "SELECT key, title FROM epics WHERE key LIKE ?1 OR title LIKE ?1
             ORDER BY status != 'active', key LIMIT 8",
        ),
        (
            "topic",
            "SELECT slug, '' FROM topics WHERE slug LIKE ?1 ORDER BY slug LIMIT 8",
        ),
        (
            "meeting",
            "SELECT name, '' FROM meetings WHERE name LIKE ?1
             GROUP BY name ORDER BY MAX(day) DESC LIMIT 8",
        ),
    ] {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([&pat], |r| {
            Ok(PageRow {
                kind,
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

/// Deletes a note and hands back (title, JSON snapshot) for the Undo toast.
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<Option<(String, String)>> {
    conn.query_row(
        "DELETE FROM notes WHERE id = ?1 RETURNING title,
           json_object('id', id, 'title', title, 'body', body, 'author', author, 'updated_at', updated_at)",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// Undo: re-inserts the snapshot under its old id (a new one if that id was reused meanwhile).
pub fn restore(conn: &Connection, snap: &str) -> rusqlite::Result<i64> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO notes(id, title, body, author, updated_at)
         SELECT CASE WHEN EXISTS (SELECT 1 FROM notes WHERE id = j.id) THEN NULL ELSE j.id END,
                j.title, j.body, j.author, j.updated_at
         FROM (SELECT ?1 ->> 'id' AS id, ?1 ->> 'title' AS title, ?1 ->> 'body' AS body,
                      ?1 ->> 'author' AS author, ?1 ->> 'updated_at' AS updated_at) j",
        [snap],
    )?;
    let id = tx.last_insert_rowid();
    let body: String = tx.query_row("SELECT body FROM notes WHERE id = ?1", [id], |r| r.get(0))?;
    retag(&tx, id, &body)?;
    tx.commit()?;
    Ok(id)
}

/// A note's `#tags` (headings `# x` are not tags: the parser needs a word right after the `#`).
fn retag(conn: &Connection, id: i64, body: &str) -> rusqlite::Result<()> {
    crate::db::retag(
        conn,
        "note",
        &id.to_string(),
        &crate::domain::tags::tags(body),
    )
}
