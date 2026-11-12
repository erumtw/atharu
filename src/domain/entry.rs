use chrono::NaiveDate;

/// What a logged line is. Stored as the lowercase `entries.kind` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Done,
    Decision,
    Blocker,
    Impact,
    Learn,
    Plan,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Done,
        Kind::Decision,
        Kind::Blocker,
        Kind::Impact,
        Kind::Learn,
        Kind::Plan,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Done => "done",
            Kind::Decision => "decision",
            Kind::Blocker => "blocker",
            Kind::Impact => "impact",
            Kind::Learn => "learn",
            Kind::Plan => "plan",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// The line-grammar prefix; `done` has none and shows a dot.
    pub fn mark(self) -> &'static str {
        match self {
            Kind::Done => "·",
            Kind::Decision => "!",
            Kind::Blocker => "x",
            Kind::Impact => "+",
            Kind::Learn => "?",
            Kind::Plan => ">",
        }
    }

    /// Class for the `.k` badge; impact and blocker are the two that stand out on the board.
    pub fn badge_class(self) -> &'static str {
        match self {
            Kind::Impact => "k imp",
            Kind::Blocker => "k bl",
            _ => "k",
        }
    }

    /// Badge tooltip, board wording ("learned", not the stored "learn").
    pub fn tooltip(self) -> &'static str {
        match self {
            Kind::Learn => "learned",
            k => k.as_str(),
        }
    }

    pub fn from_prefix(token: &str) -> Option<Kind> {
        Some(match token {
            "!" => Kind::Decision,
            "x" => Kind::Blocker,
            "+" => Kind::Impact,
            "?" => Kind::Learn,
            ">" => Kind::Plan,
            _ => return None,
        })
    }
}

/// One parsed line, before it gets a day / meeting / id from the handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub epic_key: Option<String>,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    pub due: Option<NaiveDate>,
    pub link: Option<String>,
}

/// `^[A-Z][A-Z0-9]*-\d+$` without a regex dependency.
pub fn is_epic_key(s: &str) -> bool {
    let Some((head, num)) = s.split_once('-') else {
        return false;
    };
    let mut head_chars = head.chars();
    head_chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && head_chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && !num.is_empty()
        && num.chars().all(|c| c.is_ascii_digit())
}

/// `[a-z0-9-]+`, the shape of a learn topic slug.
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Meeting names become series slugs: `OMS Sync` → `oms-sync`, so the same meeting groups across days.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// The note a line links to (D49, `entries.note_id`): every list of lines draws its note button from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRef {
    pub id: i64,
    /// As stored; `""` until the note is named.
    pub title: String,
}

impl NoteRef {
    /// From a `LEFT JOIN notes`: no id = no note.
    pub fn from_cols(id: Option<i64>, title: Option<String>) -> Option<NoteRef> {
        id.map(|id| NoteRef {
            id,
            title: title.unwrap_or_default(),
        })
    }

    pub fn label(&self) -> &str {
        if self.title.trim().is_empty() {
            "Untitled"
        } else {
            &self.title
        }
    }
}

/// One logged line as every list draws it (`partials/entry_row.html`): Today, the tag hub, the epic timeline,
/// the meeting page and Learn map their rows into this, so a row feature lands once (to-dos use `plan::Plan` and
/// `plan_row.html`). Each list fills only what it shows; empty = not drawn.
#[derive(Default)]
pub struct LineRow {
    pub id: i64,
    /// `YYYY-MM-DD` it was logged: the no-JS edit link opens that day.
    pub day: String,
    pub kind: Kind,
    pub topic: Option<String>,
    pub text: String,
    /// `me`, or the agent that filed it (badge).
    pub author: String,
    /// Under a tag: the epic / series the tag comes from when the text lacks it.
    pub via: Option<String>,
    /// Epic chip, where no epic heading already says it.
    pub epic: Option<String>,
    /// Meeting series chip.
    pub meeting: Option<String>,
    pub note: Option<NoteRef>,
    /// Lists spanning days: `today`, `Wed 11` (a tag hub adds why a merged line is there).
    pub when: String,
    /// The epic's tags the text doesn't type itself (epic page): inherited chips.
    pub inh: Vec<String>,
}

impl Entry {
    /// Back to line grammar, so "Undo" after a delete can re-log the same line: `parse(e.to_line()) == e`.
    pub fn to_line(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.extend(self.epic_key.clone());
        if self.kind != Kind::Done {
            parts.push(self.kind.mark().into());
        }
        if let Some(t) = &self.topic {
            parts.push(format!("{t}:"));
        }
        parts.push(self.text.clone());
        if let Some(d) = self.due {
            parts.push(format!("by {}", d.format("%Y-%m-%d")));
        }
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugs() {
        for (name, want) in [
            ("OMS Sync", "oms-sync"),
            ("  1:1 lead ", "1-1-lead"),
            ("oms-sync", "oms-sync"),
            ("Q4 -- planning!", "q4-planning"),
            ("!!!", ""),
        ] {
            assert_eq!(slugify(name), want, "{name}");
        }
    }
}
