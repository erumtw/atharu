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
            glass_a: crate::domain::look::glass_alpha(look, get("glass_opacity")),
        })
    }
}
