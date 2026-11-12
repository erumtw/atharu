//! Reads over `taggings` + the `entry_tags` view (migration 006). Inheritance (epic / series → line)
//! is the view's job, so every count here already includes inherited lines.
use crate::domain::entry::Kind;
use rusqlite::{Connection, params};

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
    pub due: Option<String>,
    /// Set when the tag is not in the text: the epic key or meeting series it comes from.
    pub via: Option<String>,
    /// True when `via` is the line's epic (else a meeting series).
    pub via_epic: bool,
    pub author: String,
}

/// Every line carrying `tag`, newest first. A tag both typed and inherited counts as typed.
pub fn lines(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<HubLine>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.day, e.kind, e.topic, e.text, e.plan_state, e.due,
                CASE WHEN MIN(et.via IS NOT NULL) = 1 THEN MAX(et.via) END, e.epic_key, e.author
           FROM entry_tags et JOIN entries e ON e.id = et.entry_id
          WHERE et.tag = ?1
          GROUP BY e.id
          ORDER BY e.day DESC, e.id DESC",
    )?;
    stmt.query_map([tag], |r| {
        let kind: String = r.get(2)?;
        let via: Option<String> = r.get(7)?;
        let key: Option<String> = r.get(8)?;
        Ok(HubLine {
            id: r.get(0)?,
            day: r.get(1)?,
            kind: Kind::parse(&kind).unwrap_or(Kind::Done),
            topic: r.get(3)?,
            text: r.get(4)?,
            plan_state: r.get(5)?,
            due: r.get(6)?,
            via_epic: via.is_some() && via == key,
            via,
            author: r.get(9)?,
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
