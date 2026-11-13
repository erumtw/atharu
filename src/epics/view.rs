use crate::{
    domain::{entry::LineRow, plan::Plan},
    layout::Layout,
};
use askama::Template;
use chrono::NaiveDate;

/// `14 Nov`, the board's short date.
pub fn short(d: NaiveDate) -> String {
    d.format("%-d %b").to_string()
}

#[derive(Clone)]
pub struct ListRow {
    pub key: String,
    pub title: String,
    /// Open to-dos on it.
    pub open: i64,
    /// (before, after, metric) once all three are filled.
    pub impact: Option<(String, String, String)>,
    /// `today`, `9d ago`, or `—`.
    pub last_log: String,
    pub flag: Option<Flag>,
    /// The ↗ chip: (Jira status or `Jira`, `{base}/browse/KEY`), when Jira is configured.
    pub jira: Option<(String, String)>,
    /// Jira `summary · status`, shown on an unnamed row as a hint.
    pub jira_hint: Option<String>,
    /// The name shown is Jira's (no title typed yet, D7): marked "from Jira".
    pub from_jira: bool,
    /// Jira status category (new · indeterminate · done), for the chip's look.
    pub cat: String,
    /// Under its Jira parent (D4): indented.
    pub child: bool,
    /// A done / dropped epic shown as a row (Show = Done & dropped / All): `done 20 Oct` / `dropped`.
    pub closed_chip: Option<String>,
    /// What the filter reads, lowercased: key, title, Jira title, tags (the live filter's `data-q`).
    pub q: String,
}

/// A row of an OKR group: an epic, or the header of a Jira parent that isn't tracked (D4).
pub enum ListItem {
    Row(Box<ListRow>),
    Head(Head),
}

/// A Jira parent its tracked stories sit under; not an epic until "+ Track as epic" (D4).
pub struct Head {
    pub key: String,
    pub title: String,
    pub jira: Option<(String, String)>,
    pub cat: String,
    /// Its children's `q` joined: the live filter keeps the header while one of them shows.
    pub q: String,
}

/// "In Jira, assigned to you" (D5, D6).
pub struct AssignedRow {
    pub key: String,
    pub summary: String,
    pub issue_type: String,
    pub jira: Option<(String, String)>,
    pub cat: String,
    /// `due 4 Dec` / `due Fri`.
    pub due: Option<String>,
    pub tracked: bool,
    /// Type Epic: "+ Track as epic"; anything else: "+ To-do" with this line (D6).
    pub epic: bool,
    pub line: String,
    /// What the filter reads, lowercased: key + Jira title.
    pub q: String,
}

/// A group of "In Jira, assigned to you": a Jira status (or a Jira parent).
pub struct AssignedGroup {
    pub label: String,
    pub rows: Vec<AssignedRow>,
}

/// A flag as the epic pages show it.
#[derive(Clone)]
pub struct Flag {
    pub class: &'static str,
    pub label: &'static str,
    pub reason: String,
}

/// One group of the epics list, by the picked "Group by": an OKR, a Jira status, a Jira parent, a tag, or all.
pub struct Group {
    pub label: String,
    pub items: Vec<ListItem>,
    /// `2 active` · `2 of 5 active` · `3` · `3 of 7`: the summary's count (header rows aren't epics).
    pub count: String,
}

/// The page's view: what the control row sets, all in the URL (defaults left out).
#[derive(Clone, PartialEq, Debug)]
pub struct Params {
    /// As typed (trimmed, ≤ 80 chars), for the box.
    pub q: String,
    pub group: GroupBy,
    pub show: Show,
    /// "In Jira, assigned to you" without what is already tracked.
    pub hide_tracked: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GroupBy {
    Okr,
    Status,
    Parent,
    Tag,
    None,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Show {
    Active,
    Closed,
    All,
}

impl GroupBy {
    /// Unknown → the OKR grouping the page always had.
    pub fn parse(s: Option<&str>) -> GroupBy {
        match s {
            Some("status") => GroupBy::Status,
            Some("parent") => GroupBy::Parent,
            Some("tag") => GroupBy::Tag,
            Some("none") => GroupBy::None,
            _ => GroupBy::Okr,
        }
    }
    /// The query value; `okr` is the default.
    pub fn key(self) -> &'static str {
        match self {
            GroupBy::Okr => "okr",
            GroupBy::Status => "status",
            GroupBy::Parent => "parent",
            GroupBy::Tag => "tag",
            GroupBy::None => "none",
        }
    }
}

impl Show {
    /// Unknown → active epics only, as before.
    pub fn parse(s: Option<&str>) -> Show {
        match s {
            Some("closed") => Show::Closed,
            Some("all") => Show::All,
            _ => Show::Active,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Show::Active => "active",
            Show::Closed => "closed",
            Show::All => "all",
        }
    }
}

impl Params {
    pub fn parse(
        q: Option<&str>,
        group: Option<&str>,
        show: Option<&str>,
        hide: Option<&str>,
    ) -> Params {
        Params {
            q: q.unwrap_or_default().trim().chars().take(80).collect(),
            group: GroupBy::parse(group),
            show: Show::parse(show),
            hide_tracked: hide == Some("1"),
        }
    }
    /// The lowercased needle; empty = no filter.
    pub fn needle(&self) -> String {
        self.q.to_lowercase()
    }
    /// A filter is narrowing the lists ("N of M").
    pub fn filtering(&self) -> bool {
        !self.q.is_empty()
    }
}

pub struct Closed {
    pub key: String,
    pub title: String,
    pub done: bool,
    /// `done 20 Oct` or `dropped`.
    pub chip: String,
    pub jira: Option<String>,
}

#[derive(Template)]
#[template(path = "epics/list.html")]
pub struct List {
    pub layout: Layout,
    /// Unnamed stubs, at the bottom (named in place).
    pub untitled: Vec<ListRow>,
    /// `2 active · 1 done this month · 1 unnamed`.
    pub fact: String,
    /// Jira configured: show the Refresh button.
    pub jira_on: bool,
    pub jira_failed: bool,
    /// The Refresh button just ran and Jira answered (`?jira=ok`): say so, even when nothing changed.
    pub jira_done: bool,
    /// "updated 3 min ago" beside Refresh, so the button's effect is visible.
    pub jira_age: Option<String>,
    pub groups: Vec<Group>,
    pub closed: Vec<Closed>,
    pub assigned: Vec<AssignedGroup>,
    /// `4 open` · `2 of 4 open`; and whether Jira has any assigned work at all (else no section).
    pub assigned_count: String,
    pub assigned_any: bool,
    /// The control row's current values.
    pub p: Params,
    /// D9: Jira refused the token on the last call; the page keeps the last copy and says so.
    pub dead_token: bool,
    pub token_url: &'static str,
}

/// htmx answer to the inline title box: the row, now named.
#[derive(Template)]
#[template(path = "epics/named.html")]
pub struct Named {
    pub key: String,
    pub title: String,
}

/// Property values as the inputs want them (`""` = empty date).
pub struct EpicView {
    pub key: String,
    pub title: String,
    pub role: String,
    pub okr: String,
    pub status: String,
    pub start: String,
    pub target: String,
    pub done_on: String,
    /// Jira `summary · status`, shown while `title` is empty.
    pub jira: Option<String>,
    /// The ↗ chip (status or `Jira`, issue URL), when Jira is configured.
    pub jira_link: Option<(String, String)>,
    /// The head's kind: Jira's issue type (Story, Bug, …) or `Epic`.
    pub kind: String,
    /// Story head (D3): "under PARENT title ↗" — (key, title, href: its page when tracked, else Jira).
    pub parent: Option<(String, String, String)>,
    pub cat: String,
}

/// A Jira child row in "Tickets · N in Jira · M logged here" (D3).
pub struct Ticket {
    pub key: String,
    pub summary: String,
    pub issue_type: String,
    pub jira: Option<(String, String)>,
    pub cat: String,
    pub lines: i64,
    /// Its own page here when tracked, else Jira.
    pub href: String,
}

pub struct Evidence {
    pub id: i64,
    pub url: String,
    pub note: String,
    pub author: String,
}

/// Impact line + evidence: the part of the epic page that changes when a property or evidence does.
pub struct Live {
    pub key: String,
    pub metric: String,
    pub before: String,
    pub after: String,
    pub impact: Option<String>,
    pub evidence: Vec<Evidence>,
    /// Kept in the evidence box after a rejected add.
    pub ev_line: String,
    pub ev_error: Option<String>,
    /// Right after a save made it done / dropped: ask "Also move KEY in Jira?" (D2, D10).
    pub suggest: Option<&'static str>,
}

/// A timeline row: to-dos draw as `plan_row.html` (check, Pin, Drop), every other line as `entry_row.html`.
pub enum Item {
    Line(LineRow),
    Plan(Plan),
}

pub struct DayGroup {
    pub label: String,
    pub rows: Vec<Item>,
}

#[derive(Template)]
#[template(path = "epics/page.html")]
pub struct Page {
    pub layout: Layout,
    pub field: crate::layout::TagField,
    pub epic: EpicView,
    pub flag: Option<Flag>,
    /// Entries per week, last 8 weeks, as % of the busiest.
    pub activity: Vec<i64>,
    pub live: Live,
    /// Open to-dos, pulled out of the timeline into their own section.
    pub todos: Vec<Plan>,
    pub timeline: Vec<DayGroup>,
    /// The head's read-only fact line.
    pub fact: String,
    /// For the to-do rows' due / late labels.
    pub today: NaiveDate,
    /// Top-bar status after a plain (no-JS) save failed: the reason.
    pub save_error: Option<&'static str>,
    /// Linked references: notes with `[[name]]` or `#name`, and notes on this page's lines.
    pub refs: Vec<crate::layout::Ref>,
    /// The `[[…]]` name the empty Linked references line suggests.
    pub ref_name: String,
    /// A plain (no-JS) delete was refused.
    pub delete_error: Option<&'static str>,
    /// D2: "Jira says KEY is Done on <date>" while it's active here.
    pub jira_done: Option<String>,
    pub tickets: Vec<Ticket>,
}

/// htmx answer to a property / evidence change: the live block, plus the top-bar status out-of-band.
#[derive(Template)]
#[template(path = "epics/swap.html")]
pub struct Swap {
    pub live: Live,
    pub save_error: Option<&'static str>,
    /// The Tags prop, swapped out-of-band: a chip edit changes it.
    pub field: Option<crate::layout::TagField>,
    /// The "done on" input, out-of-band, when this save filled the date in.
    pub done_on: Option<String>,
}

impl Page {
    /// Tickets with at least one line logged on their key (D3).
    pub fn logged(&self) -> usize {
        self.tickets.iter().filter(|t| t.lines > 0).count()
    }
}
