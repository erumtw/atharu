//! `#tag` tokens (D29): the text is the source of truth; `taggings` is rebuilt from what this finds.
//! A tag is `#` + `[a-z0-9][a-z0-9-]*` (typed case is folded), and only counts where a person would
//! read it as one: at the start of the text or after whitespace (so URLs' `#anchor`, `C#`, `issue#4`
//! never are), outside `code` spans and ``` fences, not all digits (`#123` is a number).
use std::ops::Range;

/// Byte span of each tag (from the `#`) and its folded name, in text order, duplicates kept.
pub fn spans(text: &str) -> Vec<(Range<usize>, String)> {
    let mut out = Vec::new();
    let mut fenced = false;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let start = at;
        at += line.len();
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut code = false;
        let mut prev: Option<char> = None;
        for (i, c) in line.char_indices() {
            if c == '`' {
                code = !code;
            } else if c == '#'
                && !code
                && prev.is_none_or(char::is_whitespace)
                && let Some((len, tag)) = token(&line[i + 1..])
            {
                out.push((start + i..start + i + 1 + len, tag));
            }
            prev = Some(c);
        }
    }
    out
}

/// What follows a `#`: (bytes used, folded name), or None when it isn't a tag.
fn token(rest: &str) -> Option<(usize, String)> {
    let run = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or(rest.len());
    // `#café` / `#snake_case`: the word goes on past what a tag can hold, so it's not one.
    if rest[run..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let name = rest[..run].trim_end_matches('-').to_ascii_lowercase();
    let ok = name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && !name.bytes().all(|b| b.is_ascii_digit());
    ok.then_some((name.len(), name))
}

/// Each tag once, in order of first appearance.
pub fn tags(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (_, t) in spans(text) {
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// One run of a line's text: plain, or a tag (rendered as a chip linking to its hub).
#[derive(Debug, PartialEq, Eq)]
pub struct Seg {
    pub text: String,
    pub tag: Option<String>,
}

pub fn segments(text: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut at = 0;
    for (r, tag) in spans(text) {
        if r.start > at {
            out.push(Seg {
                text: text[at..r.start].into(),
                tag: None,
            });
        }
        out.push(Seg {
            text: text[r.clone()].into(),
            tag: Some(tag),
        });
        at = r.end;
    }
    if at < text.len() || out.is_empty() {
        out.push(Seg {
            text: text[at..].into(),
            tag: None,
        });
    }
    out
}

/// A tags field (`expense, #Hiring`) → valid folded names, once each; anything else is dropped.
pub fn parse_list(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in s.split(|c: char| c == ',' || c.is_whitespace()) {
        let w = w.strip_prefix('#').unwrap_or(w);
        if let Some((len, t)) = token(w)
            && len == w.trim_end_matches('-').len()
            && !out.contains(&t)
        {
            out.push(t);
        }
    }
    out
}

/// A typed word names the tag, case-insensitive and plural-tolerant (expense ~ Expenses, policy ~ policies).
pub fn same_word(word: &str, tag: &str) -> bool {
    let w = word.to_lowercase();
    // Generate plurals rather than strip them: stripping can't tell expens|es from box|es.
    let plural_of = |p: &str, s: &str| {
        p.strip_prefix(s).is_some_and(|x| x == "s" || x == "es")
            || s.strip_suffix('y')
                .is_some_and(|b| p.strip_prefix(b) == Some("ies"))
    };
    w == tag || plural_of(&w, tag) || plural_of(tag, &w)
}

/// "Tag as #x?": the first plain word (not already a tag, not in `skip`) that names a known tag.
/// Returns (the word as typed, the tag).
pub fn offer<'a>(text: &'a str, known: &[String], skip: &[String]) -> Option<(&'a str, String)> {
    let tagged = tags(text);
    text.split_whitespace()
        .filter(|w| !w.starts_with('#') && !w.contains("://") && !w.contains('`'))
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 2 && !skip.iter().any(|s| s.eq_ignore_ascii_case(w)))
        .find_map(|w| {
            known
                .iter()
                .find(|t| !tagged.contains(t) && same_word(w, t))
                .map(|t| (w, t.clone()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tags() {
        for (text, want) in [
            ("booked #expense e2e review", vec!["expense"]),
            ("#Expense first, then #hiring.", vec!["expense", "hiring"]),
            ("#expense and #expense again", vec!["expense"]),
            ("p95 0.4s on canary #cms-profile", vec!["cms-profile"]),
            ("trailing #oncall-", vec!["oncall"]),
            ("#q4-2026 plan", vec!["q4-2026"]),
            // Not tags: alone, numbers, mid-word, URLs, code, non-ASCII / underscore words.
            ("# alone", vec![]),
            ("issue #123 and #42", vec![]),
            ("C# and issue#4", vec![]),
            ("see https://x.io/a#frag and http://b.io/#x", vec![]),
            ("run `git log #nope` then #yes", vec!["yes"]),
            ("#café and #snake_case", vec![]),
            ("#-dash", vec![]),
            ("(#paren)", vec![]),
        ] {
            assert_eq!(tags(text), want, "{text}");
        }
    }

    #[test]
    fn note_bodies() {
        let body = "# Heading\n## Sub #real\n```\n#in-fence\n```\n#after line\n\\#escaped";
        assert_eq!(tags(body), ["real", "after"]);
    }

    #[test]
    fn segments_round_trip() {
        let t = "booked #Expense e2e #hiring";
        let s = segments(t);
        assert_eq!(s.iter().map(|x| x.text.as_str()).collect::<String>(), t);
        assert_eq!(
            s[1],
            Seg {
                text: "#Expense".into(),
                tag: Some("expense".into())
            }
        );
        assert_eq!(s.len(), 4);
        assert_eq!(
            segments(""),
            [Seg {
                text: "".into(),
                tag: None
            }]
        );
    }

    #[test]
    fn lists() {
        assert_eq!(
            parse_list("expense, #Hiring  oncall,,expense"),
            ["expense", "hiring", "oncall"]
        );
        assert!(parse_list("123, a_b, -x, café").is_empty());
    }

    #[test]
    fn plural_words() {
        for (w, t) in [
            ("expense", "expense"),
            ("Expenses", "expense"),
            ("expense", "expenses"),
            ("policies", "policy"),
            ("boxes", "box"),
            ("taxes", "tax"),
            ("EXPORTS", "export"),
        ] {
            assert!(same_word(w, t), "{w} ~ {t}");
        }
        for (w, t) in [
            ("expensive", "expense"),
            ("ex", "expense"),
            ("hire", "hiring"),
        ] {
            assert!(!same_word(w, t), "{w} !~ {t}");
        }
    }

    #[test]
    fn offers() {
        let known = vec!["expense".to_string(), "hiring".to_string()];
        assert_eq!(
            offer("booked Expenses review", &known, &[]),
            Some(("Expenses", "expense".into()))
        );
        assert_eq!(offer("booked #expense review expenses", &known, &[]), None);
        assert_eq!(offer("booked expense", &known, &["expense".into()]), None);
        assert_eq!(offer("see https://x.io/expense", &known, &[]), None);
    }
}
