//! Note markdown → HTML for read-only views. Raw HTML is shown as text, never passed through.
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};

/// The editor's Pandoc-style blocks (`:::callout` … `:::`) and the tags they become.
fn fence(line: &str) -> Option<&'static str> {
    Some(match line {
        ":::details" => "<details>",
        ":::detailsSummary" => "<summary>",
        ":::detailsContent" => "<div>",
        ":::callout" => "<div class=\"callout\">",
        _ => return None,
    })
}

fn close(open: &str) -> &'static str {
    match open {
        "<details>" => "</details>",
        "<summary>" => "</summary>",
        _ => "</div>",
    }
}

/// Only web and mail links stay clickable; `javascript:` and friends become `#`.
fn safe_url(url: CowStr<'_>) -> CowStr<'_> {
    let lower = url.to_ascii_lowercase();
    let has_scheme = lower
        .split_once(':')
        .is_some_and(|(s, _)| !s.contains(['/', '?', '#']));
    if !has_scheme
        || ["http:", "https:", "mailto:"]
            .iter()
            .any(|p| lower.starts_with(p))
    {
        url
    } else {
        CowStr::Borrowed("#")
    }
}

fn render(md: &str, out: &mut String) {
    let opts = Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let events = Parser::new_ext(md, opts).map(|e| match e {
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: safe_url(dest_url),
            title,
            id,
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type,
            dest_url: safe_url(dest_url),
            title,
            id,
        }),
        e => e,
    });
    html::push_html(out, events);
}

pub fn to_html(md: &str) -> String {
    let mut out = String::new();
    let mut chunk = String::new();
    let mut open: Vec<&'static str> = Vec::new();
    for line in md.lines() {
        let t = line.trim();
        // `:::details {open="true"}` → only the block name matters here.
        let tag = t.split_whitespace().next().and_then(fence);
        if tag.is_some() || (t == ":::" && !open.is_empty()) {
            // Fences are block-level, so each stretch between them renders on its own.
            render(&std::mem::take(&mut chunk), &mut out);
            match tag {
                Some(tag) => {
                    out.push_str(tag);
                    open.push(tag);
                }
                None => out.push_str(open.pop().map_or("", close)),
            }
            continue;
        }
        chunk.push_str(line);
        chunk.push('\n');
    }
    render(&chunk, &mut out);
    for tag in open.into_iter().rev() {
        out.push_str(close(tag));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::to_html;

    #[test]
    fn raw_html_is_text_and_bad_links_are_dead() {
        let h = to_html(
            "<script>alert(1)</script>\n\nhi <b>x</b> [a](javascript:alert(1)) [b](/epics/OPS-1) [c](https://x.io)",
        );
        assert!(!h.contains("<script>") && !h.contains("<b>"), "{h}");
        assert!(h.contains("&lt;script&gt;"));
        assert!(h.contains("<a href=\"#\">a</a>"));
        assert!(h.contains("<a href=\"/epics/OPS-1\">b</a>"));
        assert!(h.contains("<a href=\"https://x.io\">c</a>"));
    }

    #[test]
    fn editor_blocks() {
        let md = ":::callout\n\n**Decision.** keep sync\n\n:::\n\n:::details\n\n:::detailsSummary\n\nRaw numbers\n\n:::\n\n:::detailsContent\n\n- [x] before 1.8s\n\n:::\n\n:::";
        let h = to_html(md);
        assert!(
            h.starts_with(
                "<div class=\"callout\"><p><strong>Decision.</strong> keep sync</p>\n</div>"
            ),
            "{h}"
        );
        assert!(
            h.contains("<details><summary><p>Raw numbers</p>\n</summary><div><ul>"),
            "{h}"
        );
        assert!(h.ends_with("</div></details>"), "{h}");
        assert!(h.contains("checked"));
    }
}
