use rusqlite::{Connection, params};

/// Keys worth asking Jira about: every epic, plus keys in links on lines (to-dos are lines).
/// Links come back raw; the caller picks the key out with `domain::plan::jira_key`.
pub fn epic_keys_and_links(conn: &Connection) -> rusqlite::Result<(Vec<String>, Vec<String>)> {
    let keys = conn
        .prepare("SELECT key FROM epics")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let links = conn
        .prepare("SELECT DISTINCT link FROM entries WHERE link IS NOT NULL")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok((keys, links))
}

pub struct Issue {
    pub key: String,
    pub summary: String,
    pub status: String,
}

/// Upsert by key in one transaction. Keys Jira didn't return keep their last copy (a forbidden
/// or deleted issue just stops updating; nothing of ours depends on it).
pub fn upsert(conn: &mut Connection, issues: &[Issue]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut up = tx.prepare(
            "INSERT INTO jira_issues(key, summary, status) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET summary = excluded.summary,
               status = excluded.status, fetched_at = datetime('now')",
        )?;
        for i in issues {
            up.execute(params![i.key, i.summary, i.status])?;
        }
    }
    tx.commit()
}
