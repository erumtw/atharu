//! Reads over `taggings` + the `entry_tags` view (migration 006). Inheritance (epic / series → line)
//! is the view's job, so every count here already includes inherited lines.
use crate::db::SELECT_PLAN;
use crate::domain::{
    entry::{Kind, NoteRef},
    plan::Plan,
    tags,
};
use rusqlite::{Connection, OptionalExtension, params};

/// One row of the Tags list (and `atharu tags`).
pub struct TagRow {
    pub tag: String,
    /// Lines carrying it, typed or inherited.
    pub lines: i64,
    pub open_todos: i64,
    pub notes: i64,
    /// Latest day of a line or note edit (YYYY-MM-DD).
    pub last_used: Option<String>,
    /// Epic keys whose tags field holds it.
    pub tagged_by: Vec<String>,
}

// One CTE pass per tag is fine at one person's scale; the PK on taggings serves every `tag = ?`.
const TAG_ROWS: &str = "
WITH lt AS (SELECT DISTINCT et.tag, e.id, e.day, e.plan_state
              FROM entry_tags et JOIN entries e ON e.id = et.entry_id),
     nt AS (SELECT t.tag, date(n.updated_at, 'localtime') AS day
              FROM taggings t JOIN notes n ON t.item_type = 'note' AND n.id = t.item_id)
SELECT n.tag,
       (SELECT COUNT(*) FROM lt WHERE lt.tag = n.tag),
       (SELECT COUNT(*) FROM lt WHERE lt.tag = n.tag AND lt.plan_state = 'open'),
       (SELECT COUNT(*) FROM nt WHERE nt.tag = n.tag),
       NULLIF(MAX(COALESCE((SELECT MAX(day) FROM lt WHERE lt.tag = n.tag), ''),
                  COALESCE((SELECT MAX(day) FROM nt WHERE nt.tag = n.tag), '')), ''),
       (SELECT group_concat(item_id, ',') FROM
          (SELECT item_id FROM taggings WHERE tag = n.tag AND item_type = 'epic' ORDER BY item_id))
  FROM (SELECT DISTINCT tag FROM taggings) n";

fn tag_row(r: &rusqlite::Row) -> rusqlite::Result<TagRow> {
    let by: Option<String> = r.get(5)?;
    Ok(TagRow {
        tag: r.get(0)?,
        lines: r.get(1)?,
        open_todos: r.get(2)?,
        notes: r.get(3)?,
        last_used: r.get(4)?,
        tagged_by: by
            .map(|s| s.split(',').map(String::from).collect())
            .unwrap_or_default(),
    })
}

/// Every tag in use, most used (lines + notes) first, ties by name.
pub fn all(conn: &Connection) -> rusqlite::Result<Vec<TagRow>> {
    let mut rows: Vec<TagRow> = conn
        .prepare(TAG_ROWS)?
        .query_map([], tag_row)?
        .collect::<Result<_, _>>()?;
    rows.sort_by(|a, b| {
        (b.lines + b.notes)
            .cmp(&(a.lines + a.notes))
            .then(a.tag.cmp(&b.tag))
    });
    Ok(rows)
}

pub fn one(conn: &Connection, tag: &str) -> rusqlite::Result<Option<TagRow>> {
    let mut stmt = conn.prepare(&format!("{TAG_ROWS} WHERE n.tag = ?1"))?;
    let mut rows = stmt.query_map([tag], tag_row)?;
    rows.next().transpose()
}

/// A line on the hub.
pub struct HubLine {
    pub id: i64,
    pub day: String,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    pub plan_state: Option<String>,
    /// Set when the tag is not in the text: the epic key or meeting series it comes from.
    pub via: Option<String>,
    pub author: String,
    /// On a merged hub, why an untagged line is here: `topic line` / `in cms-profile`.
    pub note: Option<String>,
    /// The line's own note (D49), for its note button.
    pub note_ref: Option<NoteRef>,
}

/// Plans by id, for the hub's open to-dos (drawn by `plan_row.html`); order is the caller's.
pub fn plans(conn: &Connection, ids: &[i64]) -> rusqlite::Result<Vec<Plan>> {
    let ids = serde_json::to_string(ids).unwrap_or_else(|_| "[]".into());
    let mut stmt = conn.prepare(&format!(
        "{SELECT_PLAN} WHERE id IN (SELECT value FROM json_each(?1))"
    ))?;
    stmt.query_map([ids], crate::db::plan)?.collect()
}

/// Every line carrying `tag`, newest first. A tag both typed and inherited counts as typed.
pub fn lines(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<HubLine>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.day, e.kind, e.topic, e.text, e.plan_state,
                CASE WHEN MIN(et.via IS NOT NULL) = 1 THEN MAX(et.via) END, e.author, n.id, n.title
           FROM entry_tags et JOIN entries e ON e.id = et.entry_id LEFT JOIN notes n ON n.id = e.note_id
          WHERE et.tag = ?1
          GROUP BY e.id
          ORDER BY e.day DESC, e.id DESC",
    )?;
    stmt.query_map([tag], |r| {
        Ok(HubLine {
            id: r.get(0)?,
            day: r.get(1)?,
            kind: crate::db::kind(r, 2)?,
            topic: r.get(3)?,
            text: r.get(4)?,
            plan_state: r.get(5)?,
            via: r.get(6)?,
            author: r.get(7)?,
            note: None,
            note_ref: NoteRef::from_cols(r.get(8)?, r.get(9)?),
        })
    })?
    .collect()
}

pub struct HubEpic {
    pub key: String,
    pub title: String,
    pub status: String,
    pub done_on: Option<String>,
    /// Its tags field holds the tag (its lines pass it down).
    pub tags_it: bool,
    /// Lines on it carrying the tag (typed or inherited).
    pub lines: i64,
}

/// Epics that tag it, then epics with tagged lines; busiest first.
pub fn epics(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<HubEpic>> {
    let mut stmt = conn.prepare(
        "SELECT ep.key, ep.title, ep.status, ep.done_on,
                EXISTS (SELECT 1 FROM taggings t WHERE t.tag = ?1 AND t.item_type = 'epic' AND t.item_id = ep.key) AS tags_it,
                (SELECT COUNT(DISTINCT et.entry_id) FROM entry_tags et JOIN entries e ON e.id = et.entry_id
                  WHERE et.tag = ?1 AND e.epic_key = ep.key) AS n
           FROM epics ep
          WHERE tags_it OR n > 0
          ORDER BY tags_it DESC, n DESC, ep.key",
    )?;
    stmt.query_map([tag], |r| {
        Ok(HubEpic {
            key: r.get(0)?,
            title: r.get(1)?,
            status: r.get(2)?,
            done_on: r.get(3)?,
            tags_it: r.get(4)?,
            lines: r.get(5)?,
        })
    })?
    .collect()
}

/// Notes whose body has the tag, newest edit first: (id, title, author, updated day).
pub fn notes(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<(i64, String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT n.id, CASE WHEN n.title = '' THEN 'Untitled' ELSE n.title END, n.author, date(n.updated_at, 'localtime')
           FROM taggings t JOIN notes n ON n.id = t.item_id
          WHERE t.tag = ?1 AND t.item_type = 'note'
          ORDER BY n.updated_at DESC, n.id DESC",
    )?;
    stmt.query_map([tag], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect()
}

/// Meetings with lines tagged in their text: (id, name, day, lines).
pub fn meetings(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<(i64, String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.name, m.day, COUNT(*)
           FROM taggings t JOIN entries e ON t.item_type = 'entry' AND e.id = t.item_id
           JOIN meetings m ON m.id = e.meeting_id
          WHERE t.tag = ?1
          GROUP BY m.id ORDER BY m.day DESC, m.id DESC",
    )?;
    stmt.query_map(params![tag], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })?
    .collect()
}

/// Epics, series and topics whose tags field holds it: (item_type, id), epics first.
pub fn tagged_by(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT item_type, item_id FROM taggings
          WHERE tag = ?1 AND item_type IN ('epic', 'series', 'topic')
          ORDER BY CASE item_type WHEN 'epic' THEN 0 WHEN 'series' THEN 1 ELSE 2 END, item_id",
    )?;
    stmt.query_map([tag], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

/// A topic on the hub: tagged with it, or the same name (one name, one hub).
pub struct HubTopic {
    pub slug: String,
    pub own_words: String,
    /// Latest self-rating, 0 = never rated.
    pub depth: u8,
    pub tags_it: bool,
}

pub fn topics(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<HubTopic>> {
    let mut stmt = conn.prepare(
        "SELECT t.slug, t.own_words,
                COALESCE((SELECT depth FROM depth_ratings d WHERE d.topic = t.slug ORDER BY day DESC, rowid DESC LIMIT 1), 0),
                EXISTS (SELECT 1 FROM taggings g WHERE g.tag = ?1 AND g.item_type = 'topic' AND g.item_id = t.slug)
           FROM topics t
          WHERE t.slug = ?1 OR EXISTS (SELECT 1 FROM taggings g WHERE g.tag = ?1 AND g.item_type = 'topic' AND g.item_id = t.slug)
          ORDER BY t.slug = ?1 DESC, t.slug",
    )?;
    stmt.query_map([tag], |r| {
        Ok(HubTopic {
            slug: r.get(0)?,
            own_words: r.get(1)?,
            depth: r.get(2)?,
            tags_it: r.get(3)?,
        })
    })?
    .collect()
}

/// A meeting series on the hub: tagged with it, or the same name. (name, meetings, last day, tags it)
pub fn series(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<(String, i64, String, bool)>> {
    let mut stmt = conn.prepare(
        "SELECT m.name, COUNT(*), MAX(m.day),
                EXISTS (SELECT 1 FROM taggings g WHERE g.tag = ?1 AND g.item_type = 'series' AND g.item_id = m.name)
           FROM meetings m
          WHERE m.name = ?1 OR EXISTS (SELECT 1 FROM taggings g WHERE g.tag = ?1 AND g.item_type = 'series' AND g.item_id = m.name)
          GROUP BY m.name ORDER BY m.name = ?1 DESC, MAX(m.day) DESC",
    )?;
    stmt.query_map([tag], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect()
}

/// Lines a same-named topic or series brings to a merged hub (one name, one hub), beyond the
/// tagged ones: `?` lines on the topic and lines logged in the series' meetings.
pub fn merged_lines(conn: &Connection, name: &str) -> rusqlite::Result<Vec<HubLine>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.day, e.kind, e.topic, e.text, e.plan_state,
                CASE WHEN e.kind = 'learn' AND e.topic = ?1 THEN 'topic line' ELSE 'in ' || m.name END,
                e.author, n.id, n.title
           FROM entries e LEFT JOIN meetings m ON m.id = e.meeting_id LEFT JOIN notes n ON n.id = e.note_id
          WHERE (e.kind = 'learn' AND e.topic = ?1) OR m.name = ?1",
    )?;
    stmt.query_map([name], |r| {
        Ok(HubLine {
            id: r.get(0)?,
            day: r.get(1)?,
            kind: crate::db::kind(r, 2)?,
            topic: r.get(3)?,
            text: r.get(4)?,
            plan_state: r.get(5)?,
            via: None,
            author: r.get(7)?,
            note: r.get(6)?,
            note_ref: NoteRef::from_cols(r.get(8)?, r.get(9)?),
        })
    })?
    .collect()
}

/// Own events tagged with it and Google events (filtered by word in the handler) from `from` on:
/// (starts_at, title, own).
pub fn upcoming(
    conn: &Connection,
    tag: &str,
    from: &str,
    to_excl: &str,
) -> rusqlite::Result<Vec<(String, String, bool)>> {
    let mut stmt = conn.prepare(
        "SELECT ev.starts_at, ev.title, ev.source = 'own' FROM events ev
          WHERE ev.starts_at >= ?2 AND ev.starts_at < ?3
            AND (ev.source <> 'own' OR EXISTS (SELECT 1 FROM taggings g
                   WHERE g.tag = ?1 AND g.item_type = 'event' AND g.item_id = CAST(ev.id AS TEXT)))
          ORDER BY ev.starts_at, ev.id",
    )?;
    stmt.query_map(params![tag, from, to_excl], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })?
    .collect()
}

/// One rewritten source: (item_type, id, its text or tags field before). Undo writes it back.
type Snap = Vec<(String, String, String)>;

/// The source of truth behind a tagging: a line's / note's / own event's text, or a tags field
/// (series have no row: their field is their taggings).
fn load(conn: &Connection, ty: &str, id: &str) -> rusqlite::Result<Option<String>> {
    let sql = match ty {
        "entry" => "SELECT text FROM entries WHERE id = ?1",
        // Title + body as one text (title is one line), so rename / delete / Undo reach both (D54).
        "note" => "SELECT title || char(10) || body FROM notes WHERE id = ?1",
        "event" => "SELECT title FROM events WHERE id = ?1",
        "epic" => "SELECT tags FROM epics WHERE key = ?1",
        "topic" => "SELECT tags FROM topics WHERE slug = ?1",
        _ => {
            "SELECT group_concat(tag, ',') FROM taggings WHERE item_type = 'series' AND item_id = ?1"
        }
    };
    conn.query_row(sql, [id], |r| r.get(0)).optional()
}

/// Write a source back and rebuild its taggings; a row deleted since is left alone.
fn store(conn: &Connection, ty: &str, id: &str, v: &str) -> rusqlite::Result<()> {
    let sql = match ty {
        "entry" => "UPDATE entries SET text = ?2 WHERE id = ?1",
        "note" => {
            "UPDATE notes SET title = substr(?2, 1, instr(?2, char(10)) - 1),
                              body = substr(?2, instr(?2, char(10)) + 1) WHERE id = ?1"
        }
        "event" => "UPDATE events SET title = ?2 WHERE id = ?1",
        "epic" => "UPDATE epics SET tags = ?2 WHERE key = ?1",
        "topic" => "UPDATE topics SET tags = ?2 WHERE slug = ?1",
        _ => return crate::db::retag(conn, ty, id, &tags::parse_list(v)),
    };
    if conn.execute(sql, [id, v])? == 0 {
        return Ok(());
    }
    let names = match ty {
        "epic" | "topic" => tags::parse_list(v),
        _ => tags::tags(v),
    };
    crate::db::retag(conn, ty, id, &names)
}

/// Rename / merge (`new` = Some) or delete (None) a tag everywhere it is written, in one
/// transaction. Returns how many sources changed and the snapshot for Undo (JSON).
pub fn rewrite(
    conn: &mut Connection,
    old: &str,
    new: Option<&str>,
) -> rusqlite::Result<(usize, String)> {
    let tx = conn.transaction()?;
    let items: Vec<(String, String)> = tx
        .prepare("SELECT item_type, item_id FROM taggings WHERE tag = ?1")?
        .query_map([old], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut snap: Snap = Vec::new();
    for (ty, id) in items {
        let Some(before) = load(&tx, &ty, &id)? else {
            continue;
        };
        let after = match ty.as_str() {
            "entry" | "note" | "event" => tags::rewrite(&before, old, new),
            _ => {
                let list = tags::parse_list(&before)
                    .into_iter()
                    .filter_map(|t| {
                        if t == old {
                            new.map(String::from)
                        } else {
                            Some(t)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                tags::parse_list(&list).join(",")
            }
        };
        store(&tx, &ty, &id, &after)?;
        snap.push((ty, id, before));
    }
    tx.commit()?;
    Ok((snap.len(), serde_json::to_string(&snap).unwrap_or_default()))
}

/// Undo of a rewrite / "Tag them": every source back as it was, one transaction.
pub fn undo(conn: &mut Connection, snap: &str) -> rusqlite::Result<usize> {
    let snap: Snap = serde_json::from_str(snap).unwrap_or_default();
    let tx = conn.transaction()?;
    for (ty, id, v) in &snap {
        store(&tx, ty, id, v)?;
    }
    tx.commit()?;
    Ok(snap.len())
}

/// Lines other than `except` where a plain word names `tag` ("N older lines mention ‘x’"): (id, text).
// ponytail: scans every line's text in Rust (same_word isn't SQL); an FTS index if logs reach 100k lines.
fn older(
    conn: &Connection,
    tag: &str,
    except: i64,
) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare("SELECT id, text FROM entries WHERE id <> ?1")?;
    let rows = stmt.query_map([except], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (id, text) = r?;
        if let Some(t) = tags::tag_word(&text, tag) {
            out.push((id, text, t));
        }
    }
    Ok(out)
}

/// "Tag them": the word becomes `#tag` in every such line, one transaction, one snapshot.
pub fn tag_older(
    conn: &mut Connection,
    tag: &str,
    except: i64,
) -> rusqlite::Result<(usize, String)> {
    let tx = conn.transaction()?;
    let mut snap: Snap = Vec::new();
    for (id, before, after) in older(&tx, tag, except)? {
        store(&tx, "entry", &id.to_string(), &after)?;
        snap.push(("entry".into(), id.to_string(), before));
    }
    tx.commit()?;
    Ok((snap.len(), serde_json::to_string(&snap).unwrap_or_default()))
}
