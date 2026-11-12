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
        if n >= 2 && toks[n - 2].eq_ignore_ascii_case("by") {
            let when = toks[n - 1];
            due = Some(parse_when(when, today).ok_or_else(|| ParseError::BadDate(when.into()))?);
            text = toks[..n - 2].join(" ");
        } else if n >= 3 && toks[n - 3].eq_ignore_ascii_case("by") {
            // Two-token form (`20 Nov`) only when it reads as a date: "by email today" is plain text.
            let when = toks[n - 2..].join(" ");
            if let Some(d) = parse_when(&when, today) {
                due = Some(d);
                text = toks[..n - 3].join(" ");
            }
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
            ("> ship by someday", "Can't read date ‘someday’"),
        ] {
            assert_eq!(
                parse(line, d(TODAY)).unwrap_err().to_string(),
                msg,
                "{line:?}"
            );
        }
    }
}
