use super::{queries, view};
use crate::{
    app::AppState,
    domain::{dates, impact::impact_line},
    error::AppResult,
    layout::Layout,
    web,
};
use askama::Template;
use axum::{
    extract::{Query, State},
    response::Response,
};
use chrono::{Datelike, Days, NaiveDate};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct RangeQuery {
    preset: Option<String>,
    from: Option<String>,
    to: Option<String>,
}

fn date(s: Option<&str>) -> Option<NaiveDate> {
    s.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

/// Both dates given (the range inputs) win over a preset; anything else falls back to this week.
fn range(q: &RangeQuery, today: NaiveDate) -> (&'static str, NaiveDate, NaiveDate) {
    if let (None, Some(a), Some(b)) = (&q.preset, date(q.from.as_deref()), date(q.to.as_deref())) {
        return ("", a.min(b), a.max(b));
    }
    match q.preset.as_deref() {
        Some("month") => {
            let s = dates::month_start(today);
            ("month", s, dates::end_of(s, 1))
        }
        Some("quarter") => {
            let s = dates::quarter_start(today);
            ("quarter", s, dates::end_of(s, 3))
        }
        _ => {
            let s = dates::week_start(today);
            ("week", s, s + Days::new(6))
        }
    }
}

fn title(preset: &str, from: NaiveDate, to: NaiveDate) -> String {
    match preset {
        "week" => format!("Week of {}", from.format("%-d %b")),
        "month" => from.format("%B %Y").to_string(),
        "quarter" => format!("Q{} {}", from.month0() / 3 + 1, from.year()),
        _ => format!("{} – {}", from.format("%-d %b %Y"), to.format("%-d %b %Y")),
    }
}

fn label(kind: &str) -> &'static str {
    match kind {
        "impact" => "Impact",
        "decision" => "Decision",
        _ => "Blocker",
    }
}

pub async fn page(State(st): State<AppState>, Query(q): Query<RangeQuery>) -> AppResult<Response> {
    let (preset, from, to) = range(&q, st.today());
    let (f, t) = (dates::iso(from), dates::iso(to));
    let conn = st.conn();

    let shipped: Vec<String> = queries::done_epics(&conn, &f, &t)?
        .into_iter()
        .map(|e| {
            let name = if e.title.is_empty() { &e.key } else { &e.title };
            impact_line(name, &e.metric, &e.before, &e.after, &e.urls)
                .unwrap_or_else(|| format!("Did {name}"))
        })
        .collect();

    let notable = queries::notable(&conn, &f, &t)?;
    let impact_lines = notable.iter().filter(|n| n.kind == "impact").count();
    let decisions = notable.iter().filter(|n| n.kind == "decision").count();
    let mut sections: Vec<(Option<String>, view::Section)> = Vec::new();
    for n in notable {
        let item = view::Item {
            label: label(&n.kind),
            text: n.text,
        };
        // Rows come sorted by epic, so a new key always means a new section.
        match sections.last_mut() {
            Some((k, s)) if *k == n.epic_key => s.items.push(item),
            _ => {
                let heading = match &n.epic_key {
                    None => "Not filed under an epic".to_string(),
                    Some(k) if n.epic_title.is_empty() => k.clone(),
                    Some(k) => format!("{k} · {}", n.epic_title),
                };
                sections.push((
                    n.epic_key,
                    view::Section {
                        heading,
                        items: vec![item],
                    },
                ));
            }
        }
    }

    let note = view::Note {
        title: title(preset, from, to),
        shipped,
        sections: sections.into_iter().map(|(_, s)| s).collect(),
    };
    let markdown = view::NoteMd { note: &note }.render()?;

    let mut okrs: Vec<view::OkrBar> = Vec::new();
    for (okr, status, n) in queries::okr_status(&conn)? {
        let okr = if okr.is_empty() {
            "No OKR".to_string()
        } else {
            okr
        };
        let color = match status.as_str() {
            "active" => "#0066cc",
            "done" => "#4d8fd1",
            _ => "#b0b0b5",
        };
        let part = format!("{n} {status}");
        let seg = view::Seg { flex: n, color };
        match okrs.last_mut() {
            Some(b) if b.okr == okr => {
                b.summary = format!("{}, {part}", b.summary);
                b.segs.push(seg);
            }
            _ => okrs.push(view::OkrBar {
                okr,
                summary: part,
                segs: vec![seg],
            }),
        }
    }

    // One bucket per Monday-started week touching the range.
    let mut weeks: Vec<(NaiveDate, i64)> = Vec::new();
    let mut w = dates::week_start(from);
    while w <= to {
        weeks.push((w, 0));
        w = w + Days::new(7);
    }
    for d in queries::impact_days(&conn, &f, &t)? {
        if let Some(d) = date(Some(&d)) {
            let i = ((dates::week_start(d) - dates::week_start(from)).num_days() / 7) as usize;
            if let Some(b) = weeks.get_mut(i) {
                b.1 += 1;
            }
        }
    }
    let max = weeks.iter().map(|w| w.1).max().unwrap_or(0).max(1);
    let wk = |w: &(NaiveDate, i64)| format!("{} · {}", w.0.format("%-d %b"), w.1);
    let week_bars = view::WeekBars {
        pct: weeks.iter().map(|w| w.1 * 100 / max).collect(),
        first: weeks.first().map(wk).unwrap_or_default(),
        last: weeks.last().map(wk).unwrap_or_default(),
    };

    web::html(&view::Page {
        layout: Layout::load(&conn, "report", "Report", vec![])?,
        preset,
        from: f,
        to: t,
        epics_done: note.shipped.len(),
        impact_lines,
        decisions,
        note,
        markdown,
        wider: match preset {
            "week" => Some(("month", "This month")),
            "quarter" => None,
            _ => Some(("quarter", "This quarter")),
        },
        okrs,
        weeks: week_bars,
    })
}
