use crate::domain::{entry::Entry, plan::Plan};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Row, params};

const SELECT_PLAN: &str =
    "SELECT id, day, epic_key, text, link, due, pinned, plan_state, closed_day FROM entries";

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
    })
}

/// Every open plan from any day (carry-over is free): pinned first, then by due, then oldest.
pub fn open_plans(conn: &Connection) -> rusqlite::Result<Vec<Plan>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_PLAN} WHERE plan_state = 'open'
         ORDER BY pinned DESC, due IS NULL, due, day, id"
    ))?;
    stmt.query_map([], plan)?.collect()
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
