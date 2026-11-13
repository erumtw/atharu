//! To-dos (`>` lines): due buckets, link chip labels, and the row every page renders the same way.
use super::dates;
use super::entry::{Entry, Kind, NoteRef, is_epic_key};
use super::parse::{ParseError, parse};
use chrono::Datelike;
use chrono::{Days, NaiveDate};

/// Pinned open plans allowed at once ("this week top 3").
pub const PIN_CAP: i64 = 3;

/// An open, done or dropped plan as the rows show it.
#[derive(Clone)]
pub struct Plan {
    pub id: i64,
    /// Day it was logged; its age on Today counts from here.
    pub day: NaiveDate,
    pub epic_key: Option<String>,
    pub text: String,
    pub link: Option<String>,
    pub due: Option<NaiveDate>,
    pub pinned: bool,
    /// `open` / `done` / `dropped`.
    pub state: String,
    pub closed_day: Option<NaiveDate>,
    /// `OPS-101 · summary` from the Jira cache when `link` is a Jira issue we've fetched.
    pub jira: Option<String>,
    /// Under a tag filter: the epic / series the matching tag comes from, when the text lacks it.
    pub via: Option<String>,
    pub note: Option<NoteRef>,
    /// `me`, or the agent that filed it (its @chip).
    pub author: String,
}

impl Plan {
    pub fn is_open(&self) -> bool {
        self.state == "open"
    }

    pub fn link_label(&self) -> String {
        match (&self.jira, self.link.as_deref()) {
            (Some(j), _) => j.clone(),
            (None, Some(l)) => link_label(l),
            (None, None) => String::new(),
        }
    }

    /// The text as a row shows it: the link's URL left out, since the ↗ chip beside it already carries it. A line
    /// that is only its URL keeps it, so the row still has text to click and edit.
    pub fn shown_text(&self) -> String {
        let Some(l) = self.link.as_deref() else {
            return self.text.clone();
        };
        let rest = self
            .text
            .split(' ')
            .filter(|w| *w != l)
            .collect::<Vec<_>>()
            .join(" ");
        if rest.trim().is_empty() {
            self.text.clone()
        } else {
            rest
        }
    }

    pub fn due_iso(&self) -> String {
        self.due.map(dates::iso).unwrap_or_default()
    }

    /// `Fri 13 Nov`.
    pub fn due_label(&self) -> String {
        self.due.map(dates::label).unwrap_or_default()
    }

    /// The row's quiet due date (owner, 2026-11-14: always shown, like before 0.2.0): `Today` · `Tomorrow` · the weekday
    /// within the coming week · `Wed 18 Nov` this year · ISO beyond. Late dates keep the day so you see how late.
    pub fn due_quiet(&self, today: &NaiveDate) -> String {
        self.due.map(|d| quiet(d, *today)).unwrap_or_default()
    }

    /// The Jira issue this to-do links, when its cache row exists (so Jira is set up): its ▾ moves it.
    pub fn jira_key(&self) -> Option<&str> {
        // `jira` is `KEY · summary`, matched from the link by the cache join (any Jira host).
        self.jira
            .as_deref()?
            .split(" · ")
            .next()
            .filter(|k| is_epic_key(k))
    }

    pub fn is_late(&self, today: &NaiveDate) -> bool {
        self.is_open() && self.due.is_some_and(|d| d < *today)
    }

    /// `6 days old`; empty on the day it was logged.
    pub fn age(&self, today: &NaiveDate) -> String {
        match (*today - self.day).num_days() {
            n if n < 1 => String::new(),
            1 => "1 day old".into(),
            n => format!("{n} days old"),
        }
    }

    /// `Tue`, the closed day in the "Done this week" list.
    pub fn closed_label(&self) -> String {
        self.closed_day
            .map(|d| d.format("%a").to_string())
            .unwrap_or_default()
    }
}

/// `/todos` groups, in page order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Overdue,
    Today,
    ThisWeek,
    Later,
    NoDate,
}

impl Bucket {
    pub const ALL: [Bucket; 5] = [
        Bucket::Overdue,
        Bucket::Today,
        Bucket::ThisWeek,
        Bucket::Later,
        Bucket::NoDate,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Bucket::Overdue => "Overdue",
            Bucket::Today => "Today",
            Bucket::ThisWeek => "This week",
            Bucket::Later => "Later",
            Bucket::NoDate => "No date",
        }
    }

    /// "This week" ends Sunday (Monday-start weeks, same as the report).
    pub fn of(due: Option<NaiveDate>, today: NaiveDate) -> Bucket {
        let Some(d) = due else {
            return Bucket::NoDate;
        };
        let week_end = dates::week_start(today) + Days::new(6);
        if d < today {
            Bucket::Overdue
        } else if d == today {
            Bucket::Today
        } else if d <= week_end {
            Bucket::ThisWeek
        } else {
            Bucket::Later
        }
    }
}

/// Chip text for a source link: Jira · Slack · GitHub · Docs, else the bare domain.
/// A date the way you'd say it next to today: `Today` · `Tomorrow` · the weekday within the coming week ·
/// `Wed 18 Nov` this year · ISO beyond (the to-do row's quiet due date, the Jira due on Epics).
pub fn quiet(d: NaiveDate, today: NaiveDate) -> String {
    match (d - today).num_days() {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        2..=6 => d.format("%a").to_string(),
        _ if d.year() == today.year() => d.format("%a %-d %b").to_string(),
        _ => d.format("%Y-%m-%d").to_string(),
    }
}

pub fn link_label(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let is = |d: &str| host == d || host.ends_with(&format!(".{d}"));
    if is("atlassian.net") || host.split('.').any(|p| p == "jira") {
        "Jira".into()
    } else if is("slack.com") {
        "Slack".into()
    } else if is("github.com") {
        "GitHub".into()
    } else if is("docs.google.com") {
        "Docs".into()
    } else {
        host.strip_prefix("www.").unwrap_or(&host).to_string()
    }
}

/// Issue key in a Jira link (`…/browse/OPS-101`, `?selectedIssue=OPS-101`); `None` for other hosts.
pub fn jira_key(url: &str) -> Option<&str> {
    if link_label(url) != "Jira" {
        return None;
    }
    url.split(['/', '?', '&', '=', '#'])
        .find(|p| is_epic_key(p))
}

/// Plan hit rate in whole percent (done ÷ total, rounded half up); `None` with no plans.
pub fn hit_rate(done: i64, total: i64) -> Option<i64> {
    (total > 0).then(|| (done * 200 + total) / (2 * total))
}

/// The To-dos box: every line is a to-do, so a missing `>` is added after the optional key.
pub fn parse_plan(line: &str, today: NaiveDate) -> Result<Entry, ParseError> {
    let e = parse(line, today)?;
    if e.kind == Kind::Plan {
        return Ok(e);
    }
    let t = line.trim();
    let fixed = match t.split_once(char::is_whitespace) {
        Some((key, rest)) if is_epic_key(key) => format!("{key} > {rest}"),
        _ => format!("> {t}"),
    };
    parse(&fixed, today)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn due_quiet_reads_the_date_the_way_you_say_it() {
        let today = d("2026-11-14"); // Saturday
        let quiet = |due: &str| {
            Plan {
                id: 1,
                day: today,
                epic_key: None,
                text: "x".into(),
                link: None,
                due: Some(d(due)),
                pinned: false,
                state: "open".into(),
                closed_day: None,
                jira: None,
                via: None,
                note: None,
                author: "me".into(),
            }
            .due_quiet(&today)
        };
        assert_eq!(quiet("2026-11-14"), "Today");
        assert_eq!(quiet("2026-11-15"), "Tomorrow");
        assert_eq!(quiet("2026-11-20"), "Fri");
        assert_eq!(quiet("2026-11-21"), "Sat 21 Nov");
        assert_eq!(quiet("2026-12-31"), "Thu 31 Dec");
        assert_eq!(quiet("2027-01-05"), "2027-01-05");
        // Late keeps the day, so you see how late.
        assert_eq!(quiet("2026-11-09"), "Mon 9 Nov");
    }

    #[test]
    fn buckets_against_fixed_today() {
        let today = d("2026-11-11"); // Wednesday; week ends Sun 15 Nov
        for (due, want) in [
            (Some("2026-11-10"), Bucket::Overdue),
            (Some("2026-11-11"), Bucket::Today),
            (Some("2026-11-15"), Bucket::ThisWeek),
            (Some("2026-11-16"), Bucket::Later),
            (None, Bucket::NoDate),
        ] {
            assert_eq!(Bucket::of(due.map(d), today), want, "{due:?}");
        }
    }

    #[test]
    fn jira_keys() {
        assert_eq!(
            jira_key("https://acme.atlassian.net/browse/OPS-12?x=1"),
            Some("OPS-12")
        );
        assert_eq!(
            jira_key(
                "https://acme.atlassian.net/jira/software/projects/OPS/boards/1?selectedIssue=OPS-7"
            ),
            Some("OPS-7")
        );
        assert_eq!(jira_key("https://github.com/acme/OPS-12"), None);
    }

    #[test]
    fn link_labels() {
        for (url, want) in [
            ("https://acme.atlassian.net/browse/OPS-1", "Jira"),
            ("https://jira.acme.io/browse/OPS-1", "Jira"),
            ("https://acme.slack.com/archives/C1/p2", "Slack"),
            ("https://github.com/o/r/pull/412", "GitHub"),
            ("https://docs.google.com/document/d/x", "Docs"),
            ("https://www.grafana.net:3000/d/p95", "grafana.net"),
            ("http://intranet/x", "intranet"),
        ] {
            assert_eq!(link_label(url), want, "{url}");
        }
    }

    #[test]
    fn hit_rate_math() {
        assert_eq!(hit_rate(4, 6), Some(67));
        assert_eq!(hit_rate(1, 3), Some(33));
        assert_eq!(hit_rate(1, 8), Some(13));
        assert_eq!(hit_rate(0, 2), Some(0));
        assert_eq!(hit_rate(5, 5), Some(100));
        assert_eq!(hit_rate(0, 0), None);
    }

    #[test]
    fn todo_box_adds_the_prefix() {
        let today = d("2026-11-11");
        let e = parse_plan("renew parking by fri", today).unwrap();
        assert_eq!((e.kind, e.text.as_str()), (Kind::Plan, "renew parking"));
        assert_eq!(e.due, Some(d("2026-11-13")));
        let e = parse_plan("OPS-101 profile join", today).unwrap();
        assert_eq!(e.epic_key.as_deref(), Some("OPS-101"));
        assert_eq!((e.kind, e.text.as_str()), (Kind::Plan, "profile join"));
        let e = parse_plan("> already a plan", today).unwrap();
        assert_eq!(e.text, "already a plan");
        assert!(parse_plan("  ", today).is_err());
    }
}
