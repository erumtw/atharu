use super::{ical, queries, view};
use crate::{
    app::AppState,
    domain::{dates, timegrid},
    error::{AppError, AppResult},
    layout::{Layout, TagFilter},
    web,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{Days, Months, NaiveDate, NaiveTime, Timelike};
use rusqlite::Connection;
use serde::Deserialize;

/// Where the calendar is pointed: `?d=` (a day in the range; for N days its first day) +
/// `view=day|week|month` (none = N days: `n` 1–7 picked by the page's width, 7 by default, D3).
#[derive(Deserialize, Default)]
pub struct RangeQuery {
    d: Option<String>,
    /// Pre-0.2.3 name of `d`, so old links still land.
    week: Option<String>,
    view: Option<String>,
    n: Option<String>,
    /// Tag filter, comma list (any of them); `add` = the filter row's add box.
    tag: Option<String>,
    add: Option<String>,
    /// `YYYY-MM-DDTHH:MM` from an empty slot: "+ Event" opens with that day and time.
    new: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Day,
    Days,
    Week,
    Month,
}

#[derive(Clone)]
struct Range {
    view: View,
    n: u32,
    anchor: NaiveDate,
    /// `d` was in the URL; without it N days pick their own window around today.
    given: bool,
    tags: Vec<String>,
}

impl Range {
    fn of(q: &RangeQuery, today: NaiveDate) -> Range {
        let d: Option<NaiveDate> =
            q.d.as_deref()
                .or(q.week.as_deref())
                .and_then(|w| w.parse().ok());
        let view = match q.view.as_deref() {
            Some("day") => View::Day,
            Some("week") => View::Week,
            Some("month") => View::Month,
            _ => View::Days,
        };
        let n = match view {
            View::Day => 1,
            View::Days => {
                q.n.as_deref()
                    .and_then(|n| n.parse::<u32>().ok())
                    .map_or(7, |n| n.clamp(1, 7))
            }
            View::Week | View::Month => 7,
        };
        Range {
            view,
            n,
            anchor: d.unwrap_or(today),
            given: d.is_some(),
            tags: TagFilter::pick(q.tag.as_deref(), q.add.as_deref()),
        }
    }

    /// First and last day: Sun–Sat of the week (D48), whole weeks covering the month, the day, or N days.
    fn span(&self) -> (NaiveDate, NaiveDate) {
        match self.view {
            View::Month => {
                let ms = dates::month_start(self.anchor);
                let last = dates::end_of(ms, 1);
                (
                    dates::calendar_week_start(ms),
                    dates::calendar_week_start(last) + Days::new(6),
                )
            }
            // 7 days is the week, so the default view and old `?week=` links stay Sun–Sat.
            View::Week | View::Days if self.n == 7 => {
                let ws = dates::calendar_week_start(self.anchor);
                (ws, ws + Days::new(6))
            }
            View::Day => (self.anchor, self.anchor),
            View::Days | View::Week => {
                // Unpointed: the N days of this week that hold today, as late in the week as fits
                // (Fri 13 with 5 → Tue 10 – Sat 14), so the past days stay in view.
                let start = if self.given {
                    self.anchor
                } else {
                    let latest =
                        dates::calendar_week_start(self.anchor) + Days::new(7 - u64::from(self.n));
                    self.anchor.min(latest)
                };
                (start, start + Days::new(u64::from(self.n) - 1))
            }
        }
    }

    /// The day forms and links carry: N days are pointed by their first day, the rest by any day in them.
    fn d(&self) -> NaiveDate {
        match self.view {
            View::Days if self.n < 7 => self.span().0,
            _ => self.anchor,
        }
    }

    fn step(&self, forward: bool) -> NaiveDate {
        let days = match self.view {
            View::Month if forward => return dates::month_start(self.anchor) + Months::new(1),
            View::Month => return dates::month_start(self.anchor) - Months::new(1),
            View::Week => 7,
            View::Day | View::Days => self.n,
        };
        let from = self.d();
        if forward {
            from + Days::new(u64::from(days))
        } else {
            from - Days::new(u64::from(days))
        }
    }

    fn view_name(&self) -> &'static str {
        match self.view {
            View::Day => "day",
            View::Days => "days",
            View::Week => "week",
            View::Month => "month",
        }
    }

    /// `/calendar?d=…[&view=…][&n=…]`, no tag: the tag filter appends its own. No `d` = around today.
    fn base(&self, d: Option<NaiveDate>) -> String {
        let mut q: Vec<String> = d
            .map(|d| format!("d={}", dates::iso(d)))
            .into_iter()
            .collect();
        match self.view {
            View::Days if self.n == 7 => {}
            View::Days => q.push(format!("n={}", self.n)),
            _ => q.push(format!("view={}", self.view_name())),
        }
        if q.is_empty() {
            "/calendar".into()
        } else {
            format!("/calendar?{}", q.join("&"))
        }
    }

    fn link(&self, d: Option<NaiveDate>) -> String {
        let base = self.base(d);
        if self.tags.is_empty() {
            base
        } else {
            let sep = if base.contains('?') { '&' } else { '?' };
            format!("{base}{sep}tag={}", self.tags.join(","))
        }
    }

    fn href(&self, d: NaiveDate) -> String {
        self.link(Some(d))
    }

    /// The view switch: the same day in another view (N days keeps its N, else 7 until the page picks one).
    fn views(&self) -> Vec<view::ViewLink> {
        [
            ("Day", View::Day),
            ("N days", View::Days),
            ("Week", View::Week),
            ("Month", View::Month),
        ]
        .into_iter()
        .map(|(label, v)| {
            let n = match v {
                View::Day => 1,
                View::Days if self.view == View::Days => self.n,
                _ => 7,
            };
            let to = Range {
                view: v,
                n,
                ..self.clone()
            };
            view::ViewLink {
                label,
                href: to.link(self.given.then_some(self.anchor)),
                on: v == self.view,
            }
        })
        .collect()
    }

    /// `9 – 15 Nov`, `30 Nov – 6 Dec`, `Fri 13 Nov`, or `November 2026`.
    fn title(&self) -> String {
        if self.view == View::Month {
            return self.anchor.format("%B %Y").to_string();
        }
        let (a, b) = self.span();
        if a == b {
            a.format("%a %-d %b").to_string()
        } else if a.format("%m").to_string() == b.format("%m").to_string() {
            format!("{} – {}", a.format("%-d"), b.format("%-d %b"))
        } else {
            format!("{} – {}", a.format("%-d %b"), b.format("%-d %b"))
        }
    }

    /// What ‹ › move by: `week`, `month`, `day` or `5 days`.
    fn unit(&self) -> String {
        match self.view {
            View::Month => "month".into(),
            _ if self.n == 7 => "week".into(),
            _ if self.n == 1 => "day".into(),
            _ => format!("{} days", self.n),
        }
    }
}

fn ix(from: NaiveDate, day: &str) -> Option<usize> {
    let d: NaiveDate = day.get(..10)?.parse().ok()?;
    usize::try_from((d - from).num_days()).ok()
}

/// `HH:MM` minutes of a stored `YYYY-MM-DD?HH:MM…` (meetings use a space, events a `T`).
fn mins(at: &str) -> Option<u32> {
    let t = NaiveTime::parse_from_str(at.get(11..16)?, "%H:%M").ok()?;
    Some(t.hour() * 60 + t.minute())
}

fn hm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Where a timed item ends: `end` on the same day, midnight when it runs past it, else `fallback`.
fn end_of(start_at: &str, end_at: Option<&str>, fallback: u32) -> u32 {
    match end_at {
        Some(e) if e.get(..10) == start_at.get(..10) => mins(e).unwrap_or(fallback),
        Some(_) => 24 * 60,
        None => fallback,
    }
}

/// A short item still needs room for its title: what the grid draws and lays out.
const MIN_LEN: u32 = 20;

fn cal(
    conn: &Connection,
    r: &Range,
    today: NaiveDate,
    notice: Option<view::Notice>,
) -> AppResult<view::Cal> {
    let (from, to) = r.span();
    let (f, t) = (dates::iso(from), dates::iso(to));
    let month = r.view == View::Month;
    let mut days: Vec<view::Day> = from
        .iter_days()
        .take_while(|d| *d <= to)
        .map(|d| view::Day {
            iso: dates::iso(d),
            wd: d.format("%a").to_string(),
            num: d.format("%-d").to_string(),
            now: d == today,
            outside: month && dates::month_start(d) != dates::month_start(r.anchor),
            logged: 0,
            all_day: Vec::new(),
            timed: Vec::new(),
        })
        .collect();
    // ponytail: the real clock even under a fixed test day; tests never assert the now line's place.
    let now = chrono::Local::now().time();
    let now = now.hour() * 60 + now.minute();
    // (day index, start, end, item): placed once every day's hours are known.
    let mut timed: Vec<(usize, u32, u32, view::Item)> = Vec::new();
    let mut uid = 0;
    let mut put = |day: &str, at: Option<(u32, u32)>, mut item: view::Item| {
        uid += 1;
        item.uid = uid;
        let Some(i) = ix(from, day).filter(|i| *i < days.len()) else {
            return;
        };
        match at {
            Some((s, e)) => timed.push((i, s, e.max(s + MIN_LEN).min(24 * 60), item)),
            None => days[i].all_day.push(item),
        }
    };
    let tags = &r.tags;
    for m in queries::meetings(conn, &f, &t, tags)? {
        let start = m.started_at.as_deref().and_then(mins);
        let at = start.map(|s| {
            let started = m.started_at.as_deref().unwrap_or_default();
            // Running: up to now on its own day.
            let fallback = if m.day == dates::iso(today) {
                now
            } else {
                s + 30
            };
            (s, end_of(started, m.ended_at.as_deref(), fallback))
        });
        let sub = match (at, &m.ended_at) {
            (Some((s, e)), Some(_)) => format!("{} – {}", hm(s), hm(e)),
            (Some((s, _)), None) => format!("{} · running", hm(s)),
            // Stored as local `YYYY-MM-DD HH:MM:SS`; the cell only needs the time.
            (None, Some(e)) => format!("ended {}", e.get(11..16).unwrap_or(e)),
            (None, None) => "running".into(),
        };
        put(
            &m.day,
            at,
            view::Item {
                class: "meet",
                time: start.map(hm),
                text: m.name,
                sub: Some(sub),
                href: None,
                meeting: Some(m.id),
                uid: 0,
                late: false,
                own_id: None,
            },
        );
    }
    let to_excl = dates::iso(to + Days::new(1));
    for e in queries::events(conn, &f, &to_excl, tags)? {
        let own = e.source == "own";
        // Google events are never tagged (D29): under a filter, a title word naming the tag is enough.
        let by_word = || {
            e.title.split_whitespace().any(|w| {
                let w = w.trim_matches(|c: char| !c.is_alphanumeric());
                tags.iter().any(|t| crate::domain::tags::same_word(w, t))
            })
        };
        if !tags.is_empty() && !(if own { e.tagged } else { by_word() }) {
            continue;
        }
        let Some(day0) = e
            .starts_at
            .get(..10)
            .and_then(|d| d.parse::<NaiveDate>().ok())
        else {
            continue;
        };
        let end_day: Option<NaiveDate> =
            e.ends_at.as_deref().and_then(|x| x.get(..10)?.parse().ok());
        let (start, end) = (mins(&e.starts_at), e.ends_at.as_deref().and_then(mins));
        let sub = start.map(|s| match end {
            Some(end) => format!("{} – {}", hm(s), hm(end)),
            None => hm(s),
        });
        // One chip per covered day in range: an all-day event runs to the day before its (exclusive) end;
        // a timed one is clipped to [00:00, 24:00) on each day; an own event has no end: an hour, as Google's default.
        let last = match (start, end_day) {
            (None, Some(d)) if d > day0 => d.pred_opt().unwrap_or(d),
            (Some(_), Some(d)) if d > day0 => d,
            _ => day0,
        };
        for d in day0.iter_days().take_while(|d| *d <= last.min(to)) {
            if d < from {
                continue;
            }
            let at = match start {
                None => None,
                Some(s) => {
                    let a = if d == day0 { s } else { 0 };
                    let b = match end {
                        _ if d < last => 24 * 60,
                        Some(end) if last > day0 || end_day == Some(day0) => end,
                        _ => s + 60,
                    };
                    // Ends at midnight: nothing on that last day.
                    if b <= a && d > day0 {
                        continue;
                    }
                    Some((a, b))
                }
            };
            put(
                &dates::iso(d),
                at,
                view::Item {
                    class: if own { "own" } else { "meet" },
                    time: start.filter(|_| d == day0).map(hm),
                    sub: sub.clone(),
                    text: e.title.clone(),
                    href: None,
                    meeting: None,
                    uid: 0,
                    late: false,
                    own_id: own.then_some(e.id),
                },
            );
        }
    }
    for (due, text) in queries::due_plans(conn, &f, &t, tags)? {
        let late = due < dates::iso(today);
        put(
            &due,
            None,
            view::Item {
                class: "todo",
                time: None,
                text,
                sub: None,
                href: Some("/todos".into()),
                meeting: None,
                uid: 0,
                late,
                own_id: None,
            },
        );
    }
    for (day, key) in queries::targets(conn, &f, &t, tags)? {
        put(
            &day,
            None,
            view::Item {
                class: "tgt",
                time: None,
                text: format!("target · {key}"),
                sub: None,
                href: Some(format!("/epics/{key}")),
                meeting: None,
                uid: 0,
                late: false,
                own_id: None,
            },
        );
    }
    for (day, n) in queries::logged(conn, &f, &t, tags)? {
        if let Some(d) = ix(from, &day).and_then(|i| days.get_mut(i)) {
            d.logged = n;
        }
    }
    // 07–21, widened to the earliest start and latest end in range.
    let h0 = timed.iter().map(|t| t.1 / 60).min().unwrap_or(7).min(7);
    let h1 = timed
        .iter()
        .map(|t| t.2.div_ceil(60))
        .max()
        .unwrap_or(21)
        .max(21);
    timed.sort_by_key(|t| (t.0, t.1));
    // Overlap columns per day: `timed` is sorted by day, so each day is one run.
    let cols_of: Vec<(usize, usize)> = timed
        .chunk_by(|a, b| a.0 == b.0)
        .flat_map(|run| timegrid::columns(&run.iter().map(|t| (t.1, t.2)).collect::<Vec<_>>()))
        .collect();
    for ((day, s, e, item), (col, cols)) in timed.into_iter().zip(cols_of) {
        days[day].timed.push(view::Timed {
            item,
            top: s - h0 * 60,
            len: e - s,
            col,
            cols,
        });
    }
    let now_min = (!month && (from..=to).contains(&today) && (h0 * 60..h1 * 60).contains(&now))
        .then(|| now - h0 * 60);
    Ok(view::Cal {
        view: r.view_name(),
        n: r.n,
        d: dates::iso(r.d()),
        tag: r.tags.join(","),
        here: r.href(r.d()),
        h0,
        h1,
        now_min,
        days,
        notice,
    })
}

fn render_page(
    st: &AppState,
    r: &Range,
    notice: Option<view::Notice>,
    form: view::EventForm,
) -> AppResult<view::Page> {
    let conn = st.conn();
    Ok(view::Page {
        layout: Layout::load(&conn, "calendar", "Calendar", vec![])?,
        title: r.title(),
        prev: r.href(r.step(false)),
        next: r.href(r.step(true)),
        unit: r.unit(),
        today: r.link(None),
        views: r.views(),
        cal: cal(&conn, r, st.today(), notice)?,
        has_ical: st.settings.ical_url.is_some(),
        filter: TagFilter {
            tags: r.tags.clone(),
            path: format!("{}&", r.base(Some(r.d()))),
            // Days carries `n`, never `view`: a `view` in the URL would stop the page picking N by width.
            keep: vec![
                ("d", dates::iso(r.d())),
                if r.view == View::Days {
                    ("n", r.n.to_string())
                } else {
                    ("view", r.view_name().into())
                },
            ],
            known: queries::tag_names(&conn)?,
        },
        form,
    })
}

pub async fn page(State(st): State<AppState>, Query(q): Query<RangeQuery>) -> AppResult<Response> {
    let r = Range::of(&q, st.today());
    // An empty slot's link: "+ Event" opens with its day and hour.
    let form = match q
        .new
        .as_deref()
        .map(|n| n.split_once('T').unwrap_or((n, "")))
    {
        Some((date, time)) => view::EventForm {
            title: String::new(),
            date: date.into(),
            time: time.into(),
        },
        None => view::EventForm::default(),
    };
    web::html(&render_page(&st, &r, None, form)?)
}

/// After an action: htmx gets the grid, a plain form gets a 303 back to the same range.
fn answer(
    st: &AppState,
    headers: &HeaderMap,
    r: &Range,
    notice: Option<view::Notice>,
) -> AppResult<Response> {
    if !web::is_htmx(headers) {
        return Ok(web::back(&r.href(r.d())));
    }
    let cal = cal(&st.conn(), r, st.today(), notice)?;
    web::html(&view::Swap { cal })
}

#[derive(Deserialize)]
pub struct AddForm {
    title: String,
    date: String,
    #[serde(default)]
    time: String,
    d: Option<String>,
    view: Option<String>,
    n: Option<String>,
    tag: Option<String>,
}

pub async fn add(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<AddForm>,
) -> AppResult<Response> {
    let r = Range::of(
        &RangeQuery {
            d: f.d.clone(),
            view: f.view.clone(),
            n: f.n.clone(),
            tag: f.tag.clone(),
            ..Default::default()
        },
        st.today(),
    );
    let title = f.title.trim();
    let date = NaiveDate::parse_from_str(f.date.trim(), "%Y-%m-%d").ok();
    let time = f.time.trim();
    let time_ok = time.is_empty() || NaiveTime::parse_from_str(time, "%H:%M").is_ok();
    // Not on the States board; same voice as "Type what you did first."
    let error = match (title.is_empty(), date, time_ok) {
        (true, _, _) => Some("Type a title first."),
        (_, None, _) => Some("Pick a date."),
        (_, _, false) => Some("Time looks like 16:00."),
        _ => None,
    };
    if let Some(e) = error {
        let notice = Some(view::Notice {
            text: e.to_string(),
            error: true,
        });
        if web::is_htmx(&headers) {
            let cal = cal(&st.conn(), &r, st.today(), notice)?;
            return web::html_status(StatusCode::UNPROCESSABLE_ENTITY, &view::Swap { cal });
        }
        let form = view::EventForm {
            title: f.title,
            date: f.date,
            time: f.time,
        };
        return web::html_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            &render_page(&st, &r, notice, form)?,
        );
    }
    let date = date.expect("checked above");
    let starts_at = if time.is_empty() {
        dates::iso(date)
    } else {
        format!("{}T{time}", dates::iso(date))
    };
    queries::add_own(&st.conn(), title, &starts_at)?;
    // Land on a range holding the new event, so it's visible; keep the shown one when it already does.
    let (from, to) = r.span();
    if (from..=to).contains(&date) {
        return answer(&st, &headers, &r, None);
    }
    let r = Range {
        anchor: date,
        given: true,
        ..r
    };
    // Another range: htmx loads that whole page (its title and ‹ › change too), a plain form gets the 303.
    if web::is_htmx(&headers) {
        return Ok(([("HX-Location", r.href(r.d()))], StatusCode::OK).into_response());
    }
    answer(&st, &headers, &r, None)
}

#[derive(Deserialize)]
pub struct RangeForm {
    d: Option<String>,
    view: Option<String>,
    n: Option<String>,
    tag: Option<String>,
}

fn range_of(st: &AppState, f: RangeForm) -> Range {
    Range::of(
        &RangeQuery {
            d: f.d,
            view: f.view,
            n: f.n,
            tag: f.tag,
            ..Default::default()
        },
        st.today(),
    )
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(f): Form<RangeForm>,
) -> AppResult<Response> {
    if !queries::delete_own(&st.conn(), id)? {
        return Err(AppError::NotFound);
    }
    let r = range_of(&st, f);
    answer(&st, &headers, &r, None)
}

pub async fn refresh(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<RangeForm>,
) -> AppResult<Response> {
    let notice = match ical::refresh(&st).await {
        Ok(None) => None,
        Ok(Some(n)) => Some(view::Notice {
            text: format!("Google Calendar refreshed · {n} events."),
            error: false,
        }),
        Err(e) => {
            eprintln!("calendar refresh: {e}");
            Some(view::Notice {
                text: "Couldn't reach Google Calendar. Showing the last copy.".into(),
                error: true,
            })
        }
    };
    let r = range_of(&st, f);
    answer(&st, &headers, &r, notice)
}
