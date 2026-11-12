use crate::domain::{entry::Entry, plan::Plan};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Row, params};

// Last column: Jira chip label, the cached issue whose key ends a path/param segment of the link.
const SELECT_PLAN: &str =
    "SELECT id, day, epic_key, text, link, due, pinned, plan_state, closed_day,
         (SELECT j.key || ' · ' || j.summary FROM jira_issues j
           WHERE (link || '#') GLOB '*[/=]' || j.key || '[^0-9]*' LIMIT 1)
         FROM entries";

fn date(s: Option<String>) -> Option<NaiveDate> {
    s.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
}

fn plan(r: &Row) -> rusqlite::Result<Plan> {
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
        // Only the filtered list selects it (11th column).
        via: r.get(10).unwrap_or(None),
    })
}

/// Every open plan from any day (carry-over is free): pinned first, then by due, then oldest.
/// `filter` = only plans carrying any of these tags, typed or inherited (D29); empty = all.
pub fn open_plans(conn: &Connection, filter: &[String]) -> rusqlite::Result<Vec<Plan>> {
    let select = SELECT_PLAN.replace(
        "\n         FROM entries",
        ",\n         (SELECT CASE WHEN MIN(via IS NOT NULL) = 1 THEN MAX(via) END FROM entry_tags
            WHERE entry_id = entries.id AND tag IN (SELECT value FROM json_each(?1)))
         FROM entries",
    );
    let mut stmt = conn.prepare(&format!(
        "{select} WHERE plan_state = 'open'
           AND (json_array_length(?1) = 0 OR EXISTS (SELECT 1 FROM entry_tags et
                 WHERE et.entry_id = entries.id AND et.tag IN (SELECT value FROM json_each(?1))))
         ORDER BY pinned DESC, due IS NULL, due, day, id"
    ))?;
    let filter = serde_json::to_string(filter).unwrap_or_else(|_| "[]".into());
    stmt.query_map([filter], plan)?.collect()
}

pub fn done_between(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<Vec<Plan>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_PLAN} WHERE plan_state = 'done' AND closed_day BETWEEN ?1 AND ?2
         ORDER BY closed_day DESC, id DESC"
    ))?;
    stmt.query_map(params![from, to], plan)?.collect()
}

pub fn plan_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<Plan>> {
    conn.query_row(
        &format!("{SELECT_PLAN} WHERE id = ?1 AND kind = 'plan'"),
        [id],
        plan,
    )
    .optional()
}

/// Tags in use, most used first: the filter box's suggestions.
pub fn tag_names(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT tag FROM taggings GROUP BY tag ORDER BY COUNT(*) DESC, tag")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

pub fn epic_keys(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT key FROM epics ORDER BY key")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

/// Same write as a `>` line on Today: unknown key → stub epic.
pub fn insert_plan(conn: &mut Connection, day: &str, e: &Entry) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    if let Some(key) = &e.epic_key {
        tx.execute("INSERT OR IGNORE INTO epics(key) VALUES (?1)", [key])?;
    }
    tx.execute(
        // Spec: while a meeting runs, every captured line belongs to it.
        "INSERT INTO entries(day, epic_key, kind, text, plan_state, due, link, meeting_id)
         VALUES (?1, ?2, 'plan', ?3, 'open', ?4, ?5, (SELECT id FROM meetings WHERE ended_at IS NULL))",
        params![
            day,
            e.epic_key,
            e.text,
            e.due.map(|d| d.format("%Y-%m-%d").to_string()),
            e.link,
        ],
    )?;
    let id = tx.last_insert_rowid().to_string();
    crate::db::retag(&tx, "entry", &id, &crate::domain::tags::tags(&e.text))?;
    tx.commit()
}

/// Open pinned plans other than `id`: what the top-3 cap counts.
pub fn pinned_open(conn: &Connection, except: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM entries WHERE plan_state = 'open' AND pinned = 1 AND id != ?1",
        [except],
        |r| r.get(0),
    )
}

/// `closed_day` is the day it left `open`; reopening clears it.
pub fn set_state(conn: &Connection, id: i64, state: &str, today: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entries SET plan_state = ?1, closed_day = CASE ?1 WHEN 'open' THEN NULL ELSE ?2 END
         WHERE id = ?3 AND kind = 'plan'",
        params![state, today, id],
    )?;
    Ok(())
}

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entries SET pinned = ?1 WHERE id = ?2 AND kind = 'plan'",
        params![pinned, id],
    )?;
    Ok(())
}

pub fn set_due(conn: &Connection, id: i64, due: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entries SET due = ?1 WHERE id = ?2 AND kind = 'plan'",
        params![due, id],
    )?;
    Ok(())
}
