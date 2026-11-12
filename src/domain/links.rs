//! `[[X]]` page links in note bodies: find them and resolve them (spec Phase 4 order).
use super::entry::is_epic_key;

/// Copy when an evidence link isn't http(s); the web form and the CLI say the same thing.
pub const NOT_HTTP: &str = "Paste a link that starts with http:// or https://.";

/// Only http(s): the url becomes an href, so `javascript:` must never get stored.
pub fn is_http(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Each `[[X]]` target once, in order of first appearance.
pub fn mentions(body: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for x in targets(body).map(|(_, x, _)| x) {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

/// `(start, X, end)` byte spans of every well-formed `[[X]]` (one line, no brackets inside).
fn targets(body: &str) -> impl Iterator<Item = (usize, &str, usize)> {
    let mut at = 0;
    std::iter::from_fn(move || {
        while let Some(i) = body[at..].find("[[").map(|i| at + i) {
            let rest = &body[i + 2..];
            let close = rest.find("]]");
            at = i + 2;
            if let Some(c) = close {
                let x = &rest[..c];
                if !x.trim().is_empty() && !x.contains(['[', ']', '\n']) {
                    at = i + 2 + c + 2;
                    return Some((i, x.trim(), at));
                }
            }
        }
        None
    })
}

/// Where `[[X]]` points: epic key → epic, else an existing topic, else an existing meeting series.
/// `None` = renders as plain text (and is a dead link the page can point out).
pub fn href(
    x: &str,
    is_topic: impl Fn(&str) -> bool,
    is_meeting: impl Fn(&str) -> bool,
) -> Option<String> {
    if is_epic_key(x) {
        Some(format!("/epics/{x}"))
    } else if is_topic(x) {
        Some(format!("/learn/{x}"))
    } else if is_meeting(x) {
        Some(format!("/meetings?name={x}"))
    } else {
        None
    }
}

/// `[[X]]` → `[X](href)` or plain `X`, so the markdown renderer only sees standard links.
pub fn rewrite(body: &str, resolve: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for (start, x, end) in targets(body) {
        out.push_str(&body[last..start]);
        match resolve(x) {
            Some(h) => out.push_str(&format!("[{x}]({h})")),
            None => out.push_str(x),
        }
        last = end;
    }
    out.push_str(&body[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(x: &str) -> Option<String> {
        // `oms-sync` is both a topic and a meeting: the topic wins.
        href(
            x,
            |s| ["vector-search", "oms-sync"].contains(&s),
            |s| ["oms-sync", "standup"].contains(&s),
        )
    }

    #[test]
    fn resolution_order() {
        for (x, want) in [
            ("OPS-101", Some("/epics/OPS-101")),
            ("vector-search", Some("/learn/vector-search")),
            ("oms-sync", Some("/learn/oms-sync")),
            ("standup", Some("/meetings?name=standup")),
            ("chekout", None),
            ("ops-101", None),
        ] {
            assert_eq!(resolve(x).as_deref(), want, "{x}");
        }
    }

    #[test]
    fn rewrite_and_mentions() {
        let body = "p95 on [[OPS-101]], see [[standup]] and [[chekout]]; [[OPS-101]] again. [[]] [[a\nb]] [x]";
        assert_eq!(
            rewrite(body, resolve),
            "p95 on [OPS-101](/epics/OPS-101), see [standup](/meetings?name=standup) and chekout; \
             [OPS-101](/epics/OPS-101) again. [[]] [[a\nb]] [x]"
        );
        assert_eq!(mentions(body), ["OPS-101", "standup", "chekout"]);
    }
}
