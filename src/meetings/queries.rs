use crate::domain::{entry::Kind, plan::Plan};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};

fn date(s: Option<String>) -> Option<NaiveDate> {
    s.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
}

/// The running meeting, if any: (id, name).
pub fn active(conn: &Connection) -> rusqlite::Result<Option<(i64, String)>> {
    conn.query_row(
        "SELECT id, name FROM meetings WHERE ended_at IS NULL",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// (name, meetings) per series, most recently met first.
pub fn series(conn: &Connection) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT name, COUNT(*) FROM meetings GROUP BY name ORDER BY MAX(day) DESC, name",
    )?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

/// `Err(name)` = another meeting is still running. Checked first so the copy can name it;
/// the unique partial index is the backstop.
pub fn start(conn: &Connection, day: &str, name: &str) -> rusqlite::Result<Result<i64, String>> {
    if let Some((_, running)) = active(conn)? {
        return Ok(Err(running));
    }
    conn.execute(
        "INSERT INTO meetings(day, name) VALUES (?1, ?2)",
        params![day, name],
    )?;
    Ok(Ok(conn.last_insert_rowid()))
}

pub fn end(conn: &Connection, id: i64, at: &str) -> rusqlite::Result<bool> {
    Ok(conn.execute(
        "UPDATE meetings SET ended_at = ?1 WHERE id = ?2 AND ended_at IS NULL",
        params![at, id],
    )? > 0)
}

pub fn set_notes(conn: &Connection, id: i64, notes: &str) -> rusqlite::Result<bool> {
    Ok(conn.execute(
        "UPDATE meetings SET notes = ?1 WHERE id = ?2",
        params![notes, id],
    )? > 0)
}

pub struct MeetingRow {
    pub id: i64,
    pub day: String,
    pub name: String,
    pub notes: String,
    pub ended_at: Option<String>,
    pub decisions: i64,
    pub learned: i64,
    pub open: i64,
    pub done: i64,
    pub dropped: i64,
}

/// Series by latest meeting, newest day first inside; `name` narrows to one series.
pub fn meetings(conn: &Connection, name: Option<&str>) -> rusqlite::Result<Vec<MeetingRow>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.day, m.name, m.notes, m.ended_at,
                COUNT(CASE WHEN e.kind = 'decision' THEN 1 END),
                COUNT(CASE WHEN e.kind = 'learn' THEN 1 END),
                COUNT(CASE WHEN e.plan_state = 'open' THEN 1 END),
                COUNT(CASE WHEN e.plan_state = 'done' THEN 1 END),
                COUNT(CASE WHEN e.plan_state = 'dropped' THEN 1 END)
         FROM meetings m LEFT JOIN entries e ON e.meeting_id = m.id
         WHERE ?1 IS NULL OR m.name = ?1
         GROUP BY m.id
         ORDER BY (SELECT MAX(x.day) FROM meetings x WHERE x.name = m.name) DESC, m.name,
                  m.day DESC, m.id DESC",
    )?;
    stmt.query_map([name], |r| {
        Ok(MeetingRow {
            id: r.get(0)?,
            day: r.get(1)?,
            name: r.get(2)?,
            notes: r.get(3)?,
            ended_at: r.get(4)?,
            decisions: r.get(5)?,
            learned: r.get(6)?,
            open: r.get(7)?,
            done: r.get(8)?,
            dropped: r.get(9)?,
        })
    })?
    .collect()
}

pub struct Meeting {
    pub day: String,
    pub name: String,
    pub notes: String,
    pub ended_at: Option<String>,
}

pub fn meeting(conn: &Connection, id: i64) -> rusqlite::Result<Option<Meeting>> {
    conn.query_row(
        "SELECT day, name, notes, ended_at FROM meetings WHERE id = ?1",
        [id],
        |r| {
            Ok(Meeting {
                day: r.get(0)?,
                name: r.get(1)?,
                notes: r.get(2)?,
                ended_at: r.get(3)?,
            })
        },
    )
    .optional()
}

/// Other meetings of the series, newest first: (id, day).
pub fn siblings(
    conn: &Connection,
    name: &str,
    except: i64,
) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, day FROM meetings WHERE name = ?1 AND id != ?2 ORDER BY day DESC, id DESC",
    )?;
    stmt.query_map(params![name, except], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

pub struct Line {
    pub kind: Kind,
    pub epic_key: Option<String>,
    pub topic: Option<String>,
    pub text: String,
}

/// Non-plan lines logged in the meeting, oldest first (plans come from `plans`).
pub fn lines(conn: &Connection, id: i64) -> rusqlite::Result<Vec<Line>> {
    let mut stmt = conn.prepare(
        "SELECT kind, epic_key, topic, text FROM entries
         WHERE meeting_id = ?1 AND kind != 'plan' ORDER BY id",
    )?;
    stmt.query_map([id], |r| {
        let kind: String = r.get(0)?;
        Ok(Line {
            kind: Kind::parse(&kind).unwrap_or(Kind::Done),
            epic_key: r.get(1)?,
            topic: r.get(2)?,
            text: r.get(3)?,
        })
    })?
    .collect()
}

/// Action items: plans logged in the meeting, any state.
pub fn plans(conn: &Connection, id: i64) -> rusqlite::Result<Vec<Plan>> {
    let mut stmt = conn.prepare(
        "SELECT id, day, epic_key, text, link, due, pinned, plan_state, closed_day FROM entries
         WHERE meeting_id = ?1 AND kind = 'plan' ORDER BY id",
    )?;
    stmt.query_map([id], |r| {
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
    })?
    .collect()
}
