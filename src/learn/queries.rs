use crate::domain::depth::Rating;
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

pub fn slugs(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT slug FROM topics ORDER BY slug")?;
    stmt.query_map([], |r| r.get(0))?.collect()
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

/// Appends today's rating; re-rating the same day replaces it (board: one rating per day).
pub fn rate(conn: &Connection, slug: &str, day: &str, depth: u8) -> rusqlite::Result<()> {
    unrate(conn, slug, day)?;
    conn.execute(
        "INSERT INTO depth_ratings(topic, day, depth) VALUES (?1, ?2, ?3)",
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

pub struct Line {
    pub day: String,
    pub text: String,
    pub epic_key: Option<String>,
}

/// `? slug: …` lines, newest first.
pub fn lines(conn: &Connection, slug: &str) -> rusqlite::Result<Vec<Line>> {
    let mut stmt = conn.prepare(
        "SELECT day, text, epic_key FROM entries WHERE kind = 'learn' AND topic = ?1
         ORDER BY day DESC, id DESC",
    )?;
    stmt.query_map([slug], |r| {
        Ok(Line {
            day: r.get(0)?,
            text: r.get(1)?,
            epic_key: r.get(2)?,
        })
    })?
    .collect()
}

/// Notes whose body has `[[slug]]`, newest first: (id, title) for "Mentioned in N notes".
pub fn mentions(conn: &Connection, x: &str) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, CASE WHEN title = '' THEN 'Untitled' ELSE title END FROM notes
         WHERE body LIKE '%[[' || ?1 || ']]%' ORDER BY updated_at DESC, id DESC",
    )?;
    stmt.query_map([x], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}
