//! `athar dump`: plain-SQL backup without needing the sqlite3 CLI (not in the Docker image).
use rusqlite::{Connection, types::ValueRef};
use std::{fmt::Write as _, path::Path};

pub fn write(conn: &Connection, out: &Path) -> anyhow::Result<()> {
    std::fs::write(out, to_sql(conn)?)?;
    Ok(())
}

pub fn to_sql(conn: &Connection) -> rusqlite::Result<String> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let mut s = String::from("PRAGMA foreign_keys=OFF;\nBEGIN TRANSACTION;\n");
    let mut stmt = conn.prepare(
        "SELECT type, name, sql FROM sqlite_master
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
         ORDER BY type = 'index', rowid",
    )?;
    let objects: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (kind, name, sql) in &objects {
        let _ = writeln!(s, "{sql};");
        if kind == "table" {
            dump_rows(conn, name, &mut s)?;
        }
    }
    let _ = writeln!(s, "PRAGMA user_version={version};\nCOMMIT;");
    Ok(s)
}

fn dump_rows(conn: &Connection, table: &str, s: &mut String) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("SELECT * FROM \"{table}\""))?;
    let cols = stmt.column_count();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let mut vals = Vec::with_capacity(cols);
        for i in 0..cols {
            vals.push(match row.get_ref(i)? {
                ValueRef::Null => "NULL".to_string(),
                ValueRef::Integer(n) => n.to_string(),
                ValueRef::Real(f) => f.to_string(),
                ValueRef::Text(t) => {
                    format!("'{}'", String::from_utf8_lossy(t).replace('\'', "''"))
                }
                ValueRef::Blob(b) => format!(
                    "X'{}'",
                    b.iter().map(|x| format!("{x:02X}")).collect::<String>()
                ),
            });
        }
        let _ = writeln!(s, "INSERT INTO \"{table}\" VALUES({});", vals.join(","));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn dump_round_trips_into_a_fresh_db() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO epics(key, title) VALUES ('OPS-1', 'it''s')",
            [],
        )
        .unwrap();
        let sql = super::to_sql(&conn).unwrap();
        let fresh = rusqlite::Connection::open_in_memory().unwrap();
        fresh.execute_batch(&sql).unwrap();
        let title: String = fresh
            .query_row("SELECT title FROM epics", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "it's");
    }
}
