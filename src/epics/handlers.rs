use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, entry::LineRow, flags, impact::impact_line, links, plan::Plan, tags},
    error::{AppError, AppResult},
    layout::{Layout, crumb},
    web,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashMap;

fn date(s: Option<&str>) -> Option<NaiveDate> {
    s.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

fn flag(e: &queries::EpicRow, today: NaiveDate) -> Option<view::Flag> {
    let facts = flags::EpicFacts {
        status: &e.status,
        target: date(e.target.as_deref()),
        last_activity: date(e.last_log.as_deref()),
        open_plans: e.open_plans,
    };
    flags::flag(&facts, today)
        .map(|f| view::Flag {
            class: f.flag.class(),
            label: f.flag.label(),
            reason: f.reason,
        })
        .or_else(|| jira_flag(e))
}

/// 0.2.5: what Jira says that Atharu doesn't, when no flag of our own speaks first. Active epics only.
fn jira_flag(e: &queries::EpicRow) -> Option<view::Flag> {
    if e.status != "active" {
        return None;
    }
    if let (Some(due), Some(target)) = (&e.jira_due, &e.target)
        && !target.is_empty()
        && due != target
    {
        let label = |d: &str| date(Some(d)).map_or_else(|| d.to_string(), dates::label);
        return Some(view::Flag {
            class: "flag stale",
            label: "due dates differ",
            reason: format!("Jira due {} · your target {}", label(due), label(target)),
        });
    }
    // Jira changed after the last line here: worth a line (or a look).
    let moved = e.jira_updated.as_deref().and_then(|u| u.get(..10))?;
    (e.last_log.as_deref().is_none_or(|l| moved > l)).then(|| view::Flag {
        class: "flag stale",
        label: "Jira moved, no log",
        reason: format!(
            "updated in Jira {}",
            date(Some(moved)).map_or_else(|| moved.to_string(), dates::label)
        ),
    })
}

/// The head's one read-only line: `Owner · O1 · target 3 Dec · 20d · last log today` (empty parts left out).
fn fact(e: &queries::EpicRow, today: NaiveDate) -> String {
    let mut parts = vec![
        if e.role == "owner" {
            "Owner"
        } else {
            "Contributor"
        }
        .to_string(),
    ];
    if !e.okr.is_empty() {
        parts.push(e.okr.clone());
    }
    if let Some(t) = date(e.target.as_deref()) {
        parts.push(match (t - today).num_days() {
            0 => format!("target {} · today", view::short(t)),
            d if d < 0 => format!("target {} · {}d late", view::short(t), -d),
            d => format!("target {} · {d}d", view::short(t)),
        });
    }
    parts.push(last_log(e, today).map_or("nothing logged yet".into(), |l| format!("last log {l}")));
    parts.join(" · ")
}

/// `today` / `9d ago`; None = nothing logged on it.
fn last_log(e: &queries::EpicRow, today: NaiveDate) -> Option<String> {
    date(e.last_log.as_deref()).map(|d| match (today - d).num_days() {
        0 => "today".to_string(),
        n => format!("{n}d ago"),
    })
}

/// The ↗ chip: the cached Jira status (or just `Jira`) linking `{base}/browse/KEY`; None while Jira isn't configured.
fn jira_link(st: &AppState, key: &str, status: Option<&str>) -> Option<(String, String)> {
    let base = st.settings.jira_base_url.as_deref()?.trim_end_matches('/');
    links::is_http(base).then(|| {
        (
            status.unwrap_or("Jira").to_string(),
            format!("{base}/browse/{key}"),
        )
    })
}

fn list_row(st: &AppState, e: queries::EpicRow, today: NaiveDate) -> view::ListRow {
    let flag = flag(&e, today);
    let last_log = last_log(&e, today).unwrap_or_else(|| "—".into());
    let impact = [&e.before, &e.after, &e.metric]
        .iter()
        .all(|v| !v.is_empty())
        .then(|| (e.before.clone(), e.after.clone(), e.metric.clone()));
    // D7: no title typed → Jira's is the name, marked; never copied into epics.
    let from_jira = e.title.is_empty() && e.jira_summary.as_deref().is_some_and(|s| !s.is_empty());
    view::ListRow {
        jira: jira_link(st, &e.key, e.jira_status.as_deref()),
        jira_hint: e.jira.filter(|_| e.title.is_empty() && !from_jira),
        from_jira,
        cat: e.jira_cat.unwrap_or_default(),
        child: false,
        title: if from_jira {
            e.jira_summary.unwrap_or_default()
        } else {
            e.title
        },
        key: e.key,
        open: e.open_plans,
        impact,
        last_log,
        flag,
    }
}

/// A list row with its Jira parent (key, Jira's title for it), if any.
type Parented = (Option<(String, String)>, view::ListRow);

/// D4: stories sit under their Jira parent. A parent that is a row here takes its children right under it; one that
/// isn't gets a header row (its Jira title + status + "+ Track as epic"), first in the group. Rows keep key order.
fn by_parent(
    st: &AppState,
    conn: &Connection,
    rows: Vec<Parented>,
) -> AppResult<Vec<view::ListItem>> {
    let here: std::collections::HashSet<String> = rows.iter().map(|(_, r)| r.key.clone()).collect();
    let mut heads: Vec<(String, String, Vec<view::ListRow>)> = Vec::new();
    let mut under: std::collections::HashMap<String, Vec<view::ListRow>> = Default::default();
    let mut top = Vec::new();
    for (parent, mut row) in rows {
        match parent {
            Some((p, title)) => {
                row.child = true;
                if here.contains(&p) {
                    under.entry(p).or_default().push(row);
                } else if let Some(h) = heads.iter_mut().find(|h| h.0 == p) {
                    h.2.push(row);
                } else {
                    heads.push((p, title, vec![row]));
                }
            }
            None => top.push(row),
        }
    }
    let mut items = Vec::new();
    for (key, title, kids) in heads {
        let cached = queries::jira_issue(conn, &key)?;
        let (status, cat) = cached.as_ref().map_or((None, String::new()), |(_, s, c)| {
            (Some(s.as_str()), c.clone())
        });
        items.push(view::ListItem::Head(view::Head {
            jira: jira_link(st, &key, status),
            title: if title.is_empty() {
                cached.map(|c| c.0).unwrap_or_default()
            } else {
                title
            },
            key,
            cat,
        }));
        items.extend(kids.into_iter().map(view::ListItem::Row));
    }
    for row in top {
        let kids = under.remove(&row.key).unwrap_or_default();
        items.push(view::ListItem::Row(row));
        items.extend(kids.into_iter().map(view::ListItem::Row));
    }
    // A child whose parent row is itself a child elsewhere: keep it, at the end.
    items.extend(under.into_values().flatten().map(view::ListItem::Row));
    Ok(items)
}

/// D5/D6: open Jira issues assigned to me. An Epic offers "+ Track as epic"; anything else "+ To-do", filled with the
/// key, Jira's title, its link and due date.
fn assigned(
    st: &AppState,
    conn: &Connection,
    today: NaiveDate,
) -> AppResult<Vec<view::AssignedRow>> {
    Ok(queries::assigned(conn)?
        .into_iter()
        .map(|a| {
            let jira = jira_link(st, &a.key, Some(&a.status));
            let due_date = date(a.due.as_deref());
            let line = format!(
                "{} > {} {}{}",
                a.key,
                a.summary,
                jira.as_ref().map(|(_, u)| u.as_str()).unwrap_or_default(),
                a.due
                    .as_ref()
                    .map(|d| format!(" by {d}"))
                    .unwrap_or_default()
            );
            view::AssignedRow {
                epic: a.issue_type == "Epic",
                due: due_date.map(|d| format!("due {}", crate::domain::plan::quiet(d, today))),
                jira,
                line,
                key: a.key,
                summary: a.summary,
                issue_type: a.issue_type,
                cat: a.cat,
                tracked: a.tracked,
            }
        })
        .collect())
}

#[derive(Deserialize)]
pub struct TrackForm {
    back: Option<String>,
}

/// "+ Track as epic" on an assigned Jira Epic (D6): the epic stub; its name stays Jira's until one is typed (D7).
pub async fn track(
    State(st): State<AppState>,
    Path(key): Path<String>,
    Form(f): Form<TrackForm>,
) -> AppResult<Response> {
    if !crate::domain::entry::is_epic_key(&key) {
        return Err(AppError::NotFound);
    }
    queries::track(&st.conn(), &key)?;
    Ok(web::back(web::safe_back(f.back.as_deref(), "/epics")))
}

#[derive(Deserialize)]
pub struct ListQuery {
    /// `failed` after the Jira Refresh button couldn't reach Jira.
    jira: Option<String>,
}

pub async fn list(State(st): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Response> {
    let today = st.today();
    let conn = st.conn();
    let mut done_month = 0;
    let (mut untitled, mut groups, mut closed) =
        (Vec::new(), Vec::<view::OkrGroup>::new(), Vec::new());
    let mut okr_rows: Vec<(String, Vec<Parented>)> = Vec::new();
    for e in queries::all_epics(&conn)? {
        if e.status != "active" {
            let done = e.status == "done";
            if done
                && date(e.done_on.as_deref())
                    .is_some_and(|d| dates::month_start(d) == dates::month_start(today))
            {
                done_month += 1;
            }
            closed.push(view::Closed {
                chip: match date(e.done_on.as_deref()) {
                    Some(d) if done => format!("done {}", view::short(d)),
                    _ => e.status.clone(),
                },
                key: e.key,
                jira: e.jira.filter(|_| e.title.is_empty()),
                title: e.title,
                done,
            });
        } else if e.title.is_empty() && e.jira_summary.as_deref().is_none_or(str::is_empty) {
            untitled.push(list_row(&st, e, today));
        } else {
            let okr = if e.okr.is_empty() {
                "No OKR".to_string()
            } else {
                e.okr.clone()
            };
            let parent = e
                .jira_parent
                .clone()
                .map(|p| (p, e.jira_parent_summary.clone().unwrap_or_default()));
            let row = (parent, list_row(&st, e, today));
            // Query sorts by OKR, so a new OKR always means a new group.
            match okr_rows.last_mut() {
                Some((o, rows)) if *o == okr => rows.push(row),
                _ => okr_rows.push((okr, vec![row])),
            }
        }
    }
    for (okr, rows) in okr_rows {
        groups.push(view::OkrGroup {
            okr,
            active: rows.len(),
            items: by_parent(&st, &conn, rows)?,
        });
    }
    let active: usize = groups.iter().map(|g| g.active).sum();
    let mut fact = vec![format!("{active} active")];
    if done_month > 0 {
        fact.push(format!("{done_month} done this month"));
    }
    if !untitled.is_empty() {
        fact.push(format!("{} unnamed", untitled.len()));
    }
    web::html(&view::List {
        layout: Layout::load(&conn, "epics", "Epics", vec![])?,
        fact: fact.join(" · "),
        untitled,
        groups,
        closed,
        assigned: assigned(&st, &conn, today)?,
        dead_token: queries::jira_auth_failed(&conn)?.is_some(),
        token_url: crate::jira::TOKEN_URL,
        jira_on: st.settings.jira().is_some(),
        jira_failed: q.jira.as_deref() == Some("failed"),
        jira_done: q.jira.as_deref() == Some("ok"),
        jira_age: queries::jira_age_min(&conn)?.map(|m| match m {
            ..=0 => "just now".to_string(),
            1..=59 => format!("{m} min ago"),
            60..=1439 => format!("{} h ago", m / 60),
            _ => format!("{} d ago", m / 1440),
        }),
    })
}

#[derive(Deserialize)]
pub struct TitleForm {
    title: String,
}

/// Inline "Type a title, press Enter" on `/epics`.
pub async fn set_title(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Form(f): Form<TitleForm>,
) -> AppResult<Response> {
    let title = f.title.trim().to_string();
    if !title.is_empty()
        && !queries::update_fields(&st.conn(), &key, &[("title", Some(title.clone()))])?
    {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back("/epics"));
    }
    web::html(&view::Named { key, title })
}

fn live(
    conn: &Connection,
    e: &queries::EpicRow,
    ev_line: String,
    ev_error: Option<String>,
) -> AppResult<view::Live> {
    let evidence: Vec<view::Evidence> = queries::evidence(conn, &e.key)?
        .into_iter()
        .map(|r| view::Evidence {
            id: r.id,
            url: r.url,
            note: r.note,
            author: r.author,
        })
        .collect();
    let urls: Vec<String> = evidence.iter().map(|v| v.url.clone()).collect();
    let title = if e.title.is_empty() { &e.key } else { &e.title };
    Ok(view::Live {
        key: e.key.clone(),
        impact: impact_line(title, &e.metric, &e.before, &e.after, &urls),
        metric: e.metric.clone(),
        before: e.before.clone(),
        after: e.after.clone(),
        evidence,
        ev_line,
        ev_error,
        suggest: None,
    })
}

fn load(conn: &Connection, key: &str) -> AppResult<queries::EpicRow> {
    queries::epic(conn, key)?.ok_or(AppError::NotFound)
}

fn render_page(
    st: &AppState,
    key: &str,
    save_error: Option<&'static str>,
    ev_line: String,
    ev_error: Option<String>,
) -> AppResult<view::Page> {
    let today = st.today();
    let conn = st.conn();
    let e = load(&conn, key)?;
    let epic_tags = tags::parse_list(&e.tags);
    let mut timeline: Vec<view::DayGroup> = Vec::new();
    let mut last_day = String::new();
    let mut plans: HashMap<i64, Plan> = queries::plans(&conn, key)?
        .into_iter()
        // The page is the epic: its chip on every to-do says nothing.
        .map(|p| {
            (
                p.id,
                Plan {
                    epic_key: None,
                    ..p
                },
            )
        })
        .collect();
    let mut todos: Vec<Plan> = Vec::new();
    for r in queries::timeline(&conn, key)? {
        if plans.get(&r.id).is_some_and(|p| p.is_open()) {
            todos.extend(plans.remove(&r.id));
            continue;
        }
        if r.day != last_day {
            let label = match date(Some(&r.day)) {
                Some(d) if d == today => "Today".to_string(),
                Some(d) => dates::label(d),
                None => r.day.clone(),
            };
            timeline.push(view::DayGroup {
                label,
                rows: Vec::new(),
            });
            last_day = r.day.clone();
        }
        if let Some(g) = timeline.last_mut() {
            g.rows.push(match plans.remove(&r.id) {
                Some(p) => view::Item::Plan(p),
                None => {
                    let own = tags::tags(&r.text);
                    let inh = epic_tags
                        .iter()
                        .filter(|t| !own.contains(t))
                        .cloned()
                        .collect();
                    view::Item::Line(LineRow { inh, ..r })
                }
            });
        }
    }
    let live = live(&conn, &e, ev_line, ev_error)?;
    let since = dates::week_start(today) - chrono::Days::new(49);
    let days: Vec<NaiveDate> = queries::entry_days_since(&conn, key, &dates::iso(since))?
        .iter()
        .filter_map(|d| date(Some(d)))
        .collect();
    let weeks = flags::weekly_counts(&days, today);
    let fact = fact(&e, today);
    let max = weeks.iter().copied().max().unwrap_or(0).max(1);
    let title = if e.title.is_empty() {
        e.jira_summary
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| e.key.clone())
    } else {
        e.title.clone()
    };
    // D2: Jira resolved it while it's still active here, unless "Not yet" was said for this very date.
    let jira_done = match (&e.jira_cat, &e.jira_resolved) {
        (Some(c), Some(r)) if c == "done" && e.status == "active" => {
            let hidden = queries::jira_not_yet(&conn, key)?;
            (hidden.as_deref() != Some(r.as_str()))
                .then(|| date(Some(r)).map_or_else(|| r.clone(), dates::label))
        }
        _ => None,
    };
    let tickets = queries::children(&conn, key)?
        .into_iter()
        .map(|c| {
            let jira = jira_link(st, &c.key, Some(&c.status));
            view::Ticket {
                href: if c.tracked {
                    format!("/epics/{}", c.key)
                } else {
                    jira.as_ref().map(|(_, u)| u.clone()).unwrap_or_default()
                },
                jira,
                key: c.key,
                summary: c.summary,
                issue_type: c.issue_type,
                cat: c.cat,
                lines: c.lines,
            }
        })
        .collect();
    // D3: a story's head says what it sits under — its page here when tracked, else Jira.
    let parent = e.jira_parent.clone().map(|p| {
        let href = if queries::epic(&conn, &p).ok().flatten().is_some() {
            format!("/epics/{p}")
        } else {
            jira_link(st, &p, None).map(|(_, u)| u).unwrap_or_default()
        };
        (p, e.jira_parent_summary.clone().unwrap_or_default(), href)
    });
    Ok(view::Page {
        jira_done,
        tickets,
        layout: Layout::load(&conn, "epics", title, vec![crumb("Epics", "/epics")])?,
        flag: flag(&e, today),
        activity: weeks.iter().map(|n| n * 100 / max).collect(),
        epic: view::EpicView {
            jira_link: jira_link(st, &e.key, e.jira_status.as_deref()),
            key: e.key,
            jira: e.jira.filter(|_| e.title.is_empty()),
            title: e.title,
            role: e.role,
            okr: e.okr,
            status: e.status,
            start: e.start.unwrap_or_default(),
            target: e.target.unwrap_or_default(),
            done_on: e.done_on.unwrap_or_default(),
            kind: e
                .jira_type
                .clone()
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "Epic".into()),
            parent,
            cat: e.jira_cat.clone().unwrap_or_default(),
        },
        field: tag_field(&conn, epic_tags)?,
        refs: crate::layout::refs(&conn, Some(key), Some(crate::layout::Lines::Epic(key)))?,
        ref_name: key.to_string(),
        live,
        todos,
        timeline,
        fact,
        today,
        save_error,
        delete_error: None,
    })
}

fn tag_field(conn: &Connection, tags: Vec<String>) -> rusqlite::Result<crate::layout::TagField> {
    Ok(crate::layout::TagField {
        id: "tags",
        tags,
        known: queries::tag_names(conn)?,
    })
}

pub async fn page(State(st): State<AppState>, Path(key): Path<String>) -> AppResult<Response> {
    web::html(&render_page(&st, &key, None, String::new(), None)?)
}

/// Turns submitted fields into column writes; `None` = some value is invalid (form can't produce it).
fn fields(form: &HashMap<String, String>) -> Option<Vec<(&'static str, Option<String>)>> {
    let mut out = Vec::new();
    for col in queries::EDITABLE {
        let Some(raw) = form.get(col) else { continue };
        let v = raw.trim();
        let val = match col {
            "role" if !["owner", "contributor"].contains(&v) => return None,
            "status" if !["active", "done", "dropped"].contains(&v) => return None,
            "start" | "target" | "done_on" if v.is_empty() => None,
            "start" | "target" | "done_on" => {
                NaiveDate::parse_from_str(v, "%Y-%m-%d").ok()?;
                Some(v.to_string())
            }
            // Free typing (`#Expense, hiring`) is folded to the stored `expense,hiring`; junk is dropped.
            "tags" => Some(crate::domain::tags::parse_list(v).join(",")),
            _ => Some(v.to_string()),
        };
        out.push((col, val));
    }
    Some(out)
}

const SAVE_FAILED: &str = "Couldn't save. Try again.";
const TARGET_BEFORE_START: &str = "Target can't be before the start.";

/// A column's value after this save: the submitted one, else the stored one (a field not in the form keeps it).
fn pick(
    f: &[(&'static str, Option<String>)],
    col: &str,
    stored: &Option<String>,
) -> Option<String> {
    f.iter()
        .find(|(c, _)| *c == col)
        .map_or(stored.clone(), |(_, v)| v.clone())
}

/// Target before start after this save. The date inputs carry min/max so the picker greys those days, but a
/// typed or scripted value still lands here.
fn backwards(e: &queries::EpicRow, f: &[(&'static str, Option<String>)]) -> bool {
    // ISO dates compare as strings.
    matches!((pick(f, "start", &e.start), pick(f, "target", &e.target)), (Some(s), Some(t)) if t < s)
}

/// Done without a date drops out of the report (it counts `done_on`), so the save that MARKS it done stamps today.
/// Only on that change: a later edit of an epic already done (e.g. its "after" metric) must not date it this week.
/// An existing date is never touched. True when it stamped.
fn stamp_done(
    e: &queries::EpicRow,
    f: &mut Vec<(&'static str, Option<String>)>,
    today: NaiveDate,
) -> bool {
    let done =
        e.status != "done" && pick(f, "status", &Some(e.status.clone())).as_deref() == Some("done");
    if !done || pick(f, "done_on", &e.done_on).is_some_and(|d| !d.is_empty()) {
        return false;
    }
    f.retain(|(c, _)| *c != "done_on");
    f.push(("done_on", Some(today.to_string())));
    true
}

/// Autosave: htmx PATCHes the form on every change; without JS the same form POSTs.
pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Form(mut form): Form<HashMap<String, String>>,
) -> AppResult<Response> {
    // A chip edit (× or the add box) folds into the plain `tags` field before validation.
    if form.contains_key("tags_add") || form.contains_key("tags_drop") {
        let cur = form.get("tags").cloned().unwrap_or_default();
        let list = tags::edit_list(
            &cur,
            form.get("tags_add").map(String::as_str),
            form.get("tags_drop").map(String::as_str),
        );
        form.insert("tags".into(), list.join(","));
    }
    let mut stamped = false;
    let mut was_active = false;
    let error = match fields(&form) {
        Some(mut f) => {
            let conn = st.conn();
            let cur = load(&conn, &key)?;
            was_active = cur.status == "active";
            if backwards(&cur, &f) {
                Some(TARGET_BEFORE_START)
            } else {
                stamped = stamp_done(&cur, &mut f, st.today());
                if !queries::update_fields(&conn, &key, &f)? {
                    return Err(AppError::NotFound);
                }
                None
            }
        }
        None => Some(SAVE_FAILED),
    };
    let ok = error.is_none();
    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    if !web::is_htmx(&headers) {
        if ok {
            return Ok(web::back(&format!("/epics/{key}")));
        }
        return web::html_status(status, &render_page(&st, &key, error, String::new(), None)?);
    }
    let conn = st.conn();
    let e = load(&conn, &key)?;
    let mut lv = live(&conn, &e, String::new(), None)?;
    // D2 / D10: just made done or dropped, and Jira knows the key: offer the matching Jira move.
    if ok
        && was_active
        && matches!(e.status.as_str(), "done" | "dropped")
        && e.jira_status.is_some()
        && st.settings.jira().is_some()
    {
        lv.suggest = Some(if e.status == "done" {
            "done"
        } else {
            "dropped"
        });
    }
    web::html_status(
        status,
        &view::Swap {
            live: lv,
            save_error: error,
            field: Some(tag_field(&conn, tags::parse_list(&e.tags))?),
            // The "done on" input isn't in the swapped block: show the date the save just filled in.
            done_on: stamped.then(|| e.done_on.clone().unwrap_or_default()),
        },
    )
}

#[derive(Deserialize)]
pub struct JiraDoneForm {
    act: String,
}

/// The D2 banner's buttons: "Mark done" (Jira's resolution date becomes done on) or "Not yet" (hidden until that
/// date changes). Plain form, 303 back to the page.
pub async fn jira_done(
    State(st): State<AppState>,
    Path(key): Path<String>,
    Form(f): Form<JiraDoneForm>,
) -> AppResult<Response> {
    let conn = st.conn();
    let e = load(&conn, &key)?;
    if let Some(r) = e.jira_resolved.as_deref() {
        match f.act.as_str() {
            "done" => queries::mark_done(&conn, &key, r)?,
            _ => queries::set_jira_not_yet(&conn, &key, r)?,
        }
    }
    Ok(web::back(&format!("/epics/{key}")))
}

#[derive(Deserialize)]
pub struct EvidenceForm {
    line: String,
}

/// "paste a link, then a note after a space".
pub async fn add_evidence(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Form(f): Form<EvidenceForm>,
) -> AppResult<Response> {
    let line = f.line.trim();
    let (url, note) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let error = (!links::is_http(url)).then(|| links::NOT_HTTP.to_string());
    if error.is_none() {
        queries::add_evidence(&st.conn(), &key, url, note.trim(), "me")?;
    }
    let (status, kept) = match &error {
        Some(_) => (StatusCode::UNPROCESSABLE_ENTITY, line.to_string()),
        None => (StatusCode::OK, String::new()),
    };
    if !web::is_htmx(&headers) {
        if error.is_none() {
            return Ok(web::back(&format!("/epics/{key}")));
        }
        return web::html_status(status, &render_page(&st, &key, None, kept, error)?);
    }
    let conn = st.conn();
    let e = load(&conn, &key)?;
    web::html_status(
        status,
        &view::Swap {
            live: live(&conn, &e, kept, error)?,
            save_error: None,
            field: None,
            done_on: None,
        },
    )
}

pub async fn delete_evidence(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((key, id)): Path<(String, i64)>,
) -> AppResult<Response> {
    if !queries::delete_evidence(&st.conn(), &key, id)? {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/epics/{key}")));
    }
    let conn = st.conn();
    let e = load(&conn, &key)?;
    web::html(&view::Swap {
        live: live(&conn, &e, String::new(), None)?,
        save_error: None,
        field: None,
        done_on: None,
    })
}

const HAS_LINES: &str = "Mark it Done or Dropped instead.";

/// Only an epic with no lines can go (lines need their key); evidence goes with it, Undo brings both back.
pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> AppResult<Response> {
    let res = queries::delete(&mut st.conn(), &key)?;
    match res {
        queries::Deleted::Missing => Err(AppError::NotFound),
        queries::Deleted::HasLines if web::is_htmx(&headers) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            [("hx-retarget", "#epic-del"), ("hx-reswap", "innerHTML")],
            HAS_LINES,
        )
            .into_response()),
        queries::Deleted::HasLines => {
            let mut page = render_page(&st, &key, None, String::new(), None)?;
            page.delete_error = Some(HAS_LINES);
            web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &page)
        }
        queries::Deleted::Gone(_) if !web::is_htmx(&headers) => Ok(web::back("/epics")),
        queries::Deleted::Gone(snap) => web::html(&deleted(&key, snap, None)),
    }
}

fn deleted(key: &str, snap: String, error: Option<String>) -> web::Deleted {
    web::Deleted {
        what: format!("Epic {key}"),
        restore: format!("/epics/{key}/restore"),
        snap,
        back: "/epics",
        back_label: "All epics",
        error,
    }
}

#[derive(Deserialize)]
pub struct SnapForm {
    snap: String,
}

pub async fn restore(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<SnapForm>,
) -> AppResult<Response> {
    let res = queries::restore(&mut st.conn(), &f.snap)?;
    match res {
        Ok(key) => Ok(web::restored(&headers, &format!("/epics/{key}"))),
        Err(key) => web::html_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            &deleted(
                &key,
                f.snap,
                Some(format!(
                    "{key} is back from a new line, so the old one can't return."
                )),
            ),
        ),
    }
}
