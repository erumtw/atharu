use crate::domain::entry::NoteRef;
use rusqlite::{Connection, OptionalExtension};

pub fn meeting_name(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT name FROM meetings WHERE id = ?1", [id], |r| {
        r.get(0)
    })
    .optional()
}

pub struct DuePlan {
    pub id: i64,
    pub text: String,
    pub due: String,
    pub note: Option<NoteRef>,
}

/// Open dated to-dos, soonest first: overdue, then today, then upcoming falls out of the date order. The popover fills its rows
/// from the top, so free rows show the soonest upcoming ones; undated to-dos never fill (they are "+N more").
pub fn dated_plans(conn: &Connection) -> rusqlite::Result<Vec<DuePlan>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.text, e.due, n.id, n.title FROM entries e LEFT JOIN notes n ON n.id = e.note_id
         WHERE e.kind = 'plan' AND e.plan_state = 'open' AND e.due IS NOT NULL AND e.due != ''
         ORDER BY e.due, e.id",
    )?;
    stmt.query_map([], |r| {
        Ok(DuePlan {
            id: r.get(0)?,
            text: r.get(1)?,
            due: r.get(2)?,
            note: NoteRef::from_cols(r.get(3)?, r.get(4)?),
        })
    })?
    .collect()
}

/// The line just saved, for "✓ Saved as … · Undo": (kind, epic key).
pub fn entry(conn: &Connection, id: i64) -> rusqlite::Result<Option<(String, Option<String>)>> {
    conn.query_row(
        "SELECT kind, epic_key FROM entries WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}
