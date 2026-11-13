use crate::db::SELECT_PLAN;
use crate::domain::{
    entry::{LineRow, NoteRef},
    plan::Plan,
};
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
    /// Comma list of tags its lines inherit (D29).
    pub tags: String,
    /// Jira status alone, for the ↗ chip (cached; None = not in Jira or never fetched).
    pub jira_status: Option<String>,
    /// 0.2.5: Jira's own title (D7: the name until ours is typed), status category, due, last update and parent.
    pub jira_summary: Option<String>,
    pub jira_cat: Option<String>,
    pub jira_due: Option<String>,
    pub jira_updated: Option<String>,
    pub jira_parent: Option<String>,
    pub jira_parent_summary: Option<String>,
    /// Jira's resolution date (`YYYY-MM-DD`) and issue type.
    pub jira_resolved: Option<String>,
    pub jira_type: Option<String>,
}

/// A Jira child of an epic (D3: `parent_key` = the epic): its type, title, status, lines logged on its key, and
/// whether it has its own page here.
pub struct Child {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub cat: String,
    pub issue_type: String,
    pub lines: i64,
    pub tracked: bool,
}

pub fn children(conn: &Connection, key: &str) -> rusqlite::Result<Vec<Child>> {
    let mut stmt = conn.prepare(
        "SELECT j.key, j.summary, j.status, j.status_cat, j.issue_type,
                (SELECT COUNT(*) FROM entries WHERE epic_key = j.key),
                EXISTS (SELECT 1 FROM epics WHERE key = j.key)
           FROM jira_issues j WHERE j.parent_key = ?1 ORDER BY j.status_cat = 'done', j.key",
    )?;
    stmt.query_map([key], |r| {
        Ok(Child {
            key: r.get(0)?,
            summary: r.get(1)?,
            status: r.get(2)?,
            cat: r.get(3)?,
            issue_type: r.get(4)?,
            lines: r.get(5)?,
            tracked: r.get(6)?,
        })
    })?
    .collect()
}

/// D2 "Not yet": hidden until Jira's resolution date changes (the date it was hidden for is kept).
pub fn jira_not_yet(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'jira_notyet:' || ?1",
        [key],
        |r| r.get(0),
    )
    .optional()
}

pub fn set_jira_not_yet(conn: &Connection, key: &str, resolved: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES ('jira_notyet:' || ?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, resolved],
    )?;
    Ok(())
}

/// D2 "Mark done": Jira's resolution date becomes "done on". Only an active epic: a stale form never reopens
/// a dropped one as done or rewrites an earlier done date.
pub fn mark_done(conn: &Connection, key: &str, on: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE epics SET status = 'done', done_on = ?2 WHERE key = ?1 AND status = 'active'",
        params![key, on],
    )?;
    Ok(())
}

/// An open Jira issue assigned to me (D5), and whether Atharu already tracks it (an epic of that key, or an open
/// to-do linking it).
pub struct Assigned {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub cat: String,
    pub issue_type: String,
    pub due: Option<String>,
    pub tracked: bool,
    /// D6/D16: Jira's epic level (hierarchyLevel 1); unknown level → the type name "Epic".
    pub epic: bool,
    /// Its Jira parent (key, Jira's title for it), for grouping by parent (0.2.7).
    pub parent_key: Option<String>,
    pub parent_summary: Option<String>,
}

/// Newest-updated first, as Jira returned them.
pub fn assigned(conn: &Connection) -> rusqlite::Result<Vec<Assigned>> {
    let mut stmt = conn.prepare(
        "SELECT j.key, j.summary, j.status, j.status_cat, j.issue_type, j.due,
                EXISTS (SELECT 1 FROM epics WHERE key = j.key)
                OR EXISTS (SELECT 1 FROM entries WHERE plan_state = 'open' AND link LIKE '%/browse/' || j.key)
                , COALESCE(j.level = 1, j.issue_type = 'Epic'), j.parent_key, j.parent_summary
           FROM jira_issues j WHERE j.assigned_me = 1 ORDER BY j.updated_at DESC, j.key",
    )?;
    stmt.query_map([], |r| {
        Ok(Assigned {
            key: r.get(0)?,
            summary: r.get(1)?,
            status: r.get(2)?,
            cat: r.get(3)?,
            issue_type: r.get(4)?,
            due: r.get(5)?,
            tracked: r.get(6)?,
            epic: r.get(7)?,
            parent_key: r.get(8)?,
            parent_summary: r.get(9)?,
        })
    })?
    .collect()
}

/// A parent's cached (summary, status, status category), for its header row.
pub fn jira_issue(
    conn: &Connection,
    key: &str,
) -> rusqlite::Result<Option<(String, String, String)>> {
    conn.query_row(
        "SELECT summary, status, status_cat FROM jira_issues WHERE key = ?1",
        [key],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
}

/// Every epic key here, any status (D4: a parent tracked in another group, or done / dropped, is not "not tracked").
pub fn epic_keys(conn: &Connection) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT key FROM epics")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

/// D9: when Jira last refused the token (None = fine).
pub fn jira_auth_failed(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'jira_auth_failed'",
        [],
        |r| r.get(0),
    )
    .optional()
}

/// "+ Track as epic" (D6): the stub only; its name stays Jira's until one is typed (D7).
pub fn track(conn: &Connection, key: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT OR IGNORE INTO epics(key) VALUES (?1)", [key])?;
    Ok(())
}

const SELECT_EPIC: &str =
    "SELECT ep.key, ep.title, ep.role, ep.okr, ep.status, ep.start, ep.target, ep.done_on,
        ep.metric, ep.before, ep.after, (SELECT MAX(e.day) FROM entries e WHERE e.epic_key = ep.key),
        (SELECT COUNT(*) FROM entries e WHERE e.epic_key = ep.key AND e.plan_state = 'open'),
        ep.author, j.summary || ' · ' || j.status, ep.tags, j.status,
        j.summary, j.status_cat, j.due, j.updated_at, j.parent_key, j.parent_summary, j.resolved_on, j.issue_type
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
        tags: r.get(15)?,
        jira_status: r.get(16)?,
        jira_summary: r.get(17)?,
        jira_cat: r.get(18)?,
        jira_due: r.get(19)?,
        jira_updated: r.get(20)?,
        jira_parent: r.get(21)?,
        jira_parent_summary: r.get(22)?,
        jira_resolved: r.get(23)?,
        jira_type: r.get(24)?,
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
pub const EDITABLE: [&str; 11] = [
    "title", "role", "okr", "status", "start", "target", "done_on", "metric", "before", "after",
    "tags",
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
    let tx = conn.unchecked_transaction()?;
    for (col, val) in fields {
        debug_assert!(EDITABLE.contains(col));
        tx.execute(
            &format!("UPDATE epics SET {col} = ?1 WHERE key = ?2"),
            params![val, key],
        )?;
        if *col == "tags" && exists {
            retag(&tx, key, val.as_deref().unwrap_or(""))?;
        }
    }
    tx.commit()?;
    Ok(exists)
}

/// The field is already normalised (`a,b`); taggings follows it in the same transaction.
fn retag(conn: &Connection, key: &str, tags: &str) -> rusqlite::Result<()> {
    crate::db::retag(conn, "epic", key, &crate::domain::tags::parse_list(tags))
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

/// Newest day first, newest line first inside a day (same order as Today).
pub fn timeline(conn: &Connection, key: &str) -> rusqlite::Result<Vec<LineRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.day, e.kind, e.topic, e.text, e.author, n.id, n.title
         FROM entries e LEFT JOIN notes n ON n.id = e.note_id WHERE e.epic_key = ?1
         ORDER BY e.day DESC, e.id DESC",
    )?;
    stmt.query_map([key], |r| {
        Ok(LineRow {
            id: r.get(0)?,
            day: r.get(1)?,
            kind: crate::db::kind(r, 2)?,
            topic: r.get(3)?,
            text: r.get(4)?,
            author: r.get(5)?,
            note: NoteRef::from_cols(r.get(6)?, r.get(7)?),
            ..LineRow::default()
        })
    })?
    .collect()
}

/// The epic's to-dos, any state, for their `plan_row.html` rows in the timeline.
pub fn plans(conn: &Connection, key: &str) -> rusqlite::Result<Vec<Plan>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_PLAN} WHERE epic_key = ?1 AND kind = 'plan'"
    ))?;
    stmt.query_map([key], crate::db::plan)?.collect()
}

/// Days of every line on the epic since `from`, for the activity sparkline.
pub fn entry_days_since(conn: &Connection, key: &str, from: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT day FROM entries WHERE epic_key = ?1 AND day >= ?2")?;
    stmt.query_map(params![key, from], |r| r.get(0))?.collect()
}

/// Agent upsert: a new key gets the agent's title/OKR and author; an existing epic only has
/// empty title/OKR filled, so nothing I typed is ever overwritten. Returns true if it was created.
pub fn agent_upsert(
    conn: &Connection,
    key: &str,
    title: &str,
    okr: &str,
    tags: &str,
    author: &str,
) -> rusqlite::Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let created = tx.execute(
        "INSERT OR IGNORE INTO epics(key, title, okr, author) VALUES (?1, ?2, ?3, ?4)",
        params![key, title, okr, author],
    )? > 0;
    for (col, v) in [("title", title), ("okr", okr), ("tags", tags)] {
        if !v.is_empty() {
            tx.execute(
                &format!("UPDATE epics SET {col} = ?1 WHERE key = ?2 AND {col} = ''"),
                params![v, key],
            )?;
        }
    }
    let now: String = tx.query_row("SELECT tags FROM epics WHERE key = ?1", [key], |r| r.get(0))?;
    retag(&tx, key, &now)?;
    tx.commit()?;
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
               'metric', metric, 'before', before, 'after', after, 'author', author, 'tags', tags,
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
        "INSERT INTO epics(key, title, role, okr, status, start, target, done_on, metric, before, after, author, tags)
         VALUES (?1 ->> 'key', ?1 ->> 'title', ?1 ->> 'role', ?1 ->> 'okr', ?1 ->> 'status',
                 ?1 ->> 'start', ?1 ->> 'target', ?1 ->> 'done_on', ?1 ->> 'metric',
                 ?1 ->> 'before', ?1 ->> 'after', ?1 ->> 'author', COALESCE(?1 ->> 'tags', ''))",
        [snap],
    )?;
    let tags: String = tx.query_row("SELECT COALESCE(?1 ->> 'tags', '')", [snap], |r| r.get(0))?;
    retag(&tx, &key, &tags)?;
    tx.execute(
        "INSERT INTO evidence(epic_key, url, note, author)
         SELECT ?1 ->> 'key', value ->> 0, value ->> 1, value ->> 2 FROM json_each(?1, '$.evidence')",
        [snap],
    )?;
    tx.commit()?;
    Ok(Ok(key))
}

/// Tags in use, most used first: the Tags prop's suggestions.
pub fn tag_names(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT tag FROM taggings GROUP BY tag ORDER BY COUNT(*) DESC, tag")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

/// Minutes since the last Jira refresh wrote any issue (`fetched_at` is UTC), None before the first one.
pub fn jira_age_min(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT CAST((julianday('now') - julianday(MAX(fetched_at))) * 1440 AS INTEGER) FROM jira_issues",
        [],
        |r| r.get(0),
    )
}
