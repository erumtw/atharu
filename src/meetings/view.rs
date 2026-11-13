use crate::recording::Banner;
use crate::{
    domain::{entry::LineRow, plan::Plan},
    layout::Layout,
};
use askama::Template;
use chrono::NaiveDate;

/// Answer to the Record switch turning on: the Screen Recording step when it's still off (empty when allowed).
#[derive(Template)]
#[template(path = "meetings/permission.html")]
pub struct Permission {
    pub needed: bool,
}

/// htmx answer to start / end.
#[derive(Template)]
#[template(path = "meetings/banner.html")]
pub struct BannerSwap {
    pub banner: Banner,
}

pub struct ListRow {
    pub id: i64,
    pub day: String,
    /// `15:02–15:20`, `15:02 · running`, or empty (no start time recorded): tells same-day meetings apart.
    pub time: String,
    /// `16 lines` of transcript, or empty.
    pub transcript: String,
    pub decisions: i64,
    /// `1 open · 1 done`, or `—`.
    pub actions: String,
    pub running: bool,
}

/// One series on the Meetings list (one row each; its page lists the meetings).
pub struct SeriesItem {
    pub name: String,
    pub meetings: i64,
    /// `today` / `Mon 9 Nov`.
    pub last: String,
    pub open: i64,
}

#[derive(Template)]
#[template(path = "meetings/list.html")]
pub struct List {
    pub layout: Layout,
    pub banner: Banner,
    pub series: Vec<SeriesItem>,
    /// Archived series, folded under the list (D4).
    pub archived: Vec<SeriesItem>,
    /// `3 series · 4 meetings · last met today`.
    pub fact: String,
}

/// `/meetings?name=X`: one series, on the shared page template.
#[derive(Template)]
#[template(path = "meetings/series.html")]
pub struct Series {
    pub layout: Layout,
    pub name: String,
    /// `2 meetings · last met today`.
    pub fact: String,
    pub archived: bool,
    pub rows: Vec<ListRow>,
    pub plans: Vec<Plan>,
    pub today: NaiveDate,
    pub back: String,
    /// The series Tags prop posts through its latest meeting (`meetings/tags.html` needs `id`).
    pub id: i64,
    pub field: crate::layout::TagField,
    pub same: crate::layout::SameName,
    pub refs: Vec<crate::layout::Ref>,
    pub ref_name: String,
}

pub struct KindGroup {
    pub title: &'static str,
    pub lines: Vec<LineRow>,
}

#[derive(Template)]
#[template(path = "meetings/page.html")]
pub struct Page {
    pub layout: Layout,
    pub id: i64,
    pub name: String,
    /// `Wed 11 Nov 2026`.
    pub day: String,
    /// `None` = still running; else `HH:MM` it ended.
    pub ended: Option<String>,
    /// Up to two earlier/later meetings of the series: (id, label).
    pub siblings: Vec<(i64, String)>,
    pub series_count: usize,
    pub plans: Vec<Plan>,
    pub groups: Vec<KindGroup>,
    /// Its note (T90): (id, title, markdown), edited in place with the Notes editor; None = "Start notes".
    pub note: Option<(i64, String, String)>,
    pub today: NaiveDate,
    pub back: String,
    /// Notes linking here with `[[…]]`: (id, title).
    pub refs: Vec<crate::layout::Ref>,
    /// The `[[…]]` name the empty Linked references line suggests.
    pub ref_name: String,
    /// The series' Tags prop.
    pub field: crate::layout::TagField,
    /// Tag hub / topic of the series' name.
    pub same: crate::layout::SameName,
    /// Shown on top while this meeting is recording (D31), so Stop is where you are.
    pub banner: Option<Banner>,
    pub tx: TxSection,
    pub shots: Shots,
}

/// Screenshots section (T89): file names + whether this view can take one (the Mac app, meeting running).
#[derive(Template)]
#[template(path = "meetings/shots.html")]
pub struct Shots {
    pub id: i64,
    pub files: Vec<String>,
    pub can_take: bool,
}

impl Shots {
    pub fn label(&self, f: &str) -> String {
        super::shots::label(f)
    }
}

#[derive(Template)]
#[template(path = "meetings/tags.html")]
pub struct Tags {
    pub id: i64,
    pub field: crate::layout::TagField,
}

#[derive(Template)]
#[template(path = "meetings/saved.html")]
pub struct Saved;

pub struct TxRow {
    /// `00:01:02` from the start of the recording.
    pub time: String,
    pub who: String,
    /// Same speaker as the row above: the name is hidden.
    pub cont: bool,
    /// Text split around `?q=` hits: (text, is a hit).
    pub parts: Vec<(String, bool)>,
}

pub struct TxView {
    pub id: i64,
    /// Muted head line: `Live · about 30 s behind · th · 8 lines`.
    pub meta: String,
    pub rows: Vec<TxRow>,
    /// Plain text for Copy: `00:00:04 Me: …` per line, the search ignored.
    pub copy: String,
    pub lines: usize,
    pub live: bool,
    /// Tail still transcribing: (percent done, `about 3 min`).
    pub finishing: Option<(u8, String)>,
    /// Closed while recording: transcribed up to `hh:mm:ss`.
    pub interrupted: Option<String>,
    pub done: bool,
    /// Split voices the owner can name (done recordings only): `Speaker 1`, or the name given.
    pub speakers: Vec<String>,
}

/// The Transcript section of a meeting page; polled while a recording is live or finishing.
#[derive(Template)]
#[template(path = "meetings/transcript.html")]
pub struct TxSection {
    pub meeting: i64,
    pub q: String,
    /// Inside Atharu.app (else the view-only browser copy).
    pub mac: bool,
    pub list: Vec<TxView>,
    pub polling: bool,
}
