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
    /// `--glass-a` in %, already clamped to the look's AA floor.
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
            glass_a: crate::domain::look::glass_alpha(look, get("glass_opacity")),
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
