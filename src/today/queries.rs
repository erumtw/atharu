use crate::domain::entry::{Entry, Kind};
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
}

/// Epic groups in order of their first line that day, unfiled last; newest line first inside a group.
pub fn day_entries(conn: &Connection, day: &str) -> rusqlite::Result<Vec<LoggedRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.epic_key, COALESCE(ep.title, ''), e.kind, e.topic, e.text
         FROM entries e LEFT JOIN epics ep ON ep.key = e.epic_key
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
        })
    })?
    .collect()
}

pub fn epic_keys(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT key FROM epics ORDER BY key")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

/// Returns the new id and whether the key was unknown (a stub epic was created for it).
pub fn insert_entry(conn: &mut Connection, day: &str, e: &Entry) -> rusqlite::Result<(i64, bool)> {
    let tx = conn.transaction()?;
    let mut new_epic = false;
    if let Some(key) = &e.epic_key {
        new_epic = tx.execute("INSERT OR IGNORE INTO epics(key) VALUES (?1)", [key])? > 0;
    }
    tx.execute(
        "INSERT INTO entries(day, epic_key, kind, topic, text, plan_state, due, link)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            day,
            e.epic_key,
            e.kind.as_str(),
            e.topic,
            e.text,
            (e.kind == Kind::Plan).then_some("open"),
            e.due.map(|d| d.format("%Y-%m-%d").to_string()),
            e.link,
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok((id, new_epic))
}

/// Deletes and hands back the line, so the toast can offer to re-log it.
pub fn delete_entry(conn: &Connection, id: i64) -> rusqlite::Result<Option<(String, Entry)>> {
    conn.query_row(
        "DELETE FROM entries WHERE id = ?1 RETURNING day, epic_key, kind, topic, text, due, link",
        [id],
        |r| {
            let kind: String = r.get(2)?;
            let due: Option<String> = r.get(5)?;
            Ok((
                r.get(0)?,
                Entry {
                    epic_key: r.get(1)?,
                    kind: Kind::parse(&kind).unwrap_or(Kind::Done),
                    topic: r.get(3)?,
                    text: r.get(4)?,
                    due: due.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    link: r.get(6)?,
                },
            ))
        },
    )
    .optional()
}
