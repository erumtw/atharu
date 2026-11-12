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

/// The look + glass settings as (key, value) rows; absent keys mean defaults.
pub fn looks(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT key, value FROM settings WHERE key IN ('look', 'glass_all', 'glass_opacity', 'glass_tint')",
    )?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}
