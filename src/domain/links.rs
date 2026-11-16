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

/// `note:7` → `7`: a link to another note by its number (an epic key or tag can't hold `:`, so it never collides).
pub fn note_id(x: &str) -> Option<&str> {
    x.strip_prefix("note:")
        .filter(|n| !n.is_empty() && n.len() <= 18 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Where `[[X]]` points: `note:N` → that note (only one that exists), else epic key → epic (only one that exists: `[[OPS-999]]` with no such epic is dead), else a tag in use → its hub (D45: one name, one hub, which also
/// shows the topic + series of that name), else an existing topic, else an existing meeting series.
/// `None` = renders as plain text (a dead link the page can point out).
pub fn href(
    x: &str,
    is_epic: impl Fn(&str) -> bool,
    is_topic: impl Fn(&str) -> bool,
    is_meeting: impl Fn(&str) -> bool,
    is_tag: impl Fn(&str) -> bool,
    is_note: impl Fn(&str) -> bool,
) -> Option<String> {
    if let Some(n) = note_id(x) {
        is_note(n).then(|| format!("/notes/{n}"))
    } else if is_epic_key(x) {
        is_epic(x).then(|| format!("/epics/{x}"))
    } else if is_tag(x) {
        Some(format!("/tags/{x}"))
    } else if is_topic(x) {
        Some(format!("/learn/{x}"))
    } else if is_meeting(x) {
        Some(format!("/meetings?name={x}"))
    } else {
        None
    }
}

/// `[[X]]` → `[label](href)` or plain `X`, so the markdown renderer only sees standard links. `resolve` gives (href, label), the
/// label already markdown-safe (a note link shows the note's title, not `note:7`).
pub fn rewrite(body: &str, resolve: impl Fn(&str) -> Option<(String, String)>) -> String {
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for (start, x, end) in targets(body) {
        out.push_str(&body[last..start]);
        match resolve(x) {
            Some((h, label)) => out.push_str(&format!("[{label}]({h})")),
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
        // `oms-sync` is a topic and a meeting (topic wins); `infra` is a topic, series and tag (the hub wins);
        // `OPS-101` is also a tag, but an epic key always wins; `OPS-999` has no epic, so it is dead.
        href(
            x,
            |s| s == "OPS-101",
            |s| ["vector-search", "oms-sync", "infra"].contains(&s),
            |s| ["oms-sync", "standup", "infra"].contains(&s),
            |s| ["expense", "infra", "OPS-101"].contains(&s),
            |n| ["7", "12"].contains(&n),
        )
    }

    #[test]
    fn resolution_order() {
        for (x, want) in [
            ("OPS-101", Some("/epics/OPS-101")),
            ("vector-search", Some("/learn/vector-search")),
            ("oms-sync", Some("/learn/oms-sync")),
            ("standup", Some("/meetings?name=standup")),
            ("expense", Some("/tags/expense")),
            ("infra", Some("/tags/infra")),
            ("chekout", None),
            ("ops-101", None),
            ("OPS-999", None),
            ("note:7", Some("/notes/7")),
            ("note:99", None),
            ("note:", None),
            ("note:7x", None),
            ("Note:7", None),
        ] {
            assert_eq!(resolve(x).as_deref(), want, "{x}");
        }
    }

    #[test]
    fn rewrite_and_mentions() {
        let body = "p95 on [[OPS-101]], see [[standup]] and [[chekout]]; [[OPS-101]] again. [[]] [[a\nb]] [x]";
        let resolve = |x: &str| resolve(x).map(|h| (h, x.to_string()));
        assert_eq!(
            rewrite(body, resolve),
            "p95 on [OPS-101](/epics/OPS-101), see [standup](/meetings?name=standup) and chekout; \
             [OPS-101](/epics/OPS-101) again. [[]] [[a\nb]] [x]"
        );
        assert_eq!(mentions(body), ["OPS-101", "standup", "chekout"]);
        // A note link shows the label the resolver gives (the note's title).
        let titled = |x: &str| {
            resolve(x).map(|(h, l)| {
                (
                    h,
                    if note_id(x).is_some() {
                        "Retro".into()
                    } else {
                        l
                    },
                )
            })
        };
        assert_eq!(
            rewrite("see [[note:7]] and [[note:99]]", titled),
            "see [Retro](/notes/7) and note:99"
        );
    }
}
