use crate::domain::entry::Kind;
use rusqlite::{Connection, OptionalExtension, params};

pub struct EpicRow {
    pub key: String,
    pub title: String,
    pub role: String,
    pub okr: String,
    pub status: String,
    pub start: Option<String>,
    pub target: Option<String>,
    pub done_on: Option<String>,
    pub metric: String,
    pub before: String,
    pub after: String,
    /// Latest `entries.day` on this epic, if any.
    pub last_log: Option<String>,
    /// Open plans on this epic (the "at risk" rule).
    pub open_plans: i64,
    /// `me`, or the agent that created it via the CLI.
    pub author: String,
    /// `summary · status` from the Jira cache, shown only while our title is empty.
    pub jira: Option<String>,
}

const SELECT_EPIC: &str =
    "SELECT ep.key, ep.title, ep.role, ep.okr, ep.status, ep.start, ep.target, ep.done_on,
        ep.metric, ep.before, ep.after, (SELECT MAX(e.day) FROM entries e WHERE e.epic_key = ep.key),
        (SELECT COUNT(*) FROM entries e WHERE e.epic_key = ep.key AND e.plan_state = 'open'),
        ep.author, j.summary || ' · ' || j.status
 FROM epics ep LEFT JOIN jira_issues j ON j.key = ep.key";

fn epic_row(r: &rusqlite::Row) -> rusqlite::Result<EpicRow> {
    Ok(EpicRow {
        key: r.get(0)?,
        title: r.get(1)?,
        role: r.get(2)?,
        okr: r.get(3)?,
        status: r.get(4)?,
        start: r.get(5)?,
        target: r.get(6)?,
        done_on: r.get(7)?,
        metric: r.get(8)?,
        before: r.get(9)?,
        after: r.get(10)?,
        last_log: r.get(11)?,
        open_plans: r.get(12)?,
        author: r.get(13)?,
        jira: r.get(14)?,
    })
}

/// Named OKRs alphabetically, then "No OKR"; keys in order inside each.
pub fn all_epics(conn: &Connection) -> rusqlite::Result<Vec<EpicRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_EPIC} ORDER BY ep.okr = '', ep.okr, ep.key"
    ))?;
    stmt.query_map([], epic_row)?.collect()
}

pub fn epic(conn: &Connection, key: &str) -> rusqlite::Result<Option<EpicRow>> {
    conn.query_row(&format!("{SELECT_EPIC} WHERE ep.key = ?1"), [key], epic_row)
        .optional()
}

/// Columns a property form may write; the handler only passes names from this list.
pub const EDITABLE: [&str; 10] = [
    "title", "role", "okr", "status", "start", "target", "done_on", "metric", "before", "after",
];

/// Writes only the given columns. `None` value = SQL NULL (cleared date).
/// Returns false when the key does not exist.
pub fn update_fields(
    conn: &Connection,
    key: &str,
    fields: &[(&'static str, Option<String>)],
) -> rusqlite::Result<bool> {
    let exists = conn
        .query_row("SELECT 1 FROM epics WHERE key = ?1", [key], |_| Ok(()))
        .optional()?
        .is_some();
    for (col, val) in fields {
        debug_assert!(EDITABLE.contains(col));
        conn.execute(
            &format!("UPDATE epics SET {col} = ?1 WHERE key = ?2"),
            params![val, key],
        )?;
    }
    Ok(exists)
}

pub struct EvidenceRow {
    pub id: i64,
    pub url: String,
    pub note: String,
    pub author: String,
}

pub fn evidence(conn: &Connection, key: &str) -> rusqlite::Result<Vec<EvidenceRow>> {
    let mut stmt =
        conn.prepare("SELECT id, url, note, author FROM evidence WHERE epic_key = ?1 ORDER BY id")?;
    stmt.query_map([key], |r| {
        Ok(EvidenceRow {
            id: r.get(0)?,
            url: r.get(1)?,
            note: r.get(2)?,
            author: r.get(3)?,
        })
    })?
    .collect()
}

pub fn add_evidence(
    conn: &Connection,
    key: &str,
    url: &str,
    note: &str,
    author: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO evidence(epic_key, url, note, author) VALUES (?1, ?2, ?3, ?4)",
        params![key, url, note, author],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Scoped by key so a stale form on one epic page can't delete another epic's evidence.
pub fn delete_evidence(conn: &Connection, key: &str, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute(
        "DELETE FROM evidence WHERE id = ?1 AND epic_key = ?2",
        params![id, key],
    )? > 0)
}

pub struct TimelineRow {
    pub id: i64,
    pub day: String,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    pub author: String,
}

/// Newest day first, newest line first inside a day (same order as Today).
pub fn timeline(conn: &Connection, key: &str) -> rusqlite::Result<Vec<TimelineRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, day, kind, topic, text, author FROM entries WHERE epic_key = ?1
         ORDER BY day DESC, id DESC",
    )?;
    stmt.query_map([key], |r| {
        let kind: String = r.get(2)?;
        Ok(TimelineRow {
            id: r.get(0)?,
            day: r.get(1)?,
            kind: Kind::parse(&kind).unwrap_or(Kind::Done),
            topic: r.get(3)?,
            text: r.get(4)?,
            author: r.get(5)?,
        })
    })?
    .collect()
}

/// Days of every line on the epic since `from`, for the activity sparkline.
pub fn entry_days_since(conn: &Connection, key: &str, from: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT day FROM entries WHERE epic_key = ?1 AND day >= ?2")?;
    stmt.query_map(params![key, from], |r| r.get(0))?.collect()
}

/// Notes whose body has `[[KEY]]`, newest first: (id, title) for "Mentioned in N notes".
pub fn mentions(conn: &Connection, x: &str) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, CASE WHEN title = '' THEN 'Untitled' ELSE title END FROM notes
         WHERE body LIKE '%[[' || ?1 || ']]%' ORDER BY updated_at DESC, id DESC",
    )?;
    stmt.query_map([x], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

/// Agent upsert: a new key gets the agent's title/OKR and author; an existing epic only has
/// empty title/OKR filled, so nothing I typed is ever overwritten. Returns true if it was created.
pub fn agent_upsert(
    conn: &Connection,
    key: &str,
    title: &str,
    okr: &str,
    author: &str,
) -> rusqlite::Result<bool> {
    let created = conn.execute(
        "INSERT OR IGNORE INTO epics(key, title, okr, author) VALUES (?1, ?2, ?3, ?4)",
        params![key, title, okr, author],
    )? > 0;
    for (col, v) in [("title", title), ("okr", okr)] {
        if !v.is_empty() {
            conn.execute(
                &format!("UPDATE epics SET {col} = ?1 WHERE key = ?2 AND {col} = ''"),
                params![v, key],
            )?;
        }
    }
    Ok(created)
}

/// Outcome of deleting an epic: lines keep it alive (spec: mark it Done or Dropped instead).
pub enum Deleted {
    Gone(String),
    HasLines,
    Missing,
}

/// Deletes an epic with no lines (its evidence cascades); hands back a JSON snapshot for Undo.
pub fn delete(conn: &mut Connection, key: &str) -> rusqlite::Result<Deleted> {
    let tx = conn.transaction()?;
    let snap: Option<(String, bool)> = tx
        .query_row(
            "SELECT json_object('key', key, 'title', title, 'role', role, 'okr', okr,
               'status', status, 'start', start, 'target', target, 'done_on', done_on,
               'metric', metric, 'before', before, 'after', after, 'author', author,
               'evidence', json((SELECT json_group_array(json_array(v.url, v.note, v.author))
                                 FROM evidence v WHERE v.epic_key = ep.key))),
               EXISTS (SELECT 1 FROM entries e WHERE e.epic_key = ep.key)
             FROM epics ep WHERE key = ?1",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let out = match snap {
        None => Deleted::Missing,
        Some((_, true)) => Deleted::HasLines,
        Some((s, false)) => {
            tx.execute("DELETE FROM epics WHERE key = ?1", [key])?;
            Deleted::Gone(s)
        }
    };
    tx.commit()?;
    Ok(out)
}

/// Undo. `Ok(Err(key))` = the key exists again (a new line re-created it); nothing restored.
pub fn restore(conn: &mut Connection, snap: &str) -> rusqlite::Result<Result<String, String>> {
    let tx = conn.transaction()?;
    let key: String = tx.query_row("SELECT ?1 ->> 'key'", [snap], |r| r.get(0))?;
    if epic(&tx, &key)?.is_some() {
        return Ok(Err(key));
    }
    tx.execute(
        "INSERT INTO epics(key, title, role, okr, status, start, target, done_on, metric, before, after, author)
         VALUES (?1 ->> 'key', ?1 ->> 'title', ?1 ->> 'role', ?1 ->> 'okr', ?1 ->> 'status',
                 ?1 ->> 'start', ?1 ->> 'target', ?1 ->> 'done_on', ?1 ->> 'metric',
                 ?1 ->> 'before', ?1 ->> 'after', ?1 ->> 'author')",
        [snap],
    )?;
    tx.execute(
        "INSERT INTO evidence(epic_key, url, note, author)
         SELECT ?1 ->> 'key', value ->> 0, value ->> 1, value ->> 2 FROM json_each(?1, '$.evidence')",
        [snap],
    )?;
    tx.commit()?;
    Ok(Ok(key))
}
