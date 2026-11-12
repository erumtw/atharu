use crate::domain::{
    entry::{Entry, Kind},
    plan::Plan,
};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};

/// One logged line as the Today log shows it, with its epic title for the group header.
pub struct LoggedRow {
    pub id: i64,
    pub epic_key: Option<String>,
    pub epic_title: String,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    /// Meeting the line was logged in.
    pub meeting: Option<String>,
    /// `me`, or the agent that filed it via the CLI.
    pub author: String,
}

/// Epic groups in order of their first line that day, unfiled last; newest line first inside a group.
pub fn day_entries(conn: &Connection, day: &str) -> rusqlite::Result<Vec<LoggedRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.epic_key, COALESCE(ep.title, ''), e.kind, e.topic, e.text, m.name, e.author
         FROM entries e LEFT JOIN epics ep ON ep.key = e.epic_key
         LEFT JOIN meetings m ON m.id = e.meeting_id
         WHERE e.day = ?1
         ORDER BY e.epic_key IS NULL,
                  (SELECT MIN(x.id) FROM entries x WHERE x.day = ?1 AND x.epic_key IS e.epic_key),
                  e.id DESC",
    )?;
    stmt.query_map([day], |r| {
        let kind: String = r.get(3)?;
        Ok(LoggedRow {
            id: r.get(0)?,
            epic_key: r.get(1)?,
            epic_title: r.get(2)?,
            // The CHECK constraint guarantees one of the six.
            kind: Kind::parse(&kind).unwrap_or(Kind::Done),
            topic: r.get(4)?,
            text: r.get(5)?,
            meeting: r.get(6)?,
            author: r.get(7)?,
        })
    })?
    .collect()
}

/// The running meeting, if any: (id, name).
pub fn active_meeting(conn: &Connection) -> rusqlite::Result<Option<(i64, String)>> {
    conn.query_row(
        "SELECT id, name FROM meetings WHERE ended_at IS NULL",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// Series names for the start box's datalist, most recently met first.
pub fn meeting_series(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM meetings GROUP BY name ORDER BY MAX(day) DESC, name")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

pub fn epic_keys(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT key FROM epics ORDER BY key")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

/// Returns the new id and whether the key was unknown (a stub epic was created for it).
/// `author` = `me` from the web, the agent slug from the CLI (also tags a stub epic it creates).
pub fn insert_entry(
    conn: &mut Connection,
    day: &str,
    e: &Entry,
    author: &str,
) -> rusqlite::Result<(i64, bool)> {
    let tx = conn.transaction()?;
    let mut new_epic = false;
    if let Some(key) = &e.epic_key {
        new_epic = tx.execute(
            "INSERT OR IGNORE INTO epics(key, author) VALUES (?1, ?2)",
            params![key, author],
        )? > 0;
    }
    tx.execute(
        // Spec: while a meeting runs, every captured line belongs to it.
        "INSERT INTO entries(day, epic_key, kind, topic, text, plan_state, due, link, author, meeting_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, (SELECT id FROM meetings WHERE ended_at IS NULL))",
        params![
            day,
            e.epic_key,
            e.kind.as_str(),
            e.topic,
            e.text,
            (e.kind == Kind::Plan).then_some("open"),
            e.due.map(|d| d.format("%Y-%m-%d").to_string()),
            e.link,
            author,
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok((id, new_epic))
}

/// Deletes and hands back (day, JSON snapshot of the whole row) for the Undo toast.
pub fn delete_entry(conn: &Connection, id: i64) -> rusqlite::Result<Option<(String, String)>> {
    conn.query_row(
        "DELETE FROM entries WHERE id = ?1 RETURNING day,
           json_object('id', id, 'day', day, 'epic_key', epic_key, 'meeting_id', meeting_id,
             'kind', kind, 'topic', topic, 'text', text, 'plan_state', plan_state,
             'closed_day', closed_day, 'pinned', pinned, 'due', due, 'link', link,
             'created_at', created_at, 'author', author)",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// Undo: re-inserts the snapshot as it was (author, plan state, pin, meeting), under its old id
/// unless that id was reused. Whatever it pointed at may be gone since: the epic comes back as a
/// stub, a deleted meeting becomes no meeting. Returns the day.
pub fn restore_entry(conn: &mut Connection, snap: &str) -> rusqlite::Result<String> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO epics(key) SELECT ?1 ->> 'epic_key' WHERE (?1 ->> 'epic_key') IS NOT NULL",
        [snap],
    )?;
    let day = tx.query_row(
        "INSERT INTO entries(id, day, epic_key, meeting_id, kind, topic, text, plan_state,
                             closed_day, pinned, due, link, created_at, author)
         SELECT CASE WHEN EXISTS (SELECT 1 FROM entries WHERE id = j.id) THEN NULL ELSE j.id END,
                j.day, j.epic_key,
                CASE WHEN EXISTS (SELECT 1 FROM meetings WHERE id = j.meeting_id) THEN j.meeting_id END,
                j.kind, j.topic, j.text, j.plan_state, j.closed_day, j.pinned, j.due, j.link,
                j.created_at, j.author
         FROM (SELECT ?1 ->> 'id' AS id, ?1 ->> 'day' AS day, ?1 ->> 'epic_key' AS epic_key,
                      ?1 ->> 'meeting_id' AS meeting_id, ?1 ->> 'kind' AS kind,
                      ?1 ->> 'topic' AS topic, ?1 ->> 'text' AS text,
                      ?1 ->> 'plan_state' AS plan_state, ?1 ->> 'closed_day' AS closed_day,
                      ?1 ->> 'pinned' AS pinned, ?1 ->> 'due' AS due, ?1 ->> 'link' AS link,
                      ?1 ->> 'created_at' AS created_at, ?1 ->> 'author' AS author) j
         RETURNING day",
        [snap],
        |r| r.get(0),
    )?;
    tx.commit()?;
    Ok(day)
}

/// The line as grammar text, for the inline edit box: (day, line).
pub fn entry_line(conn: &Connection, id: i64) -> rusqlite::Result<Option<(String, String)>> {
    conn.query_row(
        "SELECT day, epic_key, kind, topic, text, due, link FROM entries WHERE id = ?1",
        [id],
        |r| {
            let kind: String = r.get(2)?;
            let e = Entry {
                epic_key: r.get(1)?,
                kind: Kind::parse(&kind).unwrap_or(Kind::Done),
                topic: r.get(3)?,
                text: r.get(4)?,
                due: date(r.get(5)?),
                link: r.get(6)?,
            };
            Ok((r.get(0)?, e.to_line()))
        },
    )
    .optional()
}

/// Inline edit: rewrite what the line says (kind, epic, topic, text, due, link), keep id, day,
/// created_at, author and meeting. A plan staying a plan keeps its state/pin; leaving or becoming
/// one resets them, as the CHECK needs. Unknown key → stub epic, as on capture. Returns the day.
pub fn update_entry(conn: &mut Connection, id: i64, e: &Entry) -> rusqlite::Result<Option<String>> {
    let tx = conn.transaction()?;
    if let Some(key) = &e.epic_key {
        tx.execute("INSERT OR IGNORE INTO epics(key) VALUES (?1)", [key])?;
    }
    let plan = e.kind == Kind::Plan;
    // SET expressions read the old row, so `plan_state` here is the value before the edit.
    let day = tx
        .query_row(
            "UPDATE entries SET epic_key = ?2, kind = ?3, topic = ?4, text = ?5, due = ?6, link = ?7,
               plan_state = CASE WHEN ?8 THEN COALESCE(plan_state, 'open') END,
               pinned = CASE WHEN ?8 AND plan_state IS NOT NULL THEN pinned ELSE 0 END,
               closed_day = CASE WHEN ?8 THEN closed_day END
             WHERE id = ?1 RETURNING day",
            params![
                id,
                e.epic_key,
                e.kind.as_str(),
                e.topic,
                e.text,
                e.due.map(|d| d.format("%Y-%m-%d").to_string()),
                e.link,
                plan,
            ],
            |r| r.get(0),
        )
        .optional()?;
    // No such line: drop the tx (rollback) so a 404 edit leaves no stub epic behind.
    if day.is_some() {
        tx.commit()?;
    }
    Ok(day)
}

fn date(s: Option<String>) -> Option<NaiveDate> {
    s.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
}

/// Open plans from any day, plus pinned ones finished this week (they stay ticked in "This week").
/// Pinned first, then oldest.
pub fn plans(conn: &Connection, week_start: &str) -> rusqlite::Result<Vec<Plan>> {
    let mut stmt = conn.prepare(
        "SELECT id, day, epic_key, text, link, due, pinned, plan_state, closed_day,
         (SELECT j.key || ' · ' || j.summary FROM jira_issues j
           WHERE (link || '#') GLOB '*[/=]' || j.key || '[^0-9]*' LIMIT 1)
         FROM entries
         WHERE plan_state = 'open' OR (pinned = 1 AND plan_state = 'done' AND closed_day >= ?1)
         ORDER BY pinned DESC, day, id",
    )?;
    stmt.query_map([week_start], |r| {
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
        })
    })?
    .collect()
}

/// Active epics with what the flag rules read (spec "Progress flags").
pub struct EpicFacts {
    pub key: String,
    pub title: String,
    pub target: Option<NaiveDate>,
    pub last_activity: Option<NaiveDate>,
    pub open_plans: i64,
}

pub fn active_epic_facts(conn: &Connection) -> rusqlite::Result<Vec<EpicFacts>> {
    let mut stmt = conn.prepare(
        "SELECT ep.key, ep.title, ep.target,
                (SELECT MAX(e.day) FROM entries e WHERE e.epic_key = ep.key),
                (SELECT COUNT(*) FROM entries e WHERE e.epic_key = ep.key AND e.plan_state = 'open')
         FROM epics ep WHERE ep.status = 'active' ORDER BY ep.target IS NULL, ep.target, ep.key",
    )?;
    stmt.query_map([], |r| {
        Ok(EpicFacts {
            key: r.get(0)?,
            title: r.get(1)?,
            target: date(r.get(2)?),
            last_activity: date(r.get(3)?),
            open_plans: r.get(4)?,
        })
    })?
    .collect()
}
