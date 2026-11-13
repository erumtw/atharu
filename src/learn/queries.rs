use crate::domain::{
    depth::Rating,
    entry::{Kind, LineRow, NoteRef},
};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};

fn rating(day: String, depth: u8) -> Option<Rating> {
    NaiveDate::parse_from_str(&day, "%Y-%m-%d")
        .ok()
        .map(|day| Rating { day, depth })
}

/// Topic slugs from `?` lines that have no topics row yet: (slug, lines).
pub fn new_topics(conn: &Connection) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT e.topic, COUNT(*) FROM entries e
         WHERE e.kind = 'learn' AND e.topic IS NOT NULL AND e.topic != ''
           AND NOT EXISTS (SELECT 1 FROM topics t WHERE t.slug = e.topic)
         GROUP BY e.topic ORDER BY MAX(e.day) DESC, e.topic",
    )?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

/// Days of learn lines with no topic, newest first (the user files them by retyping).
pub fn untopic_days(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT day FROM entries WHERE kind = 'learn' AND (topic IS NULL OR topic = '')
         ORDER BY day DESC, id DESC",
    )?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

pub fn make_topic(conn: &Connection, slug: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT OR IGNORE INTO topics(slug) VALUES (?1)", [slug])?;
    Ok(())
}

pub struct TopicRow {
    pub slug: String,
    pub depth: Option<u8>,
    pub last_rated: Option<NaiveDate>,
    pub lines: i64,
}

/// Every topic with its latest rating; least recently rated first (never rated first of all).
pub fn topics(conn: &Connection) -> rusqlite::Result<Vec<TopicRow>> {
    let mut stmt = conn.prepare(
        "SELECT t.slug,
                (SELECT depth FROM depth_ratings d WHERE d.topic = t.slug
                 ORDER BY d.day DESC, d.rowid DESC LIMIT 1),
                (SELECT MAX(day) FROM depth_ratings d WHERE d.topic = t.slug) AS last,
                (SELECT COUNT(*) FROM entries e WHERE e.kind = 'learn' AND e.topic = t.slug)
         FROM topics t ORDER BY last IS NOT NULL, last, t.slug",
    )?;
    stmt.query_map([], |r| {
        let last: Option<String> = r.get(2)?;
        Ok(TopicRow {
            slug: r.get(0)?,
            depth: r.get(1)?,
            last_rated: last.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            lines: r.get(3)?,
        })
    })?
    .collect()
}

pub struct Topic {
    pub summary: String,
    pub own_words: String,
    pub resources: String,
}

pub fn topic(conn: &Connection, slug: &str) -> rusqlite::Result<Option<Topic>> {
    conn.query_row(
        "SELECT summary, own_words, resources FROM topics WHERE slug = ?1",
        [slug],
        |r| {
            Ok(Topic {
                summary: r.get(0)?,
                own_words: r.get(1)?,
                resources: r.get(2)?,
            })
        },
    )
    .optional()
}

/// Columns the page may write; anything else in the form is ignored.
pub const EDITABLE: [&str; 3] = ["summary", "own_words", "resources"];

/// Writes only the given columns (autosave posts one form at a time); false = unknown slug.
pub fn update_fields(conn: &Connection, slug: &str, f: &[(&str, &str)]) -> rusqlite::Result<bool> {
    if !conn
        .query_row("SELECT 1 FROM topics WHERE slug = ?1", [slug], |_| Ok(()))
        .optional()?
        .is_some()
    {
        return Ok(false);
    }
    for (col, v) in f {
        // `col` comes from EDITABLE, never from the request.
        conn.execute(
            &format!("UPDATE topics SET {col} = ?1 WHERE slug = ?2"),
            params![v, slug],
        )?;
    }
    Ok(true)
}

/// Ratings oldest first.
pub fn ratings(conn: &Connection, slug: &str) -> rusqlite::Result<Vec<Rating>> {
    let mut stmt =
        conn.prepare("SELECT day, depth FROM depth_ratings WHERE topic = ?1 ORDER BY day, rowid")?;
    let rows = stmt.query_map([slug], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (day, depth) = row?;
        out.extend(rating(day, depth));
    }
    Ok(out)
}

/// Appends today's rating; re-rating the same day replaces it (UNIQUE(topic, day), migration 005).
pub fn rate(conn: &Connection, slug: &str, day: &str, depth: u8) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO depth_ratings(topic, day, depth) VALUES (?1, ?2, ?3)
         ON CONFLICT(topic, day) DO UPDATE SET depth = excluded.depth",
        params![slug, day, depth],
    )?;
    Ok(())
}

pub fn unrate(conn: &Connection, slug: &str, day: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM depth_ratings WHERE topic = ?1 AND day = ?2",
        params![slug, day],
    )?;
    Ok(())
}

/// `? slug: …` lines, newest first. The topic is the page itself, so the row leaves it out.
pub fn lines(conn: &Connection, slug: &str) -> rusqlite::Result<Vec<LineRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.day, e.text, e.epic_key, e.author, n.id, n.title
         FROM entries e LEFT JOIN notes n ON n.id = e.note_id
         WHERE e.kind = 'learn' AND e.topic = ?1 ORDER BY e.day DESC, e.id DESC",
    )?;
    stmt.query_map([slug], |r| {
        Ok(LineRow {
            id: r.get(0)?,
            day: r.get(1)?,
            kind: Kind::Learn,
            text: r.get(2)?,
            epic: r.get(3)?,
            author: r.get(4)?,
            note: NoteRef::from_cols(r.get(5)?, r.get(6)?),
            ..LineRow::default()
        })
    })?
    .collect()
}

/// Deletes a topic page and its ratings (FK cascade). Its `?` lines stay and, with no topics row,
/// show in the Learn inbox again. Hands back a JSON snapshot incl. ratings for Undo.
pub fn delete(conn: &mut Connection, slug: &str) -> rusqlite::Result<Option<String>> {
    let tx = conn.transaction()?;
    let snap = tx
        .query_row(
            "SELECT json_object('slug', slug, 'summary', summary, 'own_words', own_words,
               'resources', resources, 'tags', tags,
               'ratings', json((SELECT json_group_array(json_array(d.day, d.depth))
                                FROM depth_ratings d WHERE d.topic = t.slug)))
             FROM topics t WHERE slug = ?1",
            [slug],
            |r| r.get(0),
        )
        .optional()?;
    tx.execute("DELETE FROM topics WHERE slug = ?1", [slug])?;
    // topics has no untag trigger (006 shipped without one): its taggings go in the same tx.
    crate::db::retag(&tx, "topic", slug, &[])?;
    tx.commit()?;
    Ok(snap)
}

/// Undo. `Ok(Err(slug))` = a topic with that slug was made again meanwhile; nothing restored.
pub fn restore(conn: &mut Connection, snap: &str) -> rusqlite::Result<Result<String, String>> {
    let tx = conn.transaction()?;
    let slug: String = tx.query_row("SELECT ?1 ->> 'slug'", [snap], |r| r.get(0))?;
    if topic(&tx, &slug)?.is_some() {
        return Ok(Err(slug));
    }
    tx.execute(
        "INSERT INTO topics(slug, summary, own_words, resources, tags)
         VALUES (?1 ->> 'slug', ?1 ->> 'summary', ?1 ->> 'own_words', ?1 ->> 'resources',
                 COALESCE(?1 ->> 'tags', ''))",
        [snap],
    )?;
    let tags: String = tx.query_row("SELECT COALESCE(?1 ->> 'tags', '')", [snap], |r| r.get(0))?;
    crate::db::retag(&tx, "topic", &slug, &crate::domain::tags::parse_list(&tags))?;
    tx.execute(
        "INSERT INTO depth_ratings(topic, day, depth)
         SELECT ?1 ->> 'slug', value ->> 0, value ->> 1 FROM json_each(?1, '$.ratings')",
        [snap],
    )?;
    tx.commit()?;
    Ok(Ok(slug))
}

/// The topic's Tags prop (stored `a,b`); taggings follows in the same transaction. False = unknown slug.
pub fn set_tags(conn: &Connection, slug: &str, tags: &[String]) -> rusqlite::Result<bool> {
    let tx = conn.unchecked_transaction()?;
    if tx.execute(
        "UPDATE topics SET tags = ?1 WHERE slug = ?2",
        params![tags.join(","), slug],
    )? == 0
    {
        return Ok(false);
    }
    crate::db::retag(&tx, "topic", slug, tags)?;
    tx.commit()?;
    Ok(true)
}

pub fn tags(conn: &Connection, slug: &str) -> rusqlite::Result<String> {
    conn.query_row("SELECT tags FROM topics WHERE slug = ?1", [slug], |r| {
        r.get(0)
    })
    .optional()
    .map(Option::unwrap_or_default)
}

/// Tags in use, most used first: the Tags prop's suggestions.
pub fn tag_names(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT tag FROM taggings GROUP BY tag ORDER BY COUNT(*) DESC, tag")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}
