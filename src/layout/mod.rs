//! Page chrome shared by every slice: sidebar (pages + active epics) and breadcrumb.
mod queries;

pub struct Crumb {
    pub label: String,
    pub href: String,
}

pub fn crumb(label: impl Into<String>, href: impl Into<String>) -> Crumb {
    Crumb {
        label: label.into(),
        href: href.into(),
    }
}

pub struct SideEpic {
    pub key: String,
    pub title: String,
}

pub struct Layout {
    pub title: String,
    /// Sidebar item to highlight (`today`, `epics`, ...).
    pub nav: &'static str,
    /// Parent pages; the page's own title is the last, unlinked crumb.
    pub crumbs: Vec<Crumb>,
    pub epics: Vec<SideEpic>,
    /// Open to-dos, the count beside "To-dos".
    pub open_plans: i64,
    /// `data-look` + favicon (spec "Looks").
    pub look: &'static str,
    /// `data-glass="all"`: the sheet and its cards are glass too (D17, off by default).
    pub glass_all: bool,
    /// `data-tint="tinted"`: the Tinted preset's colour + contrast tokens.
    pub tinted: bool,
    /// `data-blur="text"`: text boxes get the glass blur too (D25, off by default).
    pub text_blur: bool,
    /// `--glass-a` in %, as set (may sit below the look's AA floor: owner's choice).
    pub glass_a: u8,
}

impl Layout {
    pub fn load(
        conn: &rusqlite::Connection,
        nav: &'static str,
        title: impl Into<String>,
        crumbs: Vec<Crumb>,
    ) -> rusqlite::Result<Layout> {
        let s = queries::looks(conn)?;
        let get = |k: &str| s.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
        let look = crate::domain::look::pick(get("look"));
        Ok(Layout {
            title: title.into(),
            nav,
            crumbs,
            epics: queries::active_epics(conn)?,
            open_plans: queries::open_plans(conn)?,
            look: look.slug,
            glass_all: get("glass_all") == Some("1"),
            tinted: get("glass_tint") == Some("tinted"),
            text_blur: get("text_blur") == Some("1"),
            glass_a: crate::domain::look::glass_alpha(get("glass_opacity")),
        })
    }
}

/// The "Tag" filter row on a list page (`?tag=a,b`, any of them; D29). `path` is the page with its
/// other params already in it, so each × link and the add form keep them.
pub struct TagFilter {
    pub tags: Vec<String>,
    /// e.g. `/todos?` or `/?day=2026-11-12&group=tag&`: ready for `tag=…` to be appended.
    pub path: String,
    /// Hidden fields the add form carries (the page's other params).
    pub keep: Vec<(&'static str, String)>,
    /// Known tags, most used first, for the add box's datalist.
    pub known: Vec<String>,
}

impl TagFilter {
    /// `tag` (comma list) plus `add` (the add box) → the active filter, folded and deduplicated.
    pub fn pick(tag: Option<&str>, add: Option<&str>) -> Vec<String> {
        let both = format!("{},{}", tag.unwrap_or(""), add.unwrap_or(""));
        crate::domain::tags::parse_list(&both)
    }

    /// The page without tag `t` in the filter.
    pub fn without(&self, t: &str) -> String {
        let rest: Vec<&str> = self
            .tags
            .iter()
            .filter(|x| *x != t)
            .map(String::as_str)
            .collect();
        if rest.is_empty() {
            self.path.trim_end_matches(['?', '&']).to_string()
        } else {
            format!("{}tag={}", self.path, rest.join(","))
        }
    }

    pub fn joined(&self) -> String {
        self.tags.join(",")
    }
}

/// The Tags prop on an epic, meeting series or topic (D29): chips + an add box. Posts `tags` (the
/// list as shown) + `tags_add` / `tags_drop`; the slice folds them with `domain::tags::edit_list`.
pub struct TagField {
    /// The add box's id (its `<label for>`).
    pub id: &'static str,
    pub tags: Vec<String>,
    /// Known tags, for the add box's datalist.
    pub known: Vec<String>,
}

impl TagField {
    pub fn joined(&self) -> String {
        self.tags.join(",")
    }
}

/// How a note reaches a page in its Linked references (`partials/refs.html`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// `[[page]]` in the note's body.
    Mention,
    /// The note of one of the page's own lines.
    Line,
    /// `#page` on the note.
    Tag,
}

/// A note in a page's Linked references.
pub struct Ref {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub via: Via,
}

impl Ref {
    pub fn via_label(&self) -> &'static str {
        match self.via {
            Via::Mention => "mentions it",
            Via::Line => "on a line",
            Via::Tag => "tagged",
        }
    }
}

/// Where a page's own lines are, for "notes on its lines" (entry-id SQL, `?2` = the argument).
pub enum Lines<'a> {
    Epic(&'a str),
    Meeting(i64),
    Series(&'a str),
    Topic(&'a str),
    Tag(&'a str),
}

/// Linked references of a page called `name` (`None`: only notes on its lines, e.g. one meeting, whose name is
/// its series'), shared by every entity page so `[[X]]`, `#X` and line notes always land the same way.
pub fn refs(
    conn: &rusqlite::Connection,
    name: Option<&str>,
    lines: Option<Lines>,
) -> rusqlite::Result<Vec<Ref>> {
    let (sql, arg): (Option<&str>, Box<dyn rusqlite::ToSql>) = match lines {
        None => (None, Box::new(rusqlite::types::Null)),
        Some(Lines::Epic(k)) => (
            Some("SELECT id FROM entries WHERE epic_key = ?2"),
            Box::new(k.to_string()),
        ),
        Some(Lines::Meeting(id)) => (
            Some("SELECT id FROM entries WHERE meeting_id = ?2"),
            Box::new(id),
        ),
        Some(Lines::Series(n)) => (
            Some(
                "SELECT id FROM entries WHERE meeting_id IN (SELECT id FROM meetings WHERE name = ?2)",
            ),
            Box::new(n.to_string()),
        ),
        Some(Lines::Topic(s)) => (
            Some("SELECT id FROM entries WHERE topic = ?2"),
            Box::new(s.to_string()),
        ),
        Some(Lines::Tag(t)) => (
            Some("SELECT entry_id FROM entry_tags WHERE tag = ?2"),
            Box::new(t.to_string()),
        ),
    };
    queries::refs(conn, name, sql, arg.as_ref())
}

/// Linked references grouped for the panel, in reading order; empty groups dropped.
pub fn ref_groups(refs: &[Ref]) -> Vec<(&'static str, Vec<&Ref>)> {
    [
        ("Mentioned in", Via::Mention),
        ("On its lines", Via::Line),
        ("Tagged", Via::Tag),
    ]
    .into_iter()
    .map(|(label, via)| {
        (
            label,
            refs.iter().filter(|r| r.via == via).collect::<Vec<_>>(),
        )
    })
    .filter(|(_, g)| !g.is_empty())
    .collect()
}
