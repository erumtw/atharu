use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, entry::Kind, entry::slugify, transcript::TxStatus},
    error::{AppError, AppResult},
    layout::{Layout, crumb},
    recording::{self, Banner, MeetingState},
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

fn banner(
    st: &AppState,
    conn: &Connection,
    back: String,
    name: String,
    error: Option<String>,
) -> rusqlite::Result<Banner> {
    Ok(Banner {
        name,
        error,
        ..Banner::new(MeetingState::load(conn, st.mac.as_deref())?, back)
    })
}

fn actions(open: i64, done: i64, dropped: i64) -> String {
    let parts: Vec<String> = [(open, "open"), (done, "done"), (dropped, "dropped")]
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, l)| format!("{n} {l}"))
        .collect();
    if parts.is_empty() {
        "—".into()
    } else {
        parts.join(" · ")
    }
}

fn label(day: &str) -> String {
    NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(dates::label)
        .unwrap_or_else(|_| day.to_string())
}

#[derive(Deserialize)]
pub struct ListQuery {
    name: Option<String>,
}

fn list_row(m: queries::MeetingRow) -> view::ListRow {
    view::ListRow {
        id: m.id,
        day: label(&m.day),
        time: span_label(m.started_at.as_deref(), m.ended_at.as_deref()),
        transcript: match m.tx_lines {
            0 => String::new(),
            1 => "1 line".into(),
            n => format!("{n} lines"),
        },
        decisions: m.decisions,
        actions: actions(m.open, m.done, m.dropped),
        running: m.ended_at.is_none(),
    }
}

/// `today`, else the day label.
fn when(st: &AppState, day: &str) -> String {
    if day == dates::iso(st.today()) {
        "today".into()
    } else {
        label(day)
    }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn render_list(st: &AppState, b: Banner) -> AppResult<view::List> {
    let conn = st.conn();
    let (mut series, mut archived) = (Vec::new(), Vec::new());
    let mut total = 0;
    for r in queries::series_rows(&conn)? {
        let item = view::SeriesItem {
            last: when(st, &r.last_day),
            name: r.name,
            meetings: r.meetings,
            open: r.open,
        };
        if r.archived {
            archived.push(item);
        } else {
            total += item.meetings;
            series.push(item);
        }
    }
    let fact = match series.first() {
        None => String::new(),
        Some(f) => format!(
            "{} · {} · last met {}",
            plural(series.len() as i64, "series", "series"),
            plural(total, "meeting", "meetings"),
            f.last
        ),
    };
    Ok(view::List {
        layout: Layout::load(&conn, "meetings", "Meetings", vec![])?,
        banner: b,
        series,
        archived,
        fact,
    })
}

fn render_series(st: &AppState, name: &str) -> AppResult<view::Series> {
    let conn = st.conn();
    let rows: Vec<queries::MeetingRow> = queries::meetings(&conn, name)?;
    let Some(latest) = rows.first() else {
        return Err(AppError::NotFound);
    };
    let (id, last) = (latest.id, when(st, &latest.day));
    let archived = queries::is_archived(&conn, name)?;
    let fact = format!(
        "{} · last met {last}{}",
        plural(rows.len() as i64, "meeting", "meetings"),
        if archived { " · archived" } else { "" }
    );
    Ok(view::Series {
        layout: Layout::load(
            &conn,
            "meetings",
            name,
            vec![crumb("Meetings", "/meetings")],
        )?,
        name: name.to_string(),
        fact,
        archived,
        rows: rows.into_iter().map(list_row).collect(),
        plans: queries::series_plans(&conn, name)?,
        today: st.today(),
        back: format!("/meetings?name={name}"),
        id,
        field: tag_field(&conn, name)?,
        same: crate::layout::same_name(&conn, name, "series")?,
        // `[[series]]` / `#series` notes and the notes on lines logged in any of its meetings.
        refs: crate::layout::refs(&conn, Some(name), Some(crate::layout::Lines::Series(name)))?,
        ref_name: name.to_string(),
    })
}

pub async fn list(State(st): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Response> {
    if let Some(name) = q.name.filter(|n| !n.is_empty()) {
        return web::html(&render_series(&st, &name)?);
    }
    let b = banner(&st, &st.conn(), "/meetings".into(), String::new(), None)?;
    web::html(&render_list(&st, b)?)
}

#[derive(Deserialize)]
pub struct ArchiveForm {
    name: String,
    /// `1` archives, anything else brings it back.
    on: Option<String>,
    back: Option<String>,
}

/// Archive / Unarchive a series (D4): a plain POST, then back where it came from.
pub async fn archive(
    State(st): State<AppState>,
    Form(f): Form<ArchiveForm>,
) -> AppResult<Response> {
    queries::set_archived(&st.conn(), &f.name, f.on.as_deref() == Some("1"))?;
    let back = f
        .back
        .filter(|b| b.starts_with('/') && !b.starts_with("//"));
    Ok(web::back(back.as_deref().unwrap_or("/meetings")))
}

#[derive(Deserialize)]
pub struct StartForm {
    name: String,
    back: Option<String>,
    /// The Record audio switch (Mac app only); off unless ticked, every time.
    record: Option<String>,
}

pub async fn start(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<StartForm>,
) -> AppResult<Response> {
    let back = web::safe_back(f.back.as_deref(), "/meetings").to_string();
    let slug = slugify(&f.name);
    let error = if slug.is_empty() {
        // Not on the States board; same voice as the line box's "Type what you did first."
        Some("Type a meeting name first.".to_string())
    } else {
        // Bound first: a guard in the match head would live through record_audio's own lock.
        let started = queries::start(&st.conn(), &dates::iso(st.today()), &slug)?;
        match started {
            Ok(id) => {
                if f.record.is_some() {
                    record_audio(&st, id)?;
                }
                None
            }
            Err(running) => Some(format!("End {running} first.")),
        }
    };
    let (status, kept) = match &error {
        None if !web::is_htmx(&headers) => return Ok(web::back(&back)),
        None => (StatusCode::OK, String::new()),
        Some(_) => (StatusCode::UNPROCESSABLE_ENTITY, f.name),
    };
    let b = banner(&st, &st.conn(), back, kept, error)?;
    if web::is_htmx(&headers) {
        return web::html_status(status, &view::BannerSwap { banner: b });
    }
    // No JS and rejected: the meetings page carries the same box, with the message.
    web::html_status(status, &render_list(&st, b)?)
}

#[derive(Deserialize)]
pub struct BackForm {
    back: Option<String>,
}

pub async fn end(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<BackForm>,
) -> AppResult<Response> {
    let back = web::safe_back(f.back.as_deref(), "/meetings").to_string();
    let at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    // Already ended (double click, stale tab) is fine; only an unknown id is an error.
    if !queries::end(&st.conn(), id, &at)? && queries::meeting(&st.conn(), id)?.is_none() {
        return Err(AppError::NotFound);
    }
    // End also stops the recorder; the tail keeps transcribing after the meeting is over.
    queries::stop_transcript(&st.conn(), id, &at)?;
    if !web::is_htmx(&headers) {
        // The popover shows its own "Ended X · Open" from this.
        let back = if back == "/quick" {
            format!("/quick?ended={id}")
        } else {
            back
        };
        return Ok(web::back(&back));
    }
    let mut b = banner(&st, &st.conn(), back, String::new(), None)?;
    b.ended = queries::meeting(&st.conn(), id)?.map(|m| (id, m.name));
    web::html(&view::BannerSwap { banner: b })
}

fn swap_banner(st: &AppState, headers: &HeaderMap, back: String) -> AppResult<Response> {
    if !web::is_htmx(headers) {
        return Ok(web::back(&back));
    }
    let b = banner(st, &st.conn(), back, String::new(), None)?;
    web::html(&view::BannerSwap { banner: b })
}

/// Only the Mac app records, and only with the model on disk; a browser's `record=1` is ignored.
fn record_audio(st: &AppState, id: i64) -> rusqlite::Result<()> {
    if !st.mac.as_deref().is_some_and(recording::Mac::ready) {
        return Ok(());
    }
    let conn = st.conn();
    let lang = recording::lang(&conn)?;
    queries::start_transcript(&conn, id, &recording::now(), recording::MODEL_NAME, &lang)?;
    Ok(())
}

#[derive(Deserialize)]
pub struct BannerQuery {
    back: Option<String>,
}

/// The recording banner polls this every second: elapsed time and the two level meters.
pub async fn banner_get(
    State(st): State<AppState>,
    Query(q): Query<BannerQuery>,
) -> AppResult<Response> {
    let back = web::safe_back(q.back.as_deref(), "/meetings").to_string();
    let b = banner(&st, &st.conn(), back, String::new(), None)?;
    web::html(&view::BannerSwap { banner: b })
}

/// The Record switch just turned on (Today, the meeting pages, the popover): ask for Screen & System Audio
/// Recording now, so the Quit & Reopen it needs happens before the meeting, not in it. Browser: nothing to ask.
pub async fn permissions(State(st): State<AppState>) -> AppResult<Response> {
    let allowed = st
        .mac
        .as_deref()
        .and_then(|m| m.ask_screen.get())
        .is_none_or(|ask| ask());
    web::html(&view::Permission { needed: !allowed })
}

/// `HH:MM–HH:MM` from `YYYY-MM-DD HH:MM:SS` stamps; `HH:MM · running` while it runs; empty without a start.
fn span_label(start: Option<&str>, end: Option<&str>) -> String {
    let hm = |s: &str| s.get(11..16).unwrap_or_default().to_string();
    match (start, end) {
        (Some(s), Some(e)) => format!("{}–{}", hm(s), hm(e)),
        (Some(s), None) => format!("{} · running", hm(s)),
        _ => String::new(),
    }
}

/// "Start notes" (T90): create the meeting's note and come back to edit it in place.
pub async fn start_note(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Response> {
    queries::create_note(&mut st.conn(), id)?.ok_or(AppError::NotFound)?;
    Ok(web::back(&format!("/meetings/{id}#notes")))
}

#[derive(Deserialize)]
pub struct SeriesNameForm {
    name: String,
}

/// Rename the meeting's series from its title (T92). The page reloads after (htmx `HX-Redirect`), since the
/// crumb, sidebar and every list name it; a refusal comes back as a hint under the title.
pub async fn rename(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<SeriesNameForm>,
) -> AppResult<Response> {
    let m = queries::meeting(&st.conn(), id)?.ok_or(AppError::NotFound)?;
    let new = slugify(&f.name);
    let error = if new.is_empty() {
        Some("Type a name with letters or digits, e.g. cs-hub-sync.".to_string())
    } else if !queries::rename_series(&mut st.conn(), &m.name, &new)? {
        Some(format!(
            "“{new}” is another series already. Pick a different name."
        ))
    } else {
        None
    };
    let to = format!("/meetings/{id}");
    match error {
        None if web::is_htmx(&headers) => {
            Ok(([("HX-Redirect", to)], StatusCode::OK).into_response())
        }
        None => Ok(web::back(&to)),
        Some(e) => Ok((StatusCode::UNPROCESSABLE_ENTITY, e).into_response()),
    }
}

/// Record a running meeting that isn't recording (after Stop, or after Quit & Reopen for a permission).
pub async fn record(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<BackForm>,
) -> AppResult<Response> {
    queries::meeting(&st.conn(), id)?.ok_or(AppError::NotFound)?;
    record_audio(&st, id)?;
    swap_banner(
        &st,
        &headers,
        web::safe_back(f.back.as_deref(), "/meetings").to_string(),
    )
}

/// Stop recording; the meeting keeps running.
pub async fn stop(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<BackForm>,
) -> AppResult<Response> {
    queries::meeting(&st.conn(), id)?.ok_or(AppError::NotFound)?;
    queries::stop_transcript(&st.conn(), id, &recording::now())?;
    swap_banner(
        &st,
        &headers,
        web::safe_back(f.back.as_deref(), "/meetings").to_string(),
    )
}

#[derive(Deserialize)]
pub struct Segment {
    t: f64,
    speaker: String,
    text: String,
}

#[derive(Deserialize)]
pub struct Segments {
    #[serde(default)]
    segments: Vec<Segment>,
    /// The recorder finished the tail (or gave up): close the recording.
    #[serde(default)]
    done: bool,
    /// The speaker split (T56): Others lines as `Speaker N`, replacing every line that isn't "me".
    #[serde(default)]
    split: Option<Vec<Segment>>,
}

/// Tenths: the dedupe compares t for equality, so both sides must round the same way.
fn tidy(segs: Vec<Segment>, ok: impl Fn(&str) -> bool) -> Vec<(f64, String, String)> {
    segs.into_iter()
        .filter(|s| ok(&s.speaker) && !s.text.trim().is_empty())
        .map(|s| {
            (
                (s.t * 10.0).round() / 10.0,
                s.speaker,
                s.text.trim().to_string(),
            )
        })
        .collect()
}

/// `Speaker 3`: what the split names a voice before the owner does.
fn split_speaker(s: &str) -> bool {
    s == "others"
        || s.strip_prefix("Speaker ")
            .is_some_and(|n| n.parse::<u16>().is_ok_and(|n| n > 0))
}

/// The Mac app's recorder posts each window's lines here as soon as whisper returns them, so a crash
/// keeps everything up to the last window. JSON in, 204 out; not a form, nobody browses to it.
pub async fn segments(
    State(st): State<AppState>,
    Path(tid): Path<i64>,
    axum::Json(b): axum::Json<Segments>,
) -> AppResult<Response> {
    let segs = tidy(b.segments, |s| matches!(s, "me" | "others"));
    let split = b.split.map(|s| tidy(s, split_speaker));
    let res = queries::append_segments(
        &st.conn(),
        tid,
        &segs,
        split.as_deref(),
        b.done,
        &recording::now(),
    )?;
    Ok(match res {
        queries::Append::Saved => StatusCode::NO_CONTENT.into_response(),
        queries::Append::NotFound => return Err(AppError::NotFound),
        queries::Append::Closed => StatusCode::CONFLICT.into_response(),
    })
}

#[derive(Deserialize)]
pub struct TxQuery {
    #[serde(default)]
    q: String,
}

fn shots_view(st: &AppState, conn: &Connection, id: i64, running: bool) -> view::Shots {
    view::Shots {
        id,
        files: super::shots::root(conn)
            .map(|r| super::shots::list(&r, id))
            .unwrap_or_default(),
        can_take: st.mac.is_some() && running,
    }
}

/// The Screenshots section alone: re-fetched after the app adds one (`atharu-sync`).
pub async fn shots(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Response> {
    let conn = st.conn();
    let m = queries::meeting(&conn, id)?.ok_or(AppError::NotFound)?;
    web::html(&shots_view(&st, &conn, id, m.ended_at.is_none()))
}

/// One screenshot. Only names `shots::save` writes are served, so no path can leave the meeting's folder.
pub async fn shot_file(
    State(st): State<AppState>,
    Path((id, file)): Path<(i64, String)>,
) -> AppResult<Response> {
    if !super::shots::valid_name(&file) {
        return Err(AppError::NotFound);
    }
    let root = super::shots::root(&st.conn()).ok_or(AppError::NotFound)?;
    let bytes =
        std::fs::read(root.join(id.to_string()).join(&file)).map_err(|_| AppError::NotFound)?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/png"),
            (
                axum::http::header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
        ],
        bytes,
    )
        .into_response())
}

/// The Transcript section alone: polled while live / finishing.
pub async fn transcript(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<TxQuery>,
) -> AppResult<Response> {
    let conn = st.conn();
    queries::meeting(&conn, id)?.ok_or(AppError::NotFound)?;
    web::html(&tx_section(&conn, st.mac.is_some(), id, q.q)?)
}

fn tx_section(
    conn: &Connection,
    mac: bool,
    id: i64,
    q: String,
) -> rusqlite::Result<view::TxSection> {
    let list: Vec<view::TxView> = queries::transcripts(conn, id)?
        .into_iter()
        .map(|t| tx_view(t, mac, q.trim()))
        .collect();
    Ok(view::TxSection {
        meeting: id,
        polling: list.iter().any(|t| t.live || t.finishing.is_some()),
        q,
        mac,
        list,
    })
}

#[derive(Deserialize)]
pub struct RenameForm {
    from: String,
    to: String,
}

/// "Speaker 2 = …" on a finished recording: every line of that voice takes the name. A blank name, or one
/// that reads as Me / Others, changes nothing (the field shows the old name again).
pub async fn rename_speaker(
    State(st): State<AppState>,
    Path((id, tid)): Path<(i64, i64)>,
    headers: HeaderMap,
    Form(f): Form<RenameForm>,
) -> AppResult<Response> {
    let conn = st.conn();
    queries::meeting(&conn, id)?.ok_or(AppError::NotFound)?;
    let to = f.to.trim();
    let taken = ["me", "others"].iter().any(|r| to.eq_ignore_ascii_case(r));
    if !to.is_empty() && to.chars().count() <= 40 && !taken && f.from != "me" {
        queries::rename_speaker(&conn, id, tid, &f.from, to)?;
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/meetings/{id}#transcript")));
    }
    web::html(&tx_section(&conn, st.mac.is_some(), id, String::new())?)
}

/// Row label: the two streams by their UI names; a split voice by its own (`Speaker 2`, or the owner's name).
fn who(speaker: &str) -> &str {
    match speaker {
        "me" => "Me",
        "others" => "Others",
        s => s,
    }
}

/// `text` cut around case-insensitive hits of `q` (ASCII folding keeps byte offsets; Thai has no case).
fn marks(text: &str, q: &str) -> Vec<(String, bool)> {
    if q.is_empty() {
        return vec![(text.to_string(), false)];
    }
    let (hay, needle) = (text.to_ascii_lowercase(), q.to_ascii_lowercase());
    let mut parts = Vec::new();
    let mut at = 0;
    while let Some(i) = hay[at..].find(&needle) {
        let (s, e) = (at + i, at + i + needle.len());
        if s > at {
            parts.push((text[at..s].to_string(), false));
        }
        parts.push((text[s..e].to_string(), true));
        at = e;
    }
    if at < text.len() {
        parts.push((text[at..].to_string(), false));
    }
    parts
}

fn tx_view(t: queries::Transcript, mac: bool, q: &str) -> view::TxView {
    let lines = t.segments.len();
    let last = t.segments.iter().map(|s| s.0).fold(0.0, f64::max);
    let ran = t
        .ended_at
        .as_deref()
        .map(|e| recording::secs_between(&t.started_at, e));
    let copy = t
        .segments
        .iter()
        .map(|(at, sp, text)| format!("{} {}: {text}", recording::clock(*at), who(sp)))
        .collect::<Vec<_>>()
        .join("\n");
    let mut rows = Vec::new();
    let mut prev = "";
    for (at, sp, text) in &t.segments {
        let parts = marks(text, q);
        if !q.is_empty() && !parts.iter().any(|p| p.1) {
            continue;
        }
        rows.push(view::TxRow {
            time: recording::clock(*at),
            who: who(sp).to_string(),
            cont: prev == sp,
            parts,
        });
        prev = sp;
    }
    let recorded = || recording::span(ran.unwrap_or(0));
    let (meta, finishing) = match t.status {
        TxStatus::Live if mac => (
            format!("Live · about 30 s behind · {} · {lines} lines", t.lang),
            None,
        ),
        TxStatus::Live => (format!("Live · {}", t.lang), None),
        TxStatus::Finishing => {
            let total = ran.unwrap_or(0).max(1) as f64;
            let pct = (last / total * 100.0).clamp(0.0, 99.0) as u8;
            (
                format!(
                    "Recorded {} · transcribed up to {}",
                    recorded(),
                    recording::clock(last)
                ),
                // RTF ≈ 0.1 on the M5 (D27): the rest of the audio takes about a tenth of its length; then
                // the speaker split, about another tenth of the whole recording (D35: diarization RTF 0.08).
                Some((
                    pct,
                    recording::about((total - last).max(0.0) * 0.1 + total * 0.1),
                )),
            )
        }
        TxStatus::Done | TxStatus::Interrupted => (
            format!(
                "Recorded {} · {lines} lines · {} · {}",
                recorded(),
                t.model,
                t.lang
            ),
            None,
        ),
    };
    view::TxView {
        id: t.id,
        meta,
        rows,
        copy,
        lines,
        live: t.status == TxStatus::Live,
        finishing,
        interrupted: (t.status == TxStatus::Interrupted).then(|| recording::clock(last)),
        done: t.status == TxStatus::Done,
        speakers: if t.status == TxStatus::Done {
            t.speakers()
        } else {
            Vec::new()
        },
    }
}

pub async fn page(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<TxQuery>,
) -> AppResult<Response> {
    let b = banner(
        &st,
        &st.conn(),
        format!("/meetings/{id}"),
        String::new(),
        None,
    )?;
    let recording_here = b.state.running.as_ref().is_some_and(|a| a.id == id)
        && (b.state.rec.live.is_some() || b.state.rec.finishing);
    let conn = st.conn();
    let m = queries::meeting(&conn, id)?.ok_or(AppError::NotFound)?;
    let siblings = queries::siblings(&conn, &m.name, id)?;
    let mut groups: Vec<view::KindGroup> = [
        (Kind::Decision, "Decisions"),
        (Kind::Learn, "Learned"),
        (Kind::Impact, "Impact"),
        (Kind::Blocker, "Blockers"),
        (Kind::Done, "Done"),
    ]
    .iter()
    .map(|(_, title)| view::KindGroup {
        title,
        lines: Vec::new(),
    })
    .collect();
    for l in queries::lines(&conn, id)? {
        let i = match l.kind {
            Kind::Decision => 0,
            Kind::Learn => 1,
            Kind::Impact => 2,
            Kind::Blocker => 3,
            _ => 4,
        };
        groups[i].lines.push(l);
    }
    groups.retain(|g| !g.lines.is_empty());
    let day = NaiveDate::parse_from_str(&m.day, "%Y-%m-%d")
        .map(|d| d.format("%a %-d %b %Y").to_string())
        .unwrap_or_else(|_| m.day.clone());
    web::html(&view::Page {
        layout: Layout::load(
            &conn,
            "meetings",
            label(&m.day),
            vec![
                crumb("Meetings", "/meetings"),
                crumb(m.name.clone(), format!("/meetings?name={}", m.name)),
            ],
        )?,
        id,
        series_count: siblings.len() + 1,
        siblings: siblings
            .iter()
            .take(2)
            .map(|(i, d)| (*i, label(d)))
            .collect(),
        plans: queries::plans(&conn, id)?,
        groups,
        day,
        // Stored as local `YYYY-MM-DD HH:MM:SS`; the page only needs the time.
        ended: m
            .ended_at
            .clone()
            .map(|t| t.get(11..16).unwrap_or(&t).to_string()),
        // Only notes on this meeting's own lines: `[[series]]` mentions belong to the series page. Its own note is
        // the page body, so it isn't listed again.
        refs: crate::layout::refs(&conn, None, Some(crate::layout::Lines::Meeting(id)))?
            .into_iter()
            .filter(|r| m.note.as_ref().is_none_or(|own| own.0 != r.id))
            .collect(),
        ref_name: m.name.clone(),
        same: crate::layout::same_name(&conn, &m.name, "series")?,
        field: tag_field(&conn, &m.name)?,
        banner: recording_here.then_some(b),
        tx: tx_section(&conn, st.mac.is_some(), id, q.q)?,
        shots: shots_view(&st, &conn, id, m.ended_at.is_none()),
        name: m.name,
        note: m.note,
        today: st.today(),
        back: format!("/meetings/{id}"),
    })
}

fn tag_field(conn: &rusqlite::Connection, name: &str) -> rusqlite::Result<crate::layout::TagField> {
    Ok(crate::layout::TagField {
        id: "series-tags",
        tags: queries::series_tags(conn, name)?,
        known: queries::tag_names(conn)?,
    })
}

#[derive(Deserialize)]
pub struct TagsForm {
    tags: Option<String>,
    tags_add: Option<String>,
    tags_drop: Option<String>,
}

/// The series' Tags prop, edited from any of its meetings: lines logged in every meeting of the
/// series inherit these (entry_tags), read time.
pub async fn set_tags(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<TagsForm>,
) -> AppResult<Response> {
    let conn = st.conn();
    let m = queries::meeting(&conn, id)?.ok_or(AppError::NotFound)?;
    let list = crate::domain::tags::edit_list(
        f.tags.as_deref().unwrap_or(""),
        f.tags_add.as_deref(),
        f.tags_drop.as_deref(),
    );
    queries::set_series_tags(&conn, &m.name, &list)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/meetings/{id}")));
    }
    web::html(&view::Tags {
        id,
        field: tag_field(&conn, &m.name)?,
    })
}

#[derive(Deserialize)]
pub struct NotesForm {
    notes: String,
}

/// Autosave: htmx posts as you type; without JS the same form posts via Save.
pub async fn save_notes(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<NotesForm>,
) -> AppResult<Response> {
    if !queries::set_notes(&st.conn(), id, &f.notes)? {
        return Err(AppError::NotFound);
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&format!("/meetings/{id}")));
    }
    web::html(&view::Saved)
}

/// Delete + Undo toast; the meeting's lines stay on their days, just no longer in a meeting.
pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let (name, snap) = queries::delete(&mut st.conn(), id)?.ok_or(AppError::NotFound)?;
    if !web::is_htmx(&headers) {
        return Ok(web::back("/meetings"));
    }
    web::html(&deleted(format!("Meeting {name}"), snap, None))
}

fn deleted(what: String, snap: String, error: Option<String>) -> web::Deleted {
    web::Deleted {
        what,
        restore: "/meetings/restore".into(),
        snap,
        back: "/meetings",
        back_label: "All meetings",
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
        Ok(id) => Ok(web::restored(&headers, &format!("/meetings/{id}"))),
        // Same copy as starting a second meeting.
        Err(running) => web::html_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            &deleted(
                "Meeting".into(),
                f.snap,
                Some(format!("End {running} first.")),
            ),
        ),
    }
}
