use rusqlite::{Connection, params};

pub struct DoneEpic {
    pub key: String,
    pub title: String,
    pub metric: String,
    pub before: String,
    pub after: String,
    pub urls: Vec<String>,
}

/// Epics whose `done_on` falls in [from, to], with their evidence urls for the impact line.
pub fn done_epics(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<Vec<DoneEpic>> {
    let mut stmt = conn.prepare(
        "SELECT key, title, metric, before, after FROM epics
         WHERE status = 'done' AND done_on BETWEEN ?1 AND ?2 ORDER BY done_on, key",
    )?;
    let mut out: Vec<DoneEpic> = stmt
        .query_map(params![from, to], |r| {
            Ok(DoneEpic {
                key: r.get(0)?,
                title: r.get(1)?,
                metric: r.get(2)?,
                before: r.get(3)?,
                after: r.get(4)?,
                urls: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut ev = conn.prepare("SELECT url FROM evidence WHERE epic_key = ?1 ORDER BY id")?;
    for e in &mut out {
        e.urls = ev
            .query_map([&e.key], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
    }
    Ok(out)
}

pub struct Notable {
    pub epic_key: Option<String>,
    pub epic_title: String,
    pub kind: String,
    pub text: String,
    /// Meeting it was logged in, shown as `(oms-sync)`.
    pub meeting: Option<String>,
}

/// Impact, decisions, blockers in range: epics by key then unfiled; inside an epic impact → decision → blocker, oldest first.
pub fn notable(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<Vec<Notable>> {
    let mut stmt = conn.prepare(
        "SELECT e.epic_key, COALESCE(ep.title, ''), e.kind, e.text, m.name
         FROM entries e LEFT JOIN epics ep ON ep.key = e.epic_key
         LEFT JOIN meetings m ON m.id = e.meeting_id
         WHERE e.day BETWEEN ?1 AND ?2 AND e.kind IN ('impact', 'decision', 'blocker')
         ORDER BY e.epic_key IS NULL, e.epic_key,
                  CASE e.kind WHEN 'impact' THEN 0 WHEN 'decision' THEN 1 ELSE 2 END, e.day, e.id",
    )?;
    stmt.query_map(params![from, to], |r| {
        Ok(Notable {
            epic_key: r.get(0)?,
            epic_title: r.get(1)?,
            kind: r.get(2)?,
            text: r.get(3)?,
            meeting: r.get(4)?,
        })
    })?
    .collect()
}

/// Days of impact entries in range, for the per-week bars.
pub fn impact_days(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT day FROM entries WHERE kind = 'impact' AND day BETWEEN ?1 AND ?2")?;
    stmt.query_map(params![from, to], |r| r.get(0))?.collect()
}

/// (okr, status, count) over all epics: the "Epics by OKR" chart is a snapshot, not range-bound.
pub fn okr_status(conn: &Connection) -> rusqlite::Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT okr, status, COUNT(*) FROM epics
         GROUP BY okr, status ORDER BY okr = '', okr,
                  CASE status WHEN 'active' THEN 0 WHEN 'done' THEN 1 ELSE 2 END",
    )?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect()
}

/// Plans logged in range by state: (done, open, dropped).
pub fn plan_counts(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<(i64, i64, i64)> {
    conn.query_row(
        "SELECT COUNT(CASE WHEN plan_state = 'done' THEN 1 END),
                COUNT(CASE WHEN plan_state = 'open' THEN 1 END),
                COUNT(CASE WHEN plan_state = 'dropped' THEN 1 END)
         FROM entries WHERE kind = 'plan' AND day BETWEEN ?1 AND ?2",
        params![from, to],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
}

pub fn meetings_count(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM meetings WHERE day BETWEEN ?1 AND ?2",
        params![from, to],
        |r| r.get(0),
    )
}

/// Every rating up to `to`: (topic, day, depth), per topic oldest first.
pub fn ratings(conn: &Connection, to: &str) -> rusqlite::Result<Vec<(String, String, u8)>> {
    let mut stmt = conn.prepare(
        "SELECT topic, day, depth FROM depth_ratings WHERE day <= ?1 ORDER BY topic, day, rowid",
    )?;
    stmt.query_map([to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect()
}
