use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, entry::Kind, tags::parse_list},
    error::{AppError, AppResult},
    layout::{Layout, crumb},
    web,
};
use axum::{
    extract::{Path, Query, State},
    response::Response,
};
use chrono::NaiveDate;
use serde::Deserialize;

/// Recent lines shown before the fold.
const RECENT: usize = 8;

#[derive(Deserialize)]
pub struct ListQuery {
    q: Option<String>,
    sort: Option<String>,
}

pub async fn list(State(st): State<AppState>, Query(lq): Query<ListQuery>) -> AppResult<Response> {
    let conn = st.conn();
    let mut rows = queries::all(&conn)?;
    let total = rows.len();
    let q =
        lq.q.unwrap_or_default()
            .trim()
            .trim_start_matches('#')
            .to_lowercase();
    rows.retain(|r| r.tag.contains(&q));
    let sort = lq
        .sort
        .as_deref()
        .filter(|s| matches!(*s, "recent" | "name"))
        .unwrap_or("used");
    match sort {
        "recent" => rows.sort_by(|a, b| b.last_used.cmp(&a.last_used).then(a.tag.cmp(&b.tag))),
        "name" => rows.sort_by(|a, b| a.tag.cmp(&b.tag)),
        _ => {}
    }
    web::html(&view::List {
        layout: Layout::load(&conn, "tags", "Tags", vec![])?,
        rows,
        total,
        q,
        sort: sort.into(),
        today: st.today(),
    })
}

/// `today`, else `Wed 11` (the hub's short day).
fn when(day: &str, today: NaiveDate) -> String {
    match NaiveDate::parse_from_str(day, "%Y-%m-%d") {
        Ok(d) if d == today => "today".into(),
        Ok(d) => d.format("%a %-d").to_string(),
        Err(_) => day.into(),
    }
}

pub async fn hub(State(st): State<AppState>, Path(tag): Path<String>) -> AppResult<Response> {
    // Only a well-formed, folded name has a hub; `/tags/Expense` or `/tags/a_b` is not a page.
    if parse_list(&tag) != [tag.as_str()] {
        return Err(AppError::NotFound);
    }
    let today = st.today();
    let conn = st.conn();
    let row = queries::one(&conn, &tag)?;
    let (mut todos, mut hi, mut recent) = (Vec::new(), Vec::new(), Vec::new());
    for l in queries::lines(&conn, &tag)? {
        let due = l
            .due
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        let open = l.plan_state.as_deref() == Some("open");
        let line = view::Line {
            id: l.id,
            kind: l.kind,
            topic: l.topic,
            text: l.text,
            when: when(&l.day, today),
            via: l.via,
            via_epic: l.via_epic,
            author: l.author,
            due: due.map(dates::label),
            late: open && due.is_some_and(|d| d < today),
        };
        match l.kind {
            Kind::Plan if open => todos.push(line),
            Kind::Decision | Kind::Blocker | Kind::Impact => hi.push(line),
            _ => recent.push(line),
        }
    }
    let older = recent.split_off(recent.len().min(RECENT));
    let notes = queries::notes(&conn, &tag)?
        .into_iter()
        .map(|(id, title, author, day)| (id, title, author, when(&day, today)))
        .collect::<Vec<_>>();
    let mut used = Vec::new();
    if let Some(r) = &row {
        let n = |k: i64, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
        used.push(n(r.lines, "line", "lines"));
        if r.open_todos > 0 {
            used.push(n(r.open_todos, "open to-do", "open to-dos"));
        }
        if r.notes > 0 {
            used.push(n(r.notes, "note", "notes"));
        }
        if let Some(d) = &r.last_used {
            used.push(format!("last used {}", when(d, today)));
        }
    }
    let title = format!("#{tag}");
    web::html(&view::Hub {
        layout: Layout::load(&conn, "tags", title, vec![crumb("Tags", "/tags")])?,
        used: used.join(" · "),
        tagged_by: row.map(|r| r.tagged_by).unwrap_or_default(),
        todos,
        hi,
        recent,
        older,
        epics: queries::epics(&conn, &tag)?,
        notes,
        meetings: queries::meetings(&conn, &tag)?
            .into_iter()
            .map(|(_, name, day, n)| {
                let d = NaiveDate::parse_from_str(&day, "%Y-%m-%d")
                    .map(dates::label)
                    .unwrap_or(day);
                (name, d, n)
            })
            .collect(),
        tag,
    })
}
