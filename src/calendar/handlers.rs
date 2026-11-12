use super::{ical, queries, view};
use crate::{
    app::AppState,
    domain::dates,
    error::{AppError, AppResult},
    layout::{Layout, TagFilter},
    web,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use chrono::{Days, Months, NaiveDate, NaiveTime};
use rusqlite::Connection;
use serde::Deserialize;

/// Where the calendar is pointed: `?week=` (any day in the range) + `&view=month`.
#[derive(Deserialize, Default)]
pub struct RangeQuery {
    week: Option<String>,
    view: Option<String>,
    /// Tag filter, comma list (any of them); `add` = the filter row's add box.
    tag: Option<String>,
    add: Option<String>,
}

struct Range {
    anchor: NaiveDate,
    month: bool,
    tags: Vec<String>,
}

impl Range {
    fn of(q: &RangeQuery, today: NaiveDate) -> Range {
        Range {
            anchor: q
                .week
                .as_deref()
                .and_then(|w| w.parse().ok())
                .unwrap_or(today),
            month: q.view.as_deref() == Some("month"),
            tags: TagFilter::pick(q.tag.as_deref(), q.add.as_deref()),
        }
    }

    /// First and last day of the grid: Sun–Sat of the week (D48), or whole weeks covering the month.
    fn span(&self) -> (NaiveDate, NaiveDate) {
        if self.month {
            let ms = dates::month_start(self.anchor);
            let last = dates::end_of(ms, 1);
            (
                dates::calendar_week_start(ms),
                dates::calendar_week_start(last) + Days::new(6),
            )
        } else {
            let ws = dates::calendar_week_start(self.anchor);
            (ws, ws + Days::new(6))
        }
    }

    fn step(&self, forward: bool) -> NaiveDate {
        match (self.month, forward) {
            (true, true) => dates::month_start(self.anchor) + Months::new(1),
            (true, false) => dates::month_start(self.anchor) - Months::new(1),
            (false, true) => self.anchor + Days::new(7),
            (false, false) => self.anchor - Days::new(7),
        }
    }

    fn href(&self, anchor: NaiveDate) -> String {
        let view = if self.month { "&view=month" } else { "" };
        let tag = if self.tags.is_empty() {
            String::new()
        } else {
            format!("&tag={}", self.tags.join(","))
        };
        format!("/calendar?week={}{view}{tag}", dates::iso(anchor))
    }

    /// `9 – 15 Nov`, `30 Nov – 6 Dec`, or `November 2026`.
    fn title(&self) -> String {
        if self.month {
            return self.anchor.format("%B %Y").to_string();
        }
        let (a, b) = self.span();
        if a.format("%m").to_string() == b.format("%m").to_string() {
            format!("{} – {}", a.format("%-d"), b.format("%-d %b"))
        } else {
            format!("{} – {}", a.format("%-d %b"), b.format("%-d %b"))
        }
    }
}

fn ix(from: NaiveDate, day: &str) -> Option<usize> {
    let d: NaiveDate = day.get(..10)?.parse().ok()?;
    usize::try_from((d - from).num_days()).ok()
}

fn cal(
    conn: &Connection,
    r: &Range,
    today: NaiveDate,
    notice: Option<view::Notice>,
) -> AppResult<view::Cal> {
    let (from, to) = r.span();
    let (f, t) = (dates::iso(from), dates::iso(to));
    let mut days: Vec<view::Day> = from
        .iter_days()
        .take_while(|d| *d <= to)
        .map(|d| view::Day {
            iso: dates::iso(d),
            head: if r.month {
                d.format("%-d").to_string()
            } else {
                d.format("%a %-d").to_string()
            },
            now: d == today,
            outside: r.month && dates::month_start(d) != dates::month_start(r.anchor),
            logged: 0,
            items: Vec::new(),
        })
        .collect();
    let mut put = |day: &str, item: view::Item| {
        if let Some(d) = ix(from, day).and_then(|i| days.get_mut(i)) {
            d.items.push(item);
        }
    };
    let tags = &r.tags;
    for m in queries::meetings(conn, &f, &t, tags)? {
        put(
            &m.day,
            view::Item {
                class: "meet",
                time: None,
                // Stored as local `YYYY-MM-DD HH:MM:SS`; the cell only needs the time.
                sub: Some(match m.ended_at {
                    Some(e) => format!("ended {}", e.get(11..16).unwrap_or(&e)),
                    None => "running".into(),
                }),
                text: m.name,
                href: Some(format!("/meetings/{}", m.id)),
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
        put(
            &e.starts_at,
            view::Item {
                class: if own { "own" } else { "meet" },
                time: e.starts_at.get(11..16).map(str::to_string),
                text: e.title,
                sub: None,
                href: None,
                late: false,
                own_id: own.then_some(e.id),
            },
        );
    }
    for (due, text) in queries::due_plans(conn, &f, &t, tags)? {
        let late = due < dates::iso(today);
        put(
            &due,
            view::Item {
                class: "todo",
                time: None,
                text,
                sub: None,
                href: Some("/todos".into()),
                late,
                own_id: None,
            },
        );
    }
    for (day, key) in queries::targets(conn, &f, &t, tags)? {
        put(
            &day,
            view::Item {
                class: "tgt",
                time: None,
                text: format!("target · {key}"),
                sub: None,
                href: Some(format!("/epics/{key}")),
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
    Ok(view::Cal {
        month: r.month,
        week: dates::iso(r.anchor),
        tag: r.tags.join(","),
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
        cal: cal(&conn, r, st.today(), notice)?,
        has_ical: st.settings.ical_url.is_some(),
        filter: TagFilter {
            tags: r.tags.clone(),
            path: format!(
                "/calendar?week={}{}&",
                dates::iso(r.anchor),
                if r.month { "&view=month" } else { "" }
            ),
            keep: [("week", dates::iso(r.anchor))]
                .into_iter()
                .chain(r.month.then(|| ("view", "month".to_string())))
                .collect(),
            known: queries::tag_names(&conn)?,
        },
        form,
    })
}

pub async fn page(State(st): State<AppState>, Query(q): Query<RangeQuery>) -> AppResult<Response> {
    let r = Range::of(&q, st.today());
    web::html(&render_page(&st, &r, None, view::EventForm::default())?)
}

/// After an action: htmx gets the grid, a plain form gets a 303 back to the same range.
fn answer(
    st: &AppState,
    headers: &HeaderMap,
    r: &Range,
    notice: Option<view::Notice>,
) -> AppResult<Response> {
    if !web::is_htmx(headers) {
        return Ok(web::back(&r.href(r.anchor)));
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
    week: Option<String>,
    view: Option<String>,
    tag: Option<String>,
}

pub async fn add(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<AddForm>,
) -> AppResult<Response> {
    let r = Range::of(
        &RangeQuery {
            week: f.week.clone(),
            view: f.view.clone(),
            tag: f.tag.clone(),
            add: None,
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
    // Land on the range holding the new event, so it's visible.
    let r = Range { anchor: date, ..r };
    answer(&st, &headers, &r, None)
}

#[derive(Deserialize)]
pub struct RangeForm {
    week: Option<String>,
    view: Option<String>,
    tag: Option<String>,
}

fn range_of(st: &AppState, f: RangeForm) -> Range {
    Range::of(
        &RangeQuery {
            week: f.week,
            view: f.view,
            tag: f.tag,
            add: None,
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
