use crate::db::SELECT_PLAN;
use crate::domain::{
    entry::{LineRow, NoteRef},
    plan::Plan,
    transcript::TxStatus,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// (name, meetings) per series, most recently met first.
/// The one-running-meeting guard of start / restore, read inside their write; pages read `recording::MeetingState`.
fn running_name(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT name FROM meetings WHERE ended_at IS NULL",
        [],
        |r| r.get(0),
    )
    .optional()
}

/// `Err(name)` = another meeting is still running. Checked first so the copy can name it;
/// the unique partial index is the backstop.
pub fn start(conn: &Connection, day: &str, name: &str) -> rusqlite::Result<Result<i64, String>> {
    if let Some(running) = running_name(conn)? {
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
    pub ended_at: Option<String>,
    /// NULL for meetings from before migration 007.
    pub started_at: Option<String>,
    /// Transcript lines across its recordings (0 = never recorded).
    pub tx_lines: i64,
    pub decisions: i64,
    pub open: i64,
    pub done: i64,
    pub dropped: i64,
}

/// The meetings of one series, newest first.
pub fn meetings(conn: &Connection, name: &str) -> rusqlite::Result<Vec<MeetingRow>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.day, m.ended_at, m.started_at,
                (SELECT COALESCE(SUM(json_array_length(t.segments)), 0) FROM meeting_transcripts t
                  WHERE t.meeting_id = m.id),
                COUNT(CASE WHEN e.kind = 'decision' THEN 1 END),
                COUNT(CASE WHEN e.plan_state = 'open' THEN 1 END),
                COUNT(CASE WHEN e.plan_state = 'done' THEN 1 END),
                COUNT(CASE WHEN e.plan_state = 'dropped' THEN 1 END)
         FROM meetings m LEFT JOIN entries e ON e.meeting_id = m.id
         WHERE m.name = ?1
         GROUP BY m.id
         ORDER BY m.day DESC, m.id DESC",
    )?;
    stmt.query_map([name], |r| {
        Ok(MeetingRow {
            id: r.get(0)?,
            day: r.get(1)?,
            ended_at: r.get(2)?,
            started_at: r.get(3)?,
            tx_lines: r.get(4)?,
            decisions: r.get(5)?,
            open: r.get(6)?,
            done: r.get(7)?,
            dropped: r.get(8)?,
        })
    })?
    .collect()
}

pub struct Meeting {
    pub day: String,
    pub name: String,
    /// The pre-010 text field; kept only for rollback (the note below replaced it, D58).
    pub notes: String,
    pub ended_at: Option<String>,
    /// Its note (T90): (id, title, markdown body).
    pub note: Option<(i64, String, String)>,
}

/// Renames series `old` to `new` everywhere it is written: every meeting in it and its series tags, one
/// transaction. False = `new` is already another series (merging two is a separate, explicit choice).
pub fn rename_series(conn: &mut Connection, old: &str, new: &str) -> rusqlite::Result<bool> {
    if old == new {
        return Ok(true);
    }
    let tx = conn.transaction()?;
    let taken: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM meetings WHERE name = ?1)",
        [new],
        |r| r.get(0),
    )?;
    if taken {
        return Ok(false);
    }
    tx.execute(
        "UPDATE meetings SET name = ?2 WHERE name = ?1",
        params![old, new],
    )?;
    tx.execute(
        "UPDATE taggings SET item_id = ?2 WHERE item_type = 'series' AND item_id = ?1",
        params![old, new],
    )?;
    tx.execute(
        "UPDATE series_archive SET name = ?2 WHERE name = ?1",
        params![old, new],
    )?;
    // `[[old]]` in any note would turn into a dead link: follow the rename (the body's only change).
    tx.execute(
        "UPDATE notes SET body = replace(body, '[[' || ?1 || ']]', '[[' || ?2 || ']]')
          WHERE body LIKE '%[[' || ?1 || ']]%'",
        params![old, new],
    )?;
    tx.commit()?;
    Ok(true)
}

/// "Start notes" on a meeting without a note (T90): an owner note "<series> · <day>" ending in `[[series]]` so it
/// links back, attached to the meeting. Returns the existing one if a double click got here first.
pub fn create_note(conn: &mut Connection, id: i64) -> rusqlite::Result<Option<i64>> {
    let tx = conn.transaction()?;
    let row: Option<(String, String, Option<i64>)> = tx
        .query_row(
            "SELECT day, name, note_id FROM meetings WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((day, name, existing)) = row else {
        return Ok(None);
    };
    if existing.is_some() {
        return Ok(existing);
    }
    tx.execute(
        "INSERT INTO notes(title, body, author) VALUES (?1, ?2, 'me')",
        params![format!("{name} · {day}"), format!("\n\n[[{name}]]")],
    )?;
    let note = tx.last_insert_rowid();
    tx.execute(
        "UPDATE meetings SET note_id = ?1 WHERE id = ?2",
        params![note, id],
    )?;
    tx.commit()?;
    Ok(Some(note))
}

/// Newest meeting (CLI `meeting latest`).
pub fn latest(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row("SELECT max(id) FROM meetings", [], |r| r.get(0))
}

pub fn meeting(conn: &Connection, id: i64) -> rusqlite::Result<Option<Meeting>> {
    conn.query_row(
        "SELECT m.day, m.name, m.notes, m.ended_at, n.id, n.title, n.body
         FROM meetings m LEFT JOIN notes n ON n.id = m.note_id WHERE m.id = ?1",
        [id],
        |r| {
            let note: Option<i64> = r.get(4)?;
            Ok(Meeting {
                day: r.get(0)?,
                name: r.get(1)?,
                notes: r.get(2)?,
                ended_at: r.get(3)?,
                note: match note {
                    Some(n) => Some((n, r.get(5)?, r.get(6)?)),
                    None => None,
                },
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

/// Non-plan lines logged in the meeting, oldest first (plans come from `plans`).
pub fn lines(conn: &Connection, id: i64) -> rusqlite::Result<Vec<LineRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.day, e.kind, e.epic_key, e.topic, e.text, e.author, n.id, n.title
         FROM entries e LEFT JOIN notes n ON n.id = e.note_id
         WHERE e.meeting_id = ?1 AND e.kind != 'plan' ORDER BY e.id",
    )?;
    stmt.query_map([id], |r| {
        Ok(LineRow {
            id: r.get(0)?,
            day: r.get(1)?,
            kind: crate::db::kind(r, 2)?,
            epic: r.get(3)?,
            topic: r.get(4)?,
            text: r.get(5)?,
            author: r.get(6)?,
            note: NoteRef::from_cols(r.get(7)?, r.get(8)?),
            ..LineRow::default()
        })
    })?
    .collect()
}

/// Action items: plans logged in the meeting, any state.
/// One row per series for the Meetings list, latest meeting first.
pub struct SeriesRow {
    pub name: String,
    pub meetings: i64,
    pub last_day: String,
    pub open: i64,
    pub archived: bool,
}

pub fn series_rows(conn: &Connection) -> rusqlite::Result<Vec<SeriesRow>> {
    let mut stmt = conn.prepare(
        "SELECT m.name, COUNT(DISTINCT m.id), MAX(m.day),
                COUNT(CASE WHEN e.plan_state = 'open' THEN 1 END),
                m.name IN (SELECT name FROM series_archive)
         FROM meetings m LEFT JOIN entries e ON e.meeting_id = m.id
         GROUP BY m.name ORDER BY MAX(m.day) DESC, m.name",
    )?;
    stmt.query_map([], |r| {
        Ok(SeriesRow {
            name: r.get(0)?,
            meetings: r.get(1)?,
            last_day: r.get(2)?,
            open: r.get(3)?,
            archived: r.get(4)?,
        })
    })?
    .collect()
}

pub fn is_archived(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM series_archive WHERE name = ?1)",
        [name],
        |r| r.get(0),
    )
}

/// Archive (`on`) or bring back a series; idempotent both ways.
pub fn set_archived(conn: &Connection, name: &str, on: bool) -> rusqlite::Result<()> {
    if on {
        conn.execute(
            "INSERT OR IGNORE INTO series_archive(name) SELECT ?1 WHERE EXISTS (SELECT 1 FROM meetings WHERE name = ?1)",
            [name],
        )?;
    } else {
        conn.execute("DELETE FROM series_archive WHERE name = ?1", [name])?;
    }
    Ok(())
}

/// Open action items logged in any meeting of the series, oldest first.
pub fn series_plans(conn: &Connection, name: &str) -> rusqlite::Result<Vec<Plan>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_PLAN} WHERE meeting_id IN (SELECT id FROM meetings WHERE name = ?1)
           AND kind = 'plan' AND plan_state = 'open' ORDER BY id"
    ))?;
    stmt.query_map([name], crate::db::plan)?.collect()
}

pub fn plans(conn: &Connection, id: i64) -> rusqlite::Result<Vec<Plan>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_PLAN} WHERE meeting_id = ?1 AND kind = 'plan' ORDER BY id"
    ))?;
    stmt.query_map([id], crate::db::plan)?.collect()
}

/// Deletes a meeting; its lines stay (FK sets their meeting_id to NULL). Hands back
/// (name, JSON snapshot incl. the line ids) so Undo can re-attach them.
pub fn delete(conn: &mut Connection, id: i64) -> rusqlite::Result<Option<(String, String)>> {
    let tx = conn.transaction()?;
    let snap = tx
        .query_row(
            "SELECT name, json_object('id', id, 'day', day, 'name', name, 'notes', notes,
               'ended_at', ended_at, 'note_id', note_id,
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
    if running && let Some(name) = running_name(&tx)? {
        return Ok(Err(name));
    }
    tx.execute(
        "INSERT INTO meetings(id, day, name, notes, ended_at, note_id)
         SELECT CASE WHEN EXISTS (SELECT 1 FROM meetings WHERE id = j.id) THEN NULL ELSE j.id END,
                j.day, j.name, j.notes, j.ended_at,
                (SELECT id FROM notes WHERE id = j.note_id)
         FROM (SELECT ?1 ->> 'id' AS id, ?1 ->> 'day' AS day, ?1 ->> 'name' AS name,
                      ?1 ->> 'notes' AS notes, ?1 ->> 'ended_at' AS ended_at,
                      ?1 ->> 'note_id' AS note_id) j",
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

/// A transcript line as stored; field order is the stored JSON key order (the tests assert it).
#[derive(Serialize, Deserialize)]
struct Seg {
    t: f64,
    speaker: String,
    text: String,
}

/// Appends `new` to `have`, skipping a (speaker, t) already there or repeated in the batch. Equality is on
/// `t`'s bits: `tidy` rounded both sides to tenths, so equal times are the same f64.
fn push_new(have: &mut Vec<Seg>, new: &[(f64, String, String)]) {
    let mut seen: HashSet<(String, u64)> = have
        .iter()
        .map(|s| (s.speaker.clone(), s.t.to_bits()))
        .collect();
    for (t, speaker, text) in new {
        // A non-finite t would serialise as null and make the whole blob unreadable.
        if t.is_finite() && seen.insert((speaker.clone(), t.to_bits())) {
            have.push(Seg {
                t: *t,
                speaker: speaker.clone(),
                text: text.clone(),
            });
        }
    }
}

/// The app's segments, one recording. A (speaker, t) already there is skipped, so a retried post
/// (or an overlap window repeating a line) never doubles it. `split` (the speaker split, T56) replaces
/// every line that isn't "me". `done` closes the recording; one that
/// ends with no lines at all is dropped, there is nothing to show.
pub fn append_segments(
    conn: &Connection,
    tid: i64,
    segs: &[(f64, String, String)],
    split: Option<&[(f64, String, String)]>,
    done: bool,
    at: &str,
) -> rusqlite::Result<Append> {
    let tx = conn.unchecked_transaction()?;
    let row: Option<(TxStatus, String)> = tx
        .query_row(
            "SELECT status, segments FROM meeting_transcripts WHERE id = ?1",
            [tid],
            |r| Ok((crate::db::tx_status(r, 0)?, r.get(1)?)),
        )
        .optional()?;
    let blob = match row {
        None => return Ok(Append::NotFound),
        Some((TxStatus::Done | TxStatus::Interrupted, _)) => return Ok(Append::Closed),
        Some((TxStatus::Live | TxStatus::Finishing, blob)) => blob,
    };
    if !segs.is_empty() || split.is_some() {
        // One read, one write: a json_each dedupe per segment plus a blob rewrite each was O(n²) under the
        // global DB lock (0.40 s at 1,200 segments).
        let conv = |e: serde_json::Error| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, e.into())
        };
        let mut have: Vec<Seg> = serde_json::from_str(&blob).map_err(conv)?;
        push_new(&mut have, segs);
        if let Some(split) = split {
            have.retain(|s| s.speaker == "me");
            push_new(&mut have, split);
        }
        tx.execute(
            "UPDATE meeting_transcripts SET segments = ?2 WHERE id = ?1",
            params![tid, serde_json::to_string(&have).map_err(conv)?],
        )?;
    }
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

/// Names a split speaker on one finished recording: every line of `from` becomes `to` (T56). The name lives
/// in each line's `speaker`, so renaming two speakers to one name merges a voice the split cut in two.
pub fn rename_speaker(
    conn: &Connection,
    meeting: i64,
    tid: i64,
    from: &str,
    to: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE meeting_transcripts SET segments = (SELECT json_group_array(
             CASE WHEN value ->> 'speaker' = ?3 THEN json_set(value, '$.speaker', ?4) ELSE json(value) END)
           FROM json_each(segments))
         WHERE id = ?1 AND meeting_id = ?2 AND status IN ('done', 'interrupted')",
        params![tid, meeting, from, to],
    )?;
    Ok(())
}

pub struct Transcript {
    pub id: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: TxStatus,
    pub model: String,
    pub lang: String,
    /// (t, speaker, text), in time order.
    pub segments: Vec<(f64, String, String)>,
}

impl Transcript {
    /// The split voices (T56; not Me, not the unsplit Others), in order of first line.
    pub fn speakers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, sp, _) in &self.segments {
            if sp != "me" && sp != "others" && !out.contains(sp) {
                out.push(sp.clone());
            }
        }
        out
    }
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
                status: crate::db::tx_status(r, 3)?,
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
