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

#[derive(Debug, Default, PartialEq)]
pub struct Issue {
    pub key: String,
    pub summary: String,
    pub status: String,
    /// `Epic`, `Story`, `Task`, … (the team's own type names).
    pub issue_type: String,
    /// `statusCategory.key`: new · indeterminate · done.
    pub status_cat: String,
    pub parent_key: Option<String>,
    pub parent_summary: Option<String>,
    /// `YYYY-MM-DD`.
    pub due: Option<String>,
    pub resolved_on: Option<String>,
    /// `YYYY-MM-DDTHH:MM`.
    pub updated_at: Option<String>,
}

/// Upsert by key in one transaction. Keys Jira didn't return keep their last copy (a forbidden
/// or deleted issue just stops updating; nothing of ours depends on it).
pub fn upsert(conn: &mut Connection, issues: &[Issue]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut up = tx.prepare(
            "INSERT INTO jira_issues(key, summary, status, issue_type, status_cat, parent_key, parent_summary,
                                     due, resolved_on, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(key) DO UPDATE SET summary = excluded.summary, status = excluded.status,
               issue_type = excluded.issue_type, status_cat = excluded.status_cat,
               parent_key = excluded.parent_key, parent_summary = excluded.parent_summary, due = excluded.due,
               resolved_on = excluded.resolved_on, updated_at = excluded.updated_at, fetched_at = datetime('now')",
        )?;
        for i in issues {
            up.execute(params![
                i.key,
                i.summary,
                i.status,
                i.issue_type,
                i.status_cat,
                i.parent_key,
                i.parent_summary,
                i.due,
                i.resolved_on,
                i.updated_at
            ])?;
        }
    }
    tx.commit()
}

/// The assigned-to-me search's answer (D5): exactly these keys carry the flag; everything else loses it.
pub fn mark_assigned(conn: &Connection, keys: &[String]) -> rusqlite::Result<()> {
    let keys = serde_json::to_string(keys).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "UPDATE jira_issues SET assigned_me = (key IN (SELECT value FROM json_each(?1)))",
        [keys],
    )?;
    Ok(())
}

/// D9: Jira refused our token (401 / 403). Stamped with the local time; the next good answer clears it.
pub fn set_auth_failed(conn: &Connection, failed: bool) -> rusqlite::Result<()> {
    if failed {
        conn.execute(
            "INSERT INTO settings(key, value) VALUES ('jira_auth_failed', datetime('now', 'localtime'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )?;
    } else {
        conn.execute("DELETE FROM settings WHERE key = 'jira_auth_failed'", [])?;
    }
    Ok(())
}

/// The cached status name (`None` = not cached).
pub fn status(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT status FROM jira_issues WHERE key = ?1",
        [key],
        |r| r.get(0),
    )
    .optional()
}

/// After a move: the cache shows the new status at once (the next refresh confirms it).
pub fn moved(conn: &Connection, key: &str, status: &str, cat: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE jira_issues SET status = ?2, status_cat = ?3, fetched_at = datetime('now') WHERE key = ?1",
        params![key, status, cat],
    )?;
    Ok(())
}
