use crate::{
    domain::plan::Plan,
    layout::{Layout, TagFilter},
};
use askama::Template;
use chrono::NaiveDate;

pub struct Group {
    /// Due bucket (`Overdue`), or `No tag`; empty for a tag group.
    pub title: String,
    pub tag: Option<String>,
    pub plans: Vec<Plan>,
}

/// The `#todos` section: open groups + "Done this week".
pub struct Lists {
    pub groups: Vec<Group>,
    pub done: Vec<Plan>,
}

impl Lists {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

#[derive(Template)]
#[template(path = "todos/page.html")]
pub struct Page {
    pub layout: Layout,
    pub today: NaiveDate,
    pub lists: Lists,
    pub keys: Vec<String>,
    /// Kept in the add box after a parse error.
    pub line: String,
    pub error: Option<String>,
    /// A plan action that failed without JS (pin cap), shown above the lists.
    pub notice: Option<String>,
    pub filter: TagFilter,
    /// Group by tag instead of due date.
    pub by_tag: bool,
    /// Open to-dos in all / shown under the filter ("3 of 11 open to-dos").
    pub open_total: usize,
    pub shown: usize,
}

/// htmx answer to "Add": the lists, so the new row lands in its due group.
#[derive(Template)]
#[template(path = "todos/lists.html")]
pub struct ListsSwap {
    pub today: NaiveDate,
    pub lists: Lists,
}

/// htmx answer to a plan action: just that row.
#[derive(Template)]
#[template(path = "todos/row.html")]
pub struct Row {
    pub today: NaiveDate,
    pub p: Plan,
    pub back: String,
    pub full: bool,
    pub pin_error: bool,
    /// Just ticked done: "what's next?" under the row; the list regroups once it's answered.
    pub follow: Option<Follow>,
    /// Just made done / dropped and it links a Jira issue: (key, `done` | `dropped`) loads "Also move KEY in Jira?"
    /// into the page's #jtoast (D2, D10), where a regroup can't sweep it away.
    pub jira_ask: Option<(String, &'static str)>,
}

/// The follow-up line under a ticked to-do (htmx only): carries its epic and #tags into the next to-do.
#[derive(Template)]
#[template(path = "todos/follow.html")]
pub struct Follow {
    pub id: i64,
    pub key: Option<String>,
    pub tags: Vec<String>,
    pub line: String,
    pub error: Option<String>,
}

pub struct Preview {
    /// Trailing `by …` kept as text (`by hand`), for the soft hint (D44).
    pub soft: Option<String>,
    pub key: Option<String>,
    pub link: Option<String>,
    pub due: Option<String>,
}

#[derive(Template)]
#[template(path = "todos/hint.html")]
pub struct Hint {
    pub error: Option<String>,
    pub preview: Option<Preview>,
}
