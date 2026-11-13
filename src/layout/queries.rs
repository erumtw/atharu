use super::SideEpic;
use rusqlite::Connection;

pub fn active_epics(conn: &Connection) -> rusqlite::Result<Vec<SideEpic>> {
    let mut stmt =
        conn.prepare("SELECT key, title FROM epics WHERE status = 'active' ORDER BY key")?;
    stmt.query_map([], |r| {
        Ok(SideEpic {
            key: r.get(0)?,
            title: r.get(1)?,
        })
    })?
    .collect()
}

pub fn open_plans(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM entries WHERE plan_state = 'open'",
        [],
        |r| r.get(0),
    )
}

/// The look + glass settings as (key, value) rows; absent keys mean defaults.
pub fn looks(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT key, value FROM settings WHERE key IN ('look', 'glass_all', 'glass_opacity', 'glass_tint', 'text_blur')",
    )?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

/// Notes that point at a page, newest edit first, each once (mention beats line beats tag): `[[name]]` in the body,
/// `#name` on the note (taggings), or the note of one of the page's own lines (`lines_sql` selects entry ids).
/// `LIKE` and no link table on purpose (spec: backlinks are a body scan; a few hundred notes).
pub fn refs(
    conn: &Connection,
    name: Option<&str>,
    lines_sql: Option<&str>,
    arg: &dyn rusqlite::ToSql,
) -> rusqlite::Result<Vec<super::Ref>> {
    // ?2 is the lines' argument; a page without lines still binds it, so the statement always takes two.
    let lines = lines_sql.unwrap_or("SELECT NULL WHERE ?2 IS NULL AND 0");
    let sql = format!(
        "WITH hit(id, via) AS (
           SELECT id, 1 FROM notes WHERE ?1 IS NOT NULL AND body LIKE '%[[' || ?1 || ']]%'
           UNION ALL SELECT note_id, 2 FROM entries WHERE note_id IS NOT NULL AND id IN ({lines})
           UNION ALL SELECT CAST(item_id AS INTEGER), 3 FROM taggings
            WHERE ?1 IS NOT NULL AND item_type = 'note' AND tag = lower(?1))
         SELECT n.id, CASE WHEN n.title = '' THEN 'Untitled' ELSE n.title END, n.author, MIN(h.via)
           FROM hit h JOIN notes n ON n.id = h.id
          GROUP BY n.id ORDER BY n.updated_at DESC, n.id DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    stmt.query_map(rusqlite::params![name, arg], |r| {
        Ok(super::Ref {
            id: r.get(0)?,
            title: r.get(1)?,
            author: r.get(2)?,
            via: match r.get::<_, i64>(3)? {
                1 => super::Via::Mention,
                2 => super::Via::Line,
                _ => super::Via::Tag,
            },
        })
    })?
    .collect()
}
