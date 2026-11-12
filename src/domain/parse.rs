//! Line grammar: `[EPIC-KEY] [prefix] text [link] [by <when>]` (spec "Line grammar").
use super::dates::parse_when;
use super::entry::{Entry, Kind, is_epic_key, is_slug};
use chrono::NaiveDate;
use std::fmt;

/// Why a line was not saved. `Display` is the exact copy shown under the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    /// Holds the last token the user typed (`+`, `OPS-101`, `vector-search:`).
    NothingAfter(String),
    BadDate(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Not on the States board; same voice as its "Nothing after" tile.
            ParseError::Empty => write!(f, "Type what you did first."),
            ParseError::NothingAfter(t) => write!(f, "Nothing after “{t}”. Type what happened."),
            ParseError::BadDate(w) => write!(f, "Can't read date ‘{w}’"),
        }
    }
}

impl std::error::Error for ParseError {}

/// First whitespace token and the rest (left-trimmed); keeps inner spacing of the text intact.
fn split_first(s: &str) -> (&str, &str) {
    match s.split_once(char::is_whitespace) {
        Some((head, tail)) => (head, tail.trim_start()),
        None => (s, ""),
    }
}

/// `2026-13-01` or `32 Nov`: meant as a date, so a wrong one is an error rather than text.
fn date_shaped(when: &[&str]) -> bool {
    const MONTHS: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    match when {
        [w] => {
            let parts: Vec<&str> = w.split('-').collect();
            parts.len() == 3
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        }
        [d, m] => {
            let m = m.to_ascii_lowercase();
            d.len() <= 2
                && d.bytes().all(|b| b.is_ascii_digit())
                && m.len() >= 3
                && MONTHS.iter().any(|x| x.starts_with(&m))
        }
        _ => false,
    }
}

/// A to-do whose trailing `by …` was kept as text: those words (`by hand`), for the preview's soft hint.
pub fn by_as_text(e: &Entry) -> Option<String> {
    if e.kind != Kind::Plan || e.due.is_some() {
        return None;
    }
    let toks: Vec<&str> = e.text.split_whitespace().collect();
    let n = toks.len();
    [1, 2]
        .into_iter()
        .find(|&w| n > w && toks[n - w - 1].eq_ignore_ascii_case("by"))
        .map(|w| toks[n - w - 1..].join(" "))
}

pub fn parse(line: &str, today: NaiveDate) -> Result<Entry, ParseError> {
    let mut rest = line.trim();
    if rest.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut last = "";

    let (first, tail) = split_first(rest);
    let epic_key = is_epic_key(first).then(|| {
        last = first;
        rest = tail;
        first.to_string()
    });

    // Token-based, so `xml parser fixed` is not a blocker: `x` must be the whole token.
    let (tok, tail) = split_first(rest);
    let kind = match Kind::from_prefix(tok) {
        Some(k) => {
            last = tok;
            rest = tail;
            k
        }
        None => Kind::Done,
    };

    let mut topic = None;
    if kind == Kind::Learn {
        let (tok, tail) = split_first(rest);
        if let Some(slug) = tok.strip_suffix(':').map(str::to_ascii_lowercase)
            && is_slug(&slug)
        {
            last = tok;
            rest = tail;
            topic = Some(slug);
        }
    }

    let mut due = None;
    let mut text = rest.to_string();
    if kind == Kind::Plan {
        let toks: Vec<&str> = rest.split_whitespace().collect();
        let n = toks.len();
        // `by <when>` is one token, or two for `20 Nov`. Words that aren't a date stay text ("refactor by hand",
        // D44); only a date-shaped one that doesn't exist (`32 Nov`, `2026-13-01`) is an error.
        for w in [1, 2] {
            if n < w + 1 || !toks[n - w - 1].eq_ignore_ascii_case("by") {
                continue;
            }
            let when = toks[n - w..].join(" ");
            match parse_when(&when, today) {
                Some(d) => {
                    due = Some(d);
                    text = toks[..n - w - 1].join(" ");
                }
                None if date_shaped(&toks[n - w..]) => return Err(ParseError::BadDate(when)),
                None => continue,
            }
            break;
        }
    }

    if text.is_empty() {
        return Err(ParseError::NothingAfter(last.to_string()));
    }
    let link = text
        .split_whitespace()
        .find(|t| t.starts_with("http://") || t.starts_with("https://"))
        .map(String::from);

    Ok(Entry {
        epic_key,
        kind,
        topic,
        text,
        due,
        link,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    const TODAY: &str = "2026-11-11"; // Wednesday

    /// (line, key, kind, topic, text, due, link)
    type Want = (
        &'static str,
        Option<&'static str>,
        Kind,
        Option<&'static str>,
        &'static str,
        Option<&'static str>,
        Option<&'static str>,
    );

    #[test]
    fn parses_lines() {
        use Kind::*;
        #[rustfmt::skip]
        let cases: &[Want] = &[
            ("shipped index migration", None, Done, None, "shipped index migration", None, None),
            ("OPS-101 shipped it", Some("OPS-101"), Done, None, "shipped it", None, None),
            ("OPS-101 ! keep sync path", Some("OPS-101"), Decision, None, "keep sync path", None, None),
            ("OPS-101 x waiting on DB", Some("OPS-101"), Blocker, None, "waiting on DB", None, None),
            ("OPS-101 + p95 1.8s → 0.6s", Some("OPS-101"), Impact, None, "p95 1.8s → 0.6s", None, None),
            ("> write runbook", None, Plan, None, "write runbook", None, None),
            ("! no key decision", None, Decision, None, "no key decision", None, None),
            ("? vector-search: HNSW trades recall", None, Learn, Some("vector-search"), "HNSW trades recall", None, None),
            ("? Vector-Search: caps", None, Learn, Some("vector-search"), "caps", None, None),
            ("? how HNSW works", None, Learn, None, "how HNSW works", None, None),
            ("xml parser fixed", None, Done, None, "xml parser fixed", None, None),
            ("ops-101 lowercase is text", None, Done, None, "ops-101 lowercase is text", None, None),
            ("fixed OPS-101 later", None, Done, None, "fixed OPS-101 later", None, None),
            ("see https://x.slack.com/a and http://b.io", None, Done, None, "see https://x.slack.com/a and http://b.io", None, Some("https://x.slack.com/a")),
            ("https://github.com/o/r/pull/1 merged", None, Done, None, "https://github.com/o/r/pull/1 merged", None, Some("https://github.com/o/r/pull/1")),
            ("> reply https://x.slack.com/a by fri", None, Plan, None, "reply https://x.slack.com/a", Some("2026-11-13"), Some("https://x.slack.com/a")),
            ("OPS-1 > ship by 20 Nov", Some("OPS-1"), Plan, None, "ship", Some("2026-11-20"), None),
            ("> ship by 2026-12-01", None, Plan, None, "ship", Some("2026-12-01"), None),
            ("> ship by today", None, Plan, None, "ship", Some(TODAY), None),
            ("> ask by email today", None, Plan, None, "ask by email today", None, None),
            ("done by fri stays text", None, Done, None, "done by fri stays text", None, None),
            ("> refactor by hand", None, Plan, None, "refactor by hand", None, None),
            ("> ship by someday", None, Plan, None, "ship by someday", None, None),
            ("> paint by 2", None, Plan, None, "paint by 2", None, None),
            ("> buy 2 by 5 apples", None, Plan, None, "buy 2 by 5 apples", None, None),
        ];
        for &(line, key, kind, topic, text, due, link) in cases {
            let want = Entry {
                epic_key: key.map(String::from),
                kind,
                topic: topic.map(String::from),
                text: text.into(),
                due: due.map(d),
                link: link.map(String::from),
            };
            assert_eq!(parse(line, d(TODAY)), Ok(want.clone()), "{line}");
            assert_eq!(
                parse(&want.to_line(), d(TODAY)),
                Ok(want),
                "round trip {line}"
            );
        }
    }

    #[test]
    fn by_kept_as_text() {
        let t = d(TODAY);
        let soft = |l: &str| by_as_text(&parse(l, t).unwrap());
        assert_eq!(soft("> refactor by hand"), Some("by hand".into()));
        assert_eq!(soft("> ask by email today"), Some("by email today".into()));
        assert_eq!(soft("> ship by fri"), None);
        assert_eq!(soft("> ship it"), None);
        assert_eq!(soft("done by hand"), None, "not a to-do");
    }

    #[test]
    fn rejects_lines() {
        for (line, msg) in [
            ("", "Type what you did first."),
            ("   ", "Type what you did first."),
            ("OPS-101 +", "Nothing after “+”. Type what happened."),
            ("OPS-101", "Nothing after “OPS-101”. Type what happened."),
            ("?", "Nothing after “?”. Type what happened."),
            (
                "? vector-search:",
                "Nothing after “vector-search:”. Type what happened.",
            ),
            ("> by fri", "Nothing after “>”. Type what happened."),
            ("> ship by 32 Nov", "Can't read date ‘32 Nov’"),
            ("> ship by 2026-13-01", "Can't read date ‘2026-13-01’"),
        ] {
            assert_eq!(
                parse(line, d(TODAY)).unwrap_err().to_string(),
                msg,
                "{line:?}"
            );
        }
    }
}
