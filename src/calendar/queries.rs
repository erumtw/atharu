use rusqlite::{Connection, params};

// Every read below takes `?3` = the tag filter as a JSON array (D29); `[]` = everything. Any of the
// tags counts, typed or inherited, as on To-dos.
const ANY: &str = "(SELECT value FROM json_each(?3))";

fn json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".into())
}

/// Open to-dos due in [from, to]: (day, text).
pub fn due_plans(
    conn: &Connection,
    from: &str,
    to: &str,
    tags: &[String],
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT due, text FROM entries
         WHERE plan_state = 'open' AND due BETWEEN ?1 AND ?2
           AND (json_array_length(?3) = 0 OR EXISTS (SELECT 1 FROM entry_tags et
                 WHERE et.entry_id = entries.id AND et.tag IN {ANY}))
         ORDER BY due, id"
    ))?;
    stmt.query_map(params![from, to, json(tags)], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })?
    .collect()
}

/// Active epics whose target falls in [from, to]: (day, key).
pub fn targets(
    conn: &Connection,
    from: &str,
    to: &str,
    tags: &[String],
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT target, key FROM epics
         WHERE status = 'active' AND target BETWEEN ?1 AND ?2
           AND (json_array_length(?3) = 0 OR EXISTS (SELECT 1 FROM taggings t
                 WHERE t.item_type = 'epic' AND t.item_id = epics.key AND t.tag IN {ANY}))
         ORDER BY target, key"
    ))?;
    stmt.query_map(params![from, to, json(tags)], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })?
    .collect()
}

pub struct MeetingRow {
    pub id: i64,
    pub day: String,
    pub name: String,
    /// Local `YYYY-MM-DD HH:MM:SS`; NULL on meetings from before 007.
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

/// Filtered: a series tagged with it, or named it (one name, one hub).
pub fn meetings(
    conn: &Connection,
    from: &str,
    to: &str,
    tags: &[String],
) -> rusqlite::Result<Vec<MeetingRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT id, day, name, started_at, ended_at FROM meetings WHERE day BETWEEN ?1 AND ?2
           AND name NOT IN (SELECT name FROM series_archive)
           AND (json_array_length(?3) = 0 OR name IN {ANY} OR EXISTS (SELECT 1 FROM taggings t
                 WHERE t.item_type = 'series' AND t.item_id = meetings.name AND t.tag IN {ANY}))
         ORDER BY day, id"
    ))?;
    stmt.query_map(params![from, to, json(tags)], |r| {
        Ok(MeetingRow {
            id: r.get(0)?,
            day: r.get(1)?,
            name: r.get(2)?,
            started_at: r.get(3)?,
            ended_at: r.get(4)?,
        })
    })?
    .collect()
}

/// Lines logged per day in [from, to]: (day, count).
pub fn logged(
    conn: &Connection,
    from: &str,
    to: &str,
    tags: &[String],
) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT day, COUNT(*) FROM entries WHERE day BETWEEN ?1 AND ?2
           AND (json_array_length(?3) = 0 OR EXISTS (SELECT 1 FROM entry_tags et
                 WHERE et.entry_id = entries.id AND et.tag IN {ANY}))
         GROUP BY day"
    ))?;
    stmt.query_map(params![from, to, json(tags)], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })?
    .collect()
}

pub struct EventRow {
    pub id: i64,
    pub source: String,
    pub title: String,
    /// `YYYY-MM-DD` (all day) or `YYYY-MM-DDTHH:MM`.
    pub starts_at: String,
    /// Same forms as `starts_at`; own events have none.
    pub ends_at: Option<String>,
    /// Own event whose title has one of the filter's tags (Google events are matched by word instead).
    pub tagged: bool,
}

/// Events overlapping [from, to]: starting before the day after `to`, ending (or, with no end, starting) on or
/// after `from`. Text compare works for both stored forms: `2026-11-11` and `2026-11-11T10:00` both sort before
/// `2026-11-12`; an event that ended before `from` but still matches here (all-day end on `from`) gets no chip.
pub fn events(
    conn: &Connection,
    from: &str,
    to_excl: &str,
    tags: &[String],
) -> rusqlite::Result<Vec<EventRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT id, source, title, starts_at, ends_at, EXISTS (SELECT 1 FROM taggings t
                  WHERE t.item_type = 'event' AND t.item_id = CAST(events.id AS TEXT) AND t.tag IN {ANY})
         FROM events WHERE starts_at < ?2 AND COALESCE(ends_at, starts_at) >= ?1 ORDER BY starts_at, id"
    ))?;
    stmt.query_map(params![from, to_excl, json(tags)], |r| {
        Ok(EventRow {
            id: r.get(0)?,
            source: r.get(1)?,
            title: r.get(2)?,
            starts_at: r.get(3)?,
            ends_at: r.get(4)?,
            tagged: r.get(5)?,
        })
    })?
    .collect()
}

/// Own events only take `#tag`s from their title (D29); Google ones are matched by word on the hub, never stored.
pub fn add_own(conn: &Connection, title: &str, starts_at: &str) -> rusqlite::Result<i64> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO events(source, title, starts_at) VALUES ('own', ?1, ?2)",
        params![title, starts_at],
    )?;
    let id = tx.last_insert_rowid();
    crate::db::retag(
        &tx,
        "event",
        &id.to_string(),
        &crate::domain::tags::tags(title),
    )?;
    tx.commit()?;
    Ok(id)
}

/// Only own events: Google ones come back on the next fetch anyway, and we never write back.
pub fn delete_own(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let gone = tx.execute(
        "DELETE FROM events WHERE id = ?1 AND source = 'own'",
        params![id],
    )? > 0;
    // events has no untag trigger (006 shipped without one), so the delete clears its rows itself.
    crate::db::retag(&tx, "event", &id.to_string(), &[])?;
    tx.commit()?;
    Ok(gone)
}

pub struct GoogleEvent {
    pub ext_id: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: Option<String>,
}

/// Mirror the feed: upsert by (source, ext_id), then drop Google rows the feed no longer has
/// (cancelled/deleted upstream). One transaction, so a page never sees half a refresh.
pub fn sync_google(conn: &mut Connection, events: &[GoogleEvent]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut up = tx.prepare(
            "INSERT INTO events(source, ext_id, title, starts_at, ends_at)
             VALUES ('google', ?1, ?2, ?3, ?4)
             ON CONFLICT(source, ext_id) DO UPDATE SET
               title = excluded.title, starts_at = excluded.starts_at, ends_at = excluded.ends_at",
        )?;
        for e in events {
            up.execute(params![e.ext_id, e.title, e.starts_at, e.ends_at])?;
        }
    }
    let seen =
        serde_json::Value::from(events.iter().map(|e| e.ext_id.as_str()).collect::<Vec<_>>());
    tx.execute(
        "DELETE FROM events WHERE source = 'google'
         AND ext_id NOT IN (SELECT value FROM json_each(?1))",
        params![seen.to_string()],
    )?;
    tx.commit()
}

/// Tags in use, most used first: the filter box's suggestions.
pub fn tag_names(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT tag FROM taggings GROUP BY tag ORDER BY COUNT(*) DESC, tag")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

/// One event read from macOS Calendar, in `events`' stored forms (all-day = dates, end exclusive as in iCal).
pub struct MacEvent {
    /// EventKit `calendarItemExternalIdentifier` + `@` + start: repeats of one series stay distinct rows.
    pub ext_id: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: Option<String>,
}

/// The Mac app's sync: in one transaction, drop the `mac` events overlapping [from, to] (dates) and insert what
/// Calendar has there now, so moved, deleted and unchecked-calendar events go; history outside the window stays.
/// Stamps `mac_cal_synced` (local time) for Settings.
pub fn replace_mac_events(
    conn: &mut Connection,
    from: &str,
    to: &str,
    events: &[MacEvent],
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM events WHERE source = 'mac'
           AND starts_at < date(?2, '+1 day') AND COALESCE(ends_at, starts_at) >= ?1",
        params![from, to],
    )?;
    {
        // OR REPLACE: an event that starts before the window and was kept outside it is still the same row.
        let mut ins = tx.prepare(
            "INSERT OR REPLACE INTO events(source, ext_id, title, starts_at, ends_at)
             VALUES ('mac', ?1, ?2, ?3, ?4)",
        )?;
        for e in events {
            ins.execute(params![e.ext_id, e.title, e.starts_at, e.ends_at])?;
        }
    }
    tx.execute(
        "INSERT INTO settings(key, value) VALUES ('mac_cal_synced', datetime('now', 'localtime'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    tx.commit()
}
