use crate::{
    domain::{dates, entry::LineRow, plan::Plan},
    layout::{Layout, TagFilter},
};
use askama::Template;
use chrono::NaiveDate;

pub struct Group {
    pub key: Option<String>,
    pub title: String,
    pub rows: Vec<LineRow>,
    /// Group by tag (D29): `tag` heads the group, `None` = "No tag".
    pub by_tag: bool,
    /// Group by type: `title` is the kind's plural ("Decisions").
    pub by_type: bool,
    pub tag: Option<String>,
}

/// The page's day plus its neighbours, all pre-formatted for the day nav.
pub struct Day {
    pub iso: String,
    pub label: String,
    pub prev: String,
    pub prev_label: String,
    pub next: String,
    pub next_label: String,
    pub is_today: bool,
}

impl Day {
    pub fn new(d: NaiveDate, today: NaiveDate) -> Day {
        let prev = d.pred_opt().unwrap_or(d);
        let next = d.succ_opt().unwrap_or(d);
        Day {
            iso: dates::iso(d),
            label: dates::label(d),
            prev: dates::iso(prev),
            prev_label: prev.format("%a %-d").to_string(),
            next: dates::iso(next),
            next_label: next.format("%a %-d").to_string(),
            is_today: d == today,
        }
    }
}

pub struct Saved {
    pub id: i64,
    pub kind: &'static str,
    pub key: Option<String>,
    pub new_epic: bool,
    /// Tags typed in the line, named in the toast.
    pub tags: Vec<String>,
}

/// "Make tag" just turned a word in line `id` into `#tag` (D29): the toast offers the other lines.
pub struct Made {
    pub id: i64,
    pub tag: String,
    /// The word as selected, quoted in "N older lines mention ‘…’".
    pub word: String,
    /// Other lines where a plain word names the tag.
    pub older: usize,
    /// The line before the edit: Undo saves it back.
    pub undo_line: String,
    pub day: String,
}

pub struct Deleted {
    /// The whole row as JSON, posted back by Undo.
    pub snap: String,
}

pub struct Preview {
    /// Trailing `by …` kept as text (`by hand`), for the soft hint (D44).
    pub soft: Option<String>,
    pub kind: &'static str,
    pub key: Option<String>,
    pub text: String,
    pub due: Option<String>,
}

/// "Tag as #x?": the word as typed and the known tag it names.
pub struct Ask {
    pub word: String,
    pub tag: String,
}

/// The `#` menu under the capture bar: (tag, lines) most used first, plus "New tag #x" when `new` is set.
pub struct Sug {
    pub opts: Vec<(String, i64)>,
    pub new: Option<String>,
}

/// Focus (D3): the pinned top 3 plus every overdue to-do, overdue first, each once; always about the real today.
pub struct Plans {
    pub focus: Vec<Plan>,
    /// Open to-dos not in Focus (the To-dos link says how many).
    pub more: usize,
    pub due_today: usize,
    pub overdue: usize,
}

/// One "Needs you" line: a flagged epic (D3: overdue to-dos live in Focus).
pub struct Need {
    /// `flag over` / `flag risk` / `flag stale`.
    pub class: &'static str,
    pub label: &'static str,
    pub key: Option<String>,
    /// `Log retention · 6 days past target`.
    pub text: String,
    pub href: String,
}

#[derive(Template)]
#[template(path = "today/page.html")]
pub struct Page {
    pub layout: Layout,
    /// D5: open Jira issues assigned to me (a count; the list is on Epics).
    pub jira_assigned: i64,
    pub banner: crate::recording::Banner,
    pub today: NaiveDate,
    pub plans: Plans,
    pub needs: Vec<Need>,
    pub day: Day,
    pub groups: Vec<Group>,
    pub keys: Vec<String>,
    /// Kept in the box after a parse error.
    pub line: String,
    pub error: Option<String>,
    pub preview: Option<Preview>,
    pub saved: Option<Saved>,
    pub deleted: Option<Deleted>,
    pub made: Option<Made>,
    /// The log line open in its edit box (`/?edit=ID` without JS).
    pub edit: Option<Edit>,
    pub ask: Option<Ask>,
    /// `epic` or `tag`.
    pub group: &'static str,
    pub filter: TagFilter,
}

impl Page {
    /// The head's one line: `1 due today · 1 overdue · 6 lines logged · in checkout-sync` (empty parts left out).
    pub fn fact(&self) -> String {
        let mut parts = Vec::new();
        if self.plans.due_today > 0 {
            parts.push(format!("{} due today", self.plans.due_today));
        }
        if self.plans.overdue > 0 {
            parts.push(format!("{} overdue", self.plans.overdue));
        }
        // A line sits in several groups when grouped by tag: count it once.
        let lines: std::collections::HashSet<i64> = self
            .groups
            .iter()
            .flat_map(|g| g.rows.iter().map(|r| r.id))
            .collect();
        parts.push(match lines.len() {
            0 => "nothing logged yet".to_string(),
            1 => "1 line logged".to_string(),
            n => format!("{n} lines logged"),
        });
        if let Some(m) = &self.banner.state.running {
            parts.push(format!("in {}", m.name));
        }
        parts.join(" · ")
    }
}

/// Inline edit box of one log line.
#[derive(Template)]
#[template(path = "partials/entry_edit.html")]
pub struct Edit {
    pub id: i64,
    pub day: String,
    pub line: String,
    pub error: Option<String>,
    /// Opened from a to-do row (D51): it stands in for `#plan-<id>` and saves with no swap.
    pub plan: bool,
}

/// A to-do row just deleted (D51): it stays in place as "Line deleted · Undo" until the list re-fetches.
#[derive(Template)]
#[template(path = "partials/plan_gone.html")]
pub struct Gone {
    pub id: i64,
    pub text: String,
    pub snap: String,
}

/// htmx answer to save/delete: the log, plus the toast swapped out-of-band.
#[derive(Template)]
#[template(path = "today/swap.html")]
pub struct Swap {
    pub today: NaiveDate,
    /// Swapped out-of-band: a `>` line or its undo changes the open plans too.
    pub plans: Plans,
    pub day: Day,
    pub groups: Vec<Group>,
    pub saved: Option<Saved>,
    pub deleted: Option<Deleted>,
    pub made: Option<Made>,
    pub edit: Option<Edit>,
}

#[derive(Template)]
#[template(path = "partials/parse_hint.html")]
pub struct Hint {
    pub error: Option<String>,
    pub preview: Option<Preview>,
    pub ask: Option<Ask>,
}

/// Live preview: the hint, plus the `#` menu swapped out-of-band.
#[derive(Template)]
#[template(path = "today/preview.html")]
pub struct PreviewSwap {
    pub error: Option<String>,
    pub preview: Option<Preview>,
    pub ask: Option<Ask>,
    pub sug: Option<Sug>,
}
