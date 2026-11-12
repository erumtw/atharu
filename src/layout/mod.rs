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
}

impl Layout {
    pub fn load(
        conn: &rusqlite::Connection,
        nav: &'static str,
        title: impl Into<String>,
        crumbs: Vec<Crumb>,
    ) -> rusqlite::Result<Layout> {
        Ok(Layout {
            title: title.into(),
            nav,
            crumbs,
            epics: queries::active_epics(conn)?,
        })
    }
}
