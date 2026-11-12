use crate::domain::entry::Kind;
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
    /// Kind of its first linked line (D49), the list's badge; `None` = not linked.
    pub first_kind: Option<Kind>,
    /// Lines linked to it.
    pub lines: i64,
}

fn note_row(r: &rusqlite::Row) -> rusqlite::Result<NoteRow> {
    let kind: Option<String> = r.get(5)?;
    Ok(NoteRow {
        id: r.get(0)?,
        title: r.get(1)?,
        body: r.get(2)?,
        author: r.get(3)?,
        updated_at: r.get(4)?,
        first_kind: kind.as_deref().and_then(Kind::parse),
        lines: r.get(6)?,
    })
}

// "First" = the oldest line: the one the note was made from.
const NOTE_COLS: &str = "id, title, body, author, updated_at,
    (SELECT kind FROM entries WHERE note_id = notes.id ORDER BY day, id LIMIT 1),
    (SELECT COUNT(*) FROM entries WHERE note_id = notes.id)";

/// Newest first; `q` non-empty = substring match on title + body (case-insensitive for ASCII);
/// `tags` non-empty = only notes with any of them (D29); `linked` = only notes some line links to (D49).
// ponytail: LIKE scans every note, FTS5 when search gets slow.
pub fn list(
    conn: &Connection,
    q: &str,
    tags: &[String],
    linked: bool,
) -> rusqlite::Result<Vec<NoteRow>> {
    let pat = format!(
        "%{}%",
        q.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let mut stmt = conn.prepare(&format!(
        "SELECT {NOTE_COLS} FROM notes
         WHERE (?1 = '' OR title LIKE ?2 ESCAPE '\\' OR body LIKE ?2 ESCAPE '\\')
           AND (json_array_length(?3) = 0 OR EXISTS (SELECT 1 FROM taggings t
                 WHERE t.item_type = 'note' AND t.item_id = CAST(notes.id AS TEXT)
                   AND t.tag IN (SELECT value FROM json_each(?3))))
           AND (NOT ?4 OR EXISTS (SELECT 1 FROM entries WHERE note_id = notes.id))
         ORDER BY updated_at DESC, id DESC"
    ))?;
    let tags = serde_json::to_string(tags).unwrap_or_else(|_| "[]".into());
    stmt.query_map(params![q, pat, tags, linked], note_row)?
        .collect()
}

pub fn note(conn: &Connection, id: i64) -> rusqlite::Result<Option<NoteRow>> {
    conn.query_row(
        &format!("SELECT {NOTE_COLS} FROM notes WHERE id = ?1"),
        [id],
        note_row,
    )
    .optional()
}

/// The line's note id, making it first if it has none: titled after the line (its text, which the
/// parser already stripped of prefix and key), author me, linked in the same transaction.
/// `None` = no such line.
pub fn note_for_line(conn: &Connection, line: i64) -> rusqlite::Result<Option<i64>> {
    let tx = conn.unchecked_transaction()?;
    let Some((note, text)) = tx
        .query_row(
            "SELECT note_id, text FROM entries WHERE id = ?1",
            [line],
            |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
    else {
        return Ok(None);
    };
    if let Some(n) = note {
        return Ok(Some(n));
    }
    tx.execute("INSERT INTO notes(title) VALUES (?1)", [text.trim()])?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE entries SET note_id = ?1 WHERE id = ?2",
        params![id, line],
    )?;
    tx.commit()?;
    Ok(Some(id))
}

/// One line in a note's header (D49).
pub struct LinkedLine {
    pub id: i64,
    pub day: String,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    pub epic_key: Option<String>,
    /// Plans only: `open` / `done` / `dropped`.
    pub plan_state: Option<String>,
    pub due: Option<String>,
    pub closed_day: Option<String>,
}

/// Lines linked to the note, newest logged first.
pub fn lines(conn: &Connection, note: i64) -> rusqlite::Result<Vec<LinkedLine>> {
    let mut stmt = conn.prepare(
        "SELECT id, day, kind, topic, text, epic_key, plan_state, due, closed_day
         FROM entries WHERE note_id = ?1 ORDER BY day DESC, id DESC",
    )?;
    stmt.query_map([note], |r| {
        let kind: String = r.get(2)?;
        Ok(LinkedLine {
            id: r.get(0)?,
            day: r.get(1)?,
            kind: Kind::parse(&kind).unwrap_or(Kind::Done),
            topic: r.get(3)?,
            text: r.get(4)?,
            epic_key: r.get(5)?,
            plan_state: r.get(6)?,
            due: r.get(7)?,
            closed_day: r.get(8)?,
        })
    })?
    .collect()
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

/// Existing topic slugs, meeting series and tags in use, for resolving `[[X]]`.
type Known = (HashSet<String>, HashSet<String>, HashSet<String>);

pub fn known_pages(conn: &Connection) -> rusqlite::Result<Known> {
    let topics = conn
        .prepare("SELECT slug FROM topics")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let meetings = conn
        .prepare("SELECT DISTINCT name FROM meetings")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let tags = conn
        .prepare("SELECT DISTINCT tag FROM taggings")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok((topics, meetings, tags))
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

/// Deletes a note and hands back (title, JSON snapshot) for the Undo toast. Its lines lose the link
/// (`ON DELETE SET NULL`); the snapshot keeps their ids so Undo can link them again.
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<Option<(String, String)>> {
    // RETURNING runs after the delete, when SET NULL has already cleared the links: read them first.
    let tx = conn.unchecked_transaction()?;
    let lines: String = tx.query_row(
        "SELECT json_group_array(id) FROM entries WHERE note_id = ?1",
        [id],
        |r| r.get(0),
    )?;
    let out = tx
        .query_row(
            "DELETE FROM notes WHERE id = ?1 RETURNING title,
               json_object('id', id, 'title', title, 'body', body, 'author', author,
                           'updated_at', updated_at, 'lines', json(?2))",
            params![id, lines],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    tx.commit()?;
    Ok(out)
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
    // Only lines no other note took meanwhile (snapshots from before line notes have no `lines`).
    tx.execute(
        "UPDATE entries SET note_id = ?1 WHERE note_id IS NULL
           AND id IN (SELECT value FROM json_each(COALESCE(?2 ->> 'lines', '[]')))",
        params![id, snap],
    )?;
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

/// Tags in use, most used first: the filter box's suggestions.
pub fn tag_names(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT tag FROM taggings GROUP BY tag ORDER BY COUNT(*) DESC, tag")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}
