use crate::domain::{
    entry::{Kind, NoteRef},
    plan::Plan,
};
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
        "INSERT INTO meetings(day, name, started_at)
         VALUES (?1, ?2, strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime'))",
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

/// Newest meeting (CLI `meeting latest`).
pub fn latest(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row("SELECT max(id) FROM meetings", [], |r| r.get(0))
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
        "SELECT id, day, epic_key, text, link, due, pinned, plan_state, closed_day,
         (SELECT j.key || ' · ' || j.summary FROM jira_issues j
           WHERE (link || '#') GLOB '*[/=]' || j.key || '[^0-9]*' LIMIT 1),
         note_id, (SELECT title FROM notes WHERE notes.id = entries.note_id)
         FROM entries
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
            jira: r.get(9)?,
            via: None,
            note: NoteRef::from_cols(r.get(10)?, r.get(11)?),
        })
    })?
    .collect()
}

/// Notes whose body has `[[name]]`, newest first: (id, title) for "Mentioned in N notes".
pub fn mentions(conn: &Connection, x: &str) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, CASE WHEN title = '' THEN 'Untitled' ELSE title END FROM notes
         WHERE body LIKE '%[[' || ?1 || ']]%' ORDER BY updated_at DESC, id DESC",
    )?;
    stmt.query_map([x], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

/// Deletes a meeting; its lines stay (FK sets their meeting_id to NULL). Hands back
/// (name, JSON snapshot incl. the line ids) so Undo can re-attach them.
pub fn delete(conn: &mut Connection, id: i64) -> rusqlite::Result<Option<(String, String)>> {
    let tx = conn.transaction()?;
    let snap = tx
        .query_row(
            "SELECT name, json_object('id', id, 'day', day, 'name', name, 'notes', notes,
               'ended_at', ended_at,
               'entries', json((SELECT json_group_array(e.id) FROM entries e WHERE e.meeting_id = m.id)),
               'transcripts', json((SELECT json_group_array(json_object('started_at', t.started_at,
                   'ended_at', t.ended_at, 'status', t.status, 'model', t.model, 'lang', t.lang,
                   'segments', json(t.segments)))
                 FROM meeting_transcripts t WHERE t.meeting_id = m.id)))
             FROM meetings m WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    tx.execute("DELETE FROM meetings WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(snap)
}

/// Undo. `Ok(Err(name))` = the snapshot was running and `name` runs now (one active at a time).
/// Lines are re-attached only if nothing claimed them meanwhile.
pub fn restore(conn: &mut Connection, snap: &str) -> rusqlite::Result<Result<i64, String>> {
    let tx = conn.transaction()?;
    let running: bool = tx.query_row("SELECT (?1 ->> 'ended_at') IS NULL", [snap], |r| r.get(0))?;
    if running && let Some((_, name)) = active(&tx)? {
        return Ok(Err(name));
    }
    tx.execute(
        "INSERT INTO meetings(id, day, name, notes, ended_at)
         SELECT CASE WHEN EXISTS (SELECT 1 FROM meetings WHERE id = j.id) THEN NULL ELSE j.id END,
                j.day, j.name, j.notes, j.ended_at
         FROM (SELECT ?1 ->> 'id' AS id, ?1 ->> 'day' AS day, ?1 ->> 'name' AS name,
                      ?1 ->> 'notes' AS notes, ?1 ->> 'ended_at' AS ended_at) j",
        [snap],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE entries SET meeting_id = ?2 WHERE meeting_id IS NULL
           AND id IN (SELECT value FROM json_each(?1, '$.entries'))",
        params![snap, id],
    )?;
    // Transcripts cascade away with the meeting, so Undo brings them back from the snapshot. A recording
    // that was still running comes back interrupted: its recorder stopped when the meeting went.
    tx.execute(
        "INSERT INTO meeting_transcripts(meeting_id, started_at, ended_at, status, model, lang, segments)
         SELECT ?2, value ->> 'started_at', value ->> 'ended_at',
                CASE WHEN value ->> 'status' IN ('live', 'finishing') THEN 'interrupted'
                     ELSE value ->> 'status' END,
                value ->> 'model', value ->> 'lang', json(value -> 'segments')
         FROM json_each(?1, '$.transcripts')",
        params![snap, id],
    )?;
    tx.commit()?;
    Ok(Ok(id))
}

/// A series' Tags prop: it has no row of its own (D36 option A), so its taggings are the field.
pub fn series_tags(conn: &Connection, name: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT tag FROM taggings WHERE item_type = 'series' AND item_id = ?1 ORDER BY tag",
    )?;
    stmt.query_map([name], |r| r.get(0))?.collect()
}

pub fn set_series_tags(conn: &Connection, name: &str, tags: &[String]) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    crate::db::retag(&tx, "series", name, tags)?;
    tx.commit()
}

/// Tags in use, most used first: the Tags prop's suggestions.
pub fn tag_names(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT tag FROM taggings GROUP BY tag ORDER BY COUNT(*) DESC, tag")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

/// A new recording for a running meeting (the Mac app picks it up and starts capture).
/// `false` = the meeting has ended, or another recording is still live or finishing.
pub fn start_transcript(
    conn: &Connection,
    meeting: i64,
    at: &str,
    model: &str,
    lang: &str,
) -> rusqlite::Result<bool> {
    Ok(conn.execute(
        "INSERT INTO meeting_transcripts(meeting_id, started_at, model, lang)
         SELECT id, ?2, ?3, ?4 FROM meetings WHERE id = ?1 AND ended_at IS NULL
           AND NOT EXISTS (SELECT 1 FROM meeting_transcripts WHERE status IN ('live', 'finishing'))",
        params![meeting, at, model, lang],
    )? > 0)
}

/// Stop recording / End meeting: live → finishing; the app transcribes the tail, then posts `done`.
pub fn stop_transcript(conn: &Connection, meeting: i64, at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE meeting_transcripts SET status = 'finishing', ended_at = ?2
         WHERE meeting_id = ?1 AND status = 'live'",
        params![meeting, at],
    )?;
    Ok(())
}

pub enum Append {
    Saved,
    NotFound,
    /// Done or interrupted: a late post must not reopen it.
    Closed,
}

/// The app's segments, one recording. A (speaker, t) already there is skipped, so a retried post
/// (or an overlap window repeating a line) never doubles it. `done` closes the recording; one that
/// ends with no lines at all is dropped, there is nothing to show.
pub fn append_segments(
    conn: &Connection,
    tid: i64,
    segs: &[(f64, String, String)],
    done: bool,
    at: &str,
) -> rusqlite::Result<Append> {
    let tx = conn.unchecked_transaction()?;
    let status: Option<String> = tx
        .query_row(
            "SELECT status FROM meeting_transcripts WHERE id = ?1",
            [tid],
            |r| r.get(0),
        )
        .optional()?;
    match status.as_deref() {
        None => return Ok(Append::NotFound),
        Some("live" | "finishing") => {}
        Some(_) => return Ok(Append::Closed),
    }
    let mut add = tx.prepare_cached(
        "UPDATE meeting_transcripts
         SET segments = json_insert(segments, '$[#]', json_object('t', ?2, 'speaker', ?3, 'text', ?4))
         WHERE id = ?1 AND NOT EXISTS (
           SELECT 1 FROM json_each(segments)
           WHERE value ->> 'speaker' = ?3 AND value ->> 't' = ?2)",
    )?;
    for (t, speaker, text) in segs {
        add.execute(params![tid, t, speaker, text])?;
    }
    drop(add);
    if done {
        tx.execute(
            "DELETE FROM meeting_transcripts WHERE id = ?1 AND json_array_length(segments) = 0",
            [tid],
        )?;
        tx.execute(
            "UPDATE meeting_transcripts SET status = 'done', ended_at = COALESCE(ended_at, ?2)
             WHERE id = ?1",
            params![tid, at],
        )?;
    }
    tx.commit()?;
    Ok(Append::Saved)
}

pub struct Transcript {
    pub id: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub model: String,
    pub lang: String,
    /// (t, speaker, text), in time order.
    pub segments: Vec<(f64, String, String)>,
}

/// A meeting's recordings, oldest first, each with its segments sorted by time, mic echo of the call hidden.
pub fn transcripts(conn: &Connection, meeting: i64) -> rusqlite::Result<Vec<Transcript>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, ended_at, status, model, lang FROM meeting_transcripts
         WHERE meeting_id = ?1 ORDER BY id",
    )?;
    let mut out: Vec<Transcript> = stmt
        .query_map([meeting], |r| {
            Ok(Transcript {
                id: r.get(0)?,
                started_at: r.get(1)?,
                ended_at: r.get(2)?,
                status: r.get(3)?,
                model: r.get(4)?,
                lang: r.get(5)?,
                segments: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut segs = conn.prepare(
        "SELECT j.value ->> 't', j.value ->> 'speaker', j.value ->> 'text'
         FROM meeting_transcripts t, json_each(t.segments) j WHERE t.id = ?1
         ORDER BY j.value ->> 't', j.id",
    )?;
    for t in &mut out {
        t.segments = crate::domain::echo::drop_echo(
            segs.query_map([t.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<_, _>>()?,
        );
    }
    Ok(out)
}
