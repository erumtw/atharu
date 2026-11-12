use crate::{
    domain::{entry::Kind, plan::Plan},
    layout::Layout,
};
use askama::Template;
use chrono::NaiveDate;

pub struct Active {
    pub id: i64,
    pub name: String,
}

/// Start box or running banner (`partials/meeting_banner.html`); Today renders the same partial.
pub struct Banner {
    pub active: Option<Active>,
    pub series: Vec<String>,
    /// Kept in the box after a rejected start.
    pub name: String,
    pub error: Option<String>,
    /// Page a plain POST returns to.
    pub back: String,
    pub rec: crate::recording::Rec,
}

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

pub struct SideSeries {
    pub name: String,
    pub count: i64,
}

pub struct ListRow {
    pub id: i64,
    pub day: String,
    /// `15:02–15:20`, `15:02 · running`, or empty (no start time recorded): tells same-day meetings apart.
    pub time: String,
    /// `16 lines` of transcript, or empty.
    pub transcript: String,
    pub decisions: i64,
    pub learned: i64,
    /// `1 open · 1 done`, or `—`.
    pub actions: String,
    /// First line of the notes, cut short, or `—`.
    pub notes: String,
    pub running: bool,
}

pub struct SeriesGroup {
    pub name: String,
    pub rows: Vec<ListRow>,
}

#[derive(Template)]
#[template(path = "meetings/list.html")]
pub struct List {
    pub layout: Layout,
    pub side: Vec<SideSeries>,
    pub banner: Banner,
    pub groups: Vec<SeriesGroup>,
    /// Set when `?name=` narrows the list to one series.
    pub filter: Option<String>,
}

pub struct Line {
    pub kind: Kind,
    pub epic_key: Option<String>,
    pub topic: Option<String>,
    pub text: String,
}

pub struct KindGroup {
    pub title: &'static str,
    pub lines: Vec<Line>,
}

#[derive(Template)]
#[template(path = "meetings/page.html")]
pub struct Page {
    pub layout: Layout,
    pub side: Vec<SideSeries>,
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
    pub notes: String,
    pub today: NaiveDate,
    pub back: String,
    /// Notes linking here with `[[…]]`: (id, title).
    pub mentions: Vec<(i64, String)>,
    /// The series' Tags prop.
    pub field: crate::layout::TagField,
    /// Shown on top while this meeting is recording (D31), so Stop is where you are.
    pub banner: Option<Banner>,
    pub tx: TxSection,
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
    pub who: &'static str,
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
