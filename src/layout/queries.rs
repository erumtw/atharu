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
