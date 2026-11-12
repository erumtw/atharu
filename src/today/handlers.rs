use super::{queries, view};
use crate::{
    app::AppState,
    domain::{
        dates,
        flags::{self, Flag},
        markdown,
        parse::{by_as_text, parse, with_kind},
        tags,
    },
    error::{AppError, AppResult},
    layout::{Layout, TagFilter},
    web,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DayQuery {
    day: Option<String>,
    /// No-JS inline edit: this line shows as its edit box.
    edit: Option<i64>,
    /// `tag` = group the log by tag (D29); anything else = by epic.
    group: Option<String>,
    /// Tag filter, comma list (any of them); `add` = the filter row's add box.
    tag: Option<String>,
    add: Option<String>,
}

#[derive(Deserialize)]
pub struct LineForm {
    line: String,
    day: Option<String>,
    /// Words the capture bar was told are not tags (× on "Tag as"), comma list.
    notag: Option<String>,
    /// Inline edit via "Make tag": the word that just became a tag.
    made: Option<String>,
    /// Quick popover kind tile: the prefix to use when the line has none (`domain::parse::with_kind`).
    kind: Option<String>,
    /// Set by a capture box on another page (/quick): saving answers with a 303 there, htmx or not, so that page
    /// re-renders itself instead of receiving Today's log partial.
    back: Option<String>,
    /// `1` = edited from a to-do row (D51): the row's list re-fetches itself (`plans-changed`), not Today's log.
    plan: Option<String>,
}

/// How the log is shown: grouping + tag filter.
struct Opts {
    group: &'static str,
    tags: Vec<String>,
}

impl Opts {
    fn new(group: Option<&str>, tag: Option<&str>, add: Option<&str>) -> Opts {
        Opts {
            group: if group == Some("tag") { "tag" } else { "epic" },
            tags: TagFilter::pick(tag, add),
        }
    }

    /// htmx swaps come from a page that may be filtered or grouped: keep that view
    /// (HX-Current-URL is the page's URL), rather than snapping back to the default.
    fn from_page(headers: &HeaderMap) -> Opts {
        let get = |k| web::page_param(headers, k);
        Opts::new(get("group").as_deref(), get("tag").as_deref(), None)
    }
}

/// A bad or missing `day` means today: the date picker never produces one, a hand-typed URL might.
fn pick_day(day: Option<&str>, today: NaiveDate) -> NaiveDate {
    day.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or(today)
}

fn groups(conn: &Connection, day: NaiveDate, o: &Opts) -> rusqlite::Result<Vec<view::Group>> {
    let rows = queries::day_entries(conn, &dates::iso(day), &o.tags)?;
    if o.group == "tag" {
        return Ok(by_tag(rows));
    }
    let mut out: Vec<view::Group> = Vec::new();
    for r in rows {
        let row = view::Row {
            id: r.id,
            kind: r.kind,
            topic: r.topic,
            text: r.text,
            meeting: r.meeting,
            author: r.author,
            via: None,
            epic: None,
            note: r.note,
        };
        // Rows arrive sorted by group, so a new key always means a new group.
        match out.last_mut() {
            Some(g) if g.key == r.epic_key => g.rows.push(row),
            _ => out.push(view::Group {
                key: r.epic_key,
                title: r.epic_title,
                rows: vec![row],
                by_tag: false,
                tag: None,
            }),
        }
    }
    Ok(out)
}

/// One group per tag, biggest first, "No tag" last; a line with two tags shows under both.
fn by_tag(rows: Vec<queries::LoggedRow>) -> Vec<view::Group> {
    let mut out: Vec<view::Group> = Vec::new();
    for r in rows {
        let mut names: Vec<(Option<String>, Option<String>)> = r
            .tags
            .iter()
            .map(|(t, v)| (Some(t.clone()), v.clone()))
            .collect();
        if names.is_empty() {
            names.push((None, None));
        }
        for (tag, via) in names {
            let row = view::Row {
                id: r.id,
                kind: r.kind,
                topic: r.topic.clone(),
                text: r.text.clone(),
                meeting: r.meeting.clone(),
                author: r.author.clone(),
                // The epic chip already says where an epic-inherited tag comes from.
                epic: r.epic_key.clone().filter(|k| via.as_ref() != Some(k)),
                via,
                note: r.note.clone(),
            };
            match out.iter_mut().find(|g| g.tag == tag) {
                Some(g) => g.rows.push(row),
                None => out.push(view::Group {
                    key: None,
                    title: String::new(),
                    rows: vec![row],
                    by_tag: true,
                    tag,
                }),
            }
        }
    }
    out.sort_by(|a, b| {
        (a.tag.is_none(), std::cmp::Reverse(a.rows.len()), &a.tag).cmp(&(
            b.tag.is_none(),
            std::cmp::Reverse(b.rows.len()),
            &b.tag,
        ))
    });
    out
}

fn plans(conn: &Connection, today: NaiveDate) -> rusqlite::Result<view::Plans> {
    let (pinned, other) = queries::plans(conn, &dates::iso(dates::week_start(today)))?
        .into_iter()
        .partition(|p| p.pinned);
    Ok(view::Plans { pinned, other })
}

/// Flagged epics (worst first), then overdue to-dos.
fn needs(
    conn: &Connection,
    plans: &view::Plans,
    today: NaiveDate,
) -> rusqlite::Result<Vec<view::Need>> {
    let mut out: Vec<(Flag, view::Need)> = Vec::new();
    for e in queries::active_epic_facts(conn)? {
        let facts = flags::EpicFacts {
            status: "active",
            target: e.target,
            last_activity: e.last_activity,
            open_plans: e.open_plans,
        };
        if let Some(f) = flags::flag(&facts, today) {
            let text = if e.title.is_empty() {
                f.reason
            } else {
                format!("{} · {}", e.title, f.reason)
            };
            out.push((
                f.flag,
                view::Need {
                    class: f.flag.class(),
                    label: f.flag.label(),
                    href: format!("/epics/{}", e.key),
                    key: Some(e.key),
                    text,
                },
            ));
        }
    }
    out.sort_by_key(|(f, _)| *f as u8);
    let mut needs: Vec<view::Need> = out.into_iter().map(|(_, n)| n).collect();
    for p in plans.pinned.iter().chain(&plans.other) {
        if p.is_late(&today) {
            needs.push(view::Need {
                class: Flag::Overdue.class(),
                label: "overdue",
                key: p.epic_key.clone(),
                text: format!("{} · was due {}", p.text, p.due_label()),
                href: "/todos".into(),
            });
        }
    }
    Ok(needs)
}

fn render_page(
    st: &AppState,
    day: NaiveDate,
    o: Opts,
    line: String,
    error: Option<String>,
) -> AppResult<view::Page> {
    let conn = st.conn();
    let plans = plans(&conn, st.today())?;
    let mut path = format!("/?day={}&", dates::iso(day));
    let mut keep = vec![("day", dates::iso(day))];
    if o.group == "tag" {
        path.push_str("group=tag&");
        keep.push(("group", "tag".into()));
    }
    Ok(view::Page {
        layout: Layout::load(&conn, "today", "Today", vec![])?,
        banner: view::Banner {
            active: queries::active_meeting(&conn)?
                .map(|(id, name)| view::ActiveMeeting { id, name }),
            series: queries::meeting_series(&conn)?,
            name: String::new(),
            error: None,
            back: format!("/?day={}", dates::iso(day)),
            rec: crate::recording::Rec::load(&conn, st.mac.as_deref())?,
        },
        today: st.today(),
        needs: needs(&conn, &plans, st.today())?,
        plans,
        day: view::Day::new(day, st.today()),
        groups: groups(&conn, day, &o)?,
        keys: queries::epic_keys(&conn)?,
        line,
        error,
        preview: None,
        saved: None,
        deleted: None,
        made: None,
        edit: None,
        ask: None,
        group: o.group,
        filter: TagFilter {
            tags: o.tags,
            path,
            keep,
            known: queries::known_tags(&conn)?
                .into_iter()
                .map(|(t, _)| t)
                .collect(),
        },
    })
}

pub async fn page(State(st): State<AppState>, Query(q): Query<DayQuery>) -> AppResult<Response> {
    let day = pick_day(q.day.as_deref(), st.today());
    let o = Opts::new(q.group.as_deref(), q.tag.as_deref(), q.add.as_deref());
    let mut page = render_page(&st, day, o, String::new(), None)?;
    if let Some(id) = q.edit {
        page.edit = edit_box(&st.conn(), id, None, None, false)?;
    }
    web::html(&page)
}

/// The edit box for line `id`: its current text, or `kept` (rejected input) with `error`.
fn edit_box(
    conn: &Connection,
    id: i64,
    kept: Option<String>,
    error: Option<String>,
    plan: bool,
) -> AppResult<Option<view::Edit>> {
    Ok(
        queries::entry_line(conn, id)?.map(|(day, line)| view::Edit {
            id,
            day,
            line: kept.unwrap_or(line),
            error,
            plan,
        }),
    )
}

#[derive(Deserialize)]
pub struct EditQuery {
    plan: Option<String>,
}

/// `GET /entries/{id}/edit`: htmx swaps the row (a log line, or a to-do row with `?plan=1`) for its edit box.
pub async fn edit_form(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<EditQuery>,
) -> AppResult<Response> {
    let plan = q.plan.as_deref() == Some("1");
    let e = edit_box(&st.conn(), id, None, None, plan)?.ok_or(AppError::NotFound)?;
    web::html(&e)
}

/// After a to-do row's edit or Undo (D51): no swap of its own; every to-do list on the page re-fetches.
fn plans_changed() -> Response {
    ([("hx-trigger", "plans-changed")], StatusCode::OK).into_response()
}

/// Save an inline edit: same parser and error copy as the capture bar.
pub async fn edit(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<LineForm>,
) -> AppResult<Response> {
    let today = st.today();
    let plan_row = f.plan.as_deref() == Some("1");
    let entry = match parse(&f.line, today) {
        Ok(e) => e,
        Err(err) => {
            let msg = Some(err.to_string());
            if web::is_htmx(&headers) {
                let e = edit_box(&st.conn(), id, Some(f.line), msg, plan_row)?
                    .ok_or(AppError::NotFound)?;
                let target = format!("#{}-{id}", if plan_row { "plan" } else { "entry" });
                let retarget = [("hx-retarget", target.as_str()), ("hx-reswap", "outerHTML")];
                return Ok((
                    retarget,
                    web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &e)?,
                )
                    .into_response());
            }
            let e =
                edit_box(&st.conn(), id, Some(f.line), msg, false)?.ok_or(AppError::NotFound)?;
            let day = pick_day(Some(&e.day), today);
            let mut page = render_page(&st, day, Opts::new(None, None, None), String::new(), None)?;
            page.edit = Some(e);
            return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &page);
        }
    };
    let before = queries::entry_line(&st.conn(), id)?;
    let day = queries::update_entry(&mut st.conn(), id, &entry)?.ok_or(AppError::NotFound)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/?day={day}")));
    }
    if plan_row {
        return Ok(plans_changed());
    }
    let d = pick_day(Some(&day), today);
    let conn = st.conn();
    let typed = tags::tags(&entry.text);
    let made = match (f.made, before) {
        (Some(word), Some((_, undo_line))) => match tags::parse_list(&word).pop() {
            Some(tag) if typed.contains(&tag) => Some(view::Made {
                id,
                older: queries::older_mentions(&conn, &tag, id)?,
                tag,
                word,
                undo_line,
                day: day.clone(),
            }),
            _ => None,
        },
        _ => None,
    };
    web::html(&view::Swap {
        today,
        plans: plans(&conn, today)?,
        day: view::Day::new(d, today),
        groups: groups(&conn, d, &Opts::from_page(&headers))?,
        saved: None,
        deleted: None,
        made,
        edit: None,
    })
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<LineForm>,
) -> AppResult<Response> {
    // `n …` in the popover's box is a quick note (D55); quick.ts switches in place, so this is the no-JS path. A 307
    // re-sends the same POST to the notes slice, which owns note saving (features don't import each other).
    if f.back.as_deref().is_some_and(|b| b.starts_with("/quick"))
        && markdown::quick_note(&f.line).is_some()
    {
        return Ok((
            StatusCode::TEMPORARY_REDIRECT,
            [(header::LOCATION, "/notes/new")],
        )
            .into_response());
    }
    let today = st.today();
    let day = pick_day(f.day.as_deref(), today);
    let entry = match parse(&with_kind(&f.line, f.kind.as_deref()), today) {
        Ok(e) => e,
        Err(e) if web::is_htmx(&headers) => {
            // Form keeps its text (htmx only resets on 2xx); only the hint under it changes.
            let hint = view::Hint {
                error: Some(e.to_string()),
                preview: None,
                ask: None,
            };
            let retarget = [("hx-retarget", "#hint"), ("hx-reswap", "innerHTML")];
            return Ok((
                retarget,
                web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &hint)?,
            )
                .into_response());
        }
        Err(e) => {
            let page = render_page(
                &st,
                day,
                Opts::new(None, None, None),
                f.line,
                Some(e.to_string()),
            )?;
            return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &page);
        }
    };

    let (id, new_epic) = queries::insert_entry(&mut st.conn(), &dates::iso(day), &entry, "me")?;
    if let Some(b) = f.back.as_deref() {
        let to = web::safe_back(Some(b), "/");
        let sep = if to.contains('?') { '&' } else { '?' };
        return Ok(web::back(&format!("{to}{sep}saved={id}")));
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/?day={}", dates::iso(day))));
    }
    // One guard per statement: two `st.conn()` temporaries in one expression deadlock the mutex.
    let conn = st.conn();
    let swap = view::Swap {
        today,
        plans: plans(&conn, today)?,
        day: view::Day::new(day, today),
        groups: groups(&conn, day, &Opts::from_page(&headers))?,
        saved: Some(view::Saved {
            id,
            kind: entry.kind.as_str(),
            key: entry.epic_key,
            new_epic,
            tags: tags::tags(&entry.text),
        }),
        deleted: None,
        made: None,
        edit: None,
    };
    web::html(&swap)
}

/// `DELETE /entries/{id}` from htmx, `POST /entries/{id}/delete` from a plain form.
pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<BackQuery>,
) -> AppResult<Response> {
    let (day, snap) = queries::delete_entry(&st.conn(), id)?.ok_or(AppError::NotFound)?;
    st.line_gone(&snap);
    // In the query, not the body: htmx sends DELETE without one. Same rule as `back` on create.
    if web::is_htmx(&headers) && q.plan.as_deref() == Some("1") {
        // No plans-changed here: a list re-fetch would wipe the Undo before it can be clicked.
        let text = serde_json::from_str::<serde_json::Value>(&snap)
            .ok()
            .and_then(|v| v["text"].as_str().map(String::from))
            .unwrap_or_default();
        return web::html(&view::Gone { id, text, snap });
    }
    if let Some(b) = q.back.as_deref() {
        return Ok(web::back(web::safe_back(Some(b), "/")));
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/?day={day}")));
    }
    log_swap(&st, &headers, &day, Some(view::Deleted { snap }))
}

#[derive(Deserialize)]
pub struct BackQuery {
    back: Option<String>,
    /// `1` = deleted from a to-do row (D51): the row becomes its own Undo, see `view::Gone`.
    plan: Option<String>,
}

#[derive(Deserialize)]
pub struct SnapForm {
    snap: String,
    /// A note header's Undo (D49) comes back to the note, not to the line's day.
    back: Option<String>,
    /// `1` = the Undo a deleted to-do row left in its place (D51).
    plan: Option<String>,
}

/// Undo of a delete: the row comes back exactly as it was (see `queries::restore_entry`).
pub async fn restore(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<SnapForm>,
) -> AppResult<Response> {
    let day = queries::restore_entry(&mut st.conn(), &f.snap)?;
    st.line_back(&f.snap);
    if let Some(b) = f.back.as_deref() {
        return Ok(web::back(web::safe_back(Some(b), "/")));
    }
    if f.plan.as_deref() == Some("1") && web::is_htmx(&headers) {
        return Ok(plans_changed());
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/?day={day}")));
    }
    log_swap(&st, &headers, &day, None)
}

fn log_swap(
    st: &AppState,
    headers: &HeaderMap,
    day: &str,
    deleted: Option<view::Deleted>,
) -> AppResult<Response> {
    let d = pick_day(Some(day), st.today());
    let conn = st.conn();
    let swap = view::Swap {
        today: st.today(),
        plans: plans(&conn, st.today())?,
        day: view::Day::new(d, st.today()),
        groups: groups(&conn, d, &Opts::from_page(headers))?,
        saved: None,
        deleted,
        made: None,
        edit: None,
    };
    web::html(&swap)
}

/// Most `#` suggestions the menu lists.
const SUGGEST: usize = 5;

/// Live "→ will save as …" under the input; blank box = blank hint, not an error. Also the tag
/// helpers (D29): the `#` menu while the last word is `#…`, else a "Tag as #x?" offer for a plain
/// word that names a known tag.
pub async fn preview(State(st): State<AppState>, Form(f): Form<LineForm>) -> AppResult<Response> {
    let mut out = view::PreviewSwap {
        error: None,
        preview: None,
        ask: None,
        sug: None,
    };
    if f.line.trim().is_empty() {
        return web::html(&out);
    }
    match parse(&with_kind(&f.line, f.kind.as_deref()), st.today()) {
        Ok(e) => {
            out.preview = Some(view::Preview {
                soft: by_as_text(&e),
                kind: e.kind.as_str(),
                key: e.epic_key,
                text: e.text,
                due: e.due.map(dates::label),
            })
        }
        Err(e) => out.error = Some(e.to_string()),
    }
    let known = queries::known_tags(&st.conn())?;
    // ponytail: assumes the caret is at the end of the line (where typing happens); mid-line `#` gets no menu.
    let last = f.line.rsplit(char::is_whitespace).next().unwrap_or("");
    if let Some(q) = last.strip_prefix('#')
        && q.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        let q = q.to_ascii_lowercase();
        let opts: Vec<(String, i64)> = known
            .iter()
            .filter(|(t, _)| t.starts_with(&q))
            .take(SUGGEST)
            .cloned()
            .collect();
        let exact = opts.iter().any(|(t, _)| *t == q);
        let new = tags::parse_list(&q).pop().filter(|_| !exact);
        if !opts.is_empty() || new.is_some() {
            out.sug = Some(view::Sug { opts, new });
        }
        return web::html(&out);
    }
    let names: Vec<String> = known.into_iter().map(|(t, _)| t).collect();
    let skip: Vec<String> = f
        .notag
        .unwrap_or_default()
        .split(',')
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .collect();
    out.ask = tags::offer(&f.line, &names, &skip).map(|(word, tag)| view::Ask {
        word: word.into(),
        tag,
    });
    web::html(&out)
}
