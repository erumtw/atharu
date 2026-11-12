//! Note markdown → HTML for read-only views. Raw HTML is shown as text, never passed through.
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};

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

/// The editor's drawing block, one line: `![Drawing](data:image/svg+xml;base64,…)`, or `![Drawing]()` before anything is
/// drawn. `Some(src)` only for that exact form: base64 has no `"`, `<` or `:`, so the src can't break out of the attribute,
/// and an SVG shown through `<img>` runs no script and fetches nothing (D24). Anything else stays an ordinary image.
fn drawing(line: &str) -> Option<&str> {
    let src = line.strip_prefix("![Drawing](")?.strip_suffix(')')?;
    let b64 = src
        .strip_prefix("data:image/svg+xml;base64,")
        .unwrap_or(src);
    let ok = (src.is_empty() || b64.len() < src.len())
        && b64
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='));
    ok.then_some(src)
}

pub fn to_html(md: &str) -> String {
    let mut out = String::new();
    let mut chunk = String::new();
    let mut open: Vec<&'static str> = Vec::new();
    for line in md.lines() {
        let t = line.trim();
        if let Some(src) = drawing(t) {
            render(&std::mem::take(&mut chunk), &mut out);
            if src.is_empty() {
                out.push_str("<figure class=\"drawing empty\"><span class=\"small muted\">Empty drawing</span></figure>");
            } else {
                out.push_str("<figure class=\"drawing\"><img src=\"");
                out.push_str(src);
                out.push_str("\" alt=\"Drawing\"></figure>");
            }
            continue;
        }
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

/// The words of a note without its markup (list snippets). `[[X]]` stays literal: it is how a note names a page.
pub fn to_text(md: &str) -> String {
    let body: String = md
        .lines()
        .filter(|l| {
            let t = l.trim();
            t != ":::" && t.split_whitespace().next().and_then(fence).is_none()
        })
        .flat_map(|l| [l, "\n"])
        .collect();
    let mut out = String::new();
    for e in Parser::new_ext(
        &body,
        Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH,
    ) {
        match e {
            Event::Text(t) | Event::Code(t) | Event::Html(t) | Event::InlineHtml(t) => {
                out.push_str(&t)
            }
            // Inline ends glue on (`**p95**s` stays "p95s"); block ends separate words.
            Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image,
            ) => {}
            Event::SoftBreak | Event::HardBreak | Event::End(_) | Event::Rule => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Why a quick note (D53) can't be saved: the hint error, from the notes slice (htmx) or the popover (plain POST).
pub const NO_TITLE: &str = "Nothing to save. Type the note’s title first.";

/// One text box → a note (the quick popover, D53): the first non-blank line is the title, the rest the body.
/// `None` = only whitespace.
pub fn split_note(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    let (title, body) = text.split_once('\n').unwrap_or((text, ""));
    let title = title.trim();
    (!title.is_empty()).then(|| (title, body.trim_start_matches(['\r', '\n']).trim_end()))
}

#[cfg(test)]
mod tests {
    use super::{to_html, to_text};

    #[test]
    fn text_drops_markup_keeps_page_links() {
        let md = "## Retro\n\n:::callout\n\n**p9**5 hit `1.8s` on [[OPS-101]], see [doc](https://x.io)\n\n:::\n\n- [x] done\n---\n";
        assert_eq!(
            to_text(md),
            "Retro p95 hit 1.8s on [[OPS-101]], see doc done"
        );
    }

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

    #[test]
    fn drawing_block_is_the_only_data_image() {
        let h = to_html(
            "before\n\n![Drawing](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)\n\n![Drawing]()\nafter",
        );
        assert_eq!(
            h,
            "<p>before</p>\n<figure class=\"drawing\"><img src=\"data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=\" alt=\"Drawing\"></figure><figure class=\"drawing empty\"><span class=\"small muted\">Empty drawing</span></figure><p>after</p>\n"
        );
        for bad in [
            "![Drawing](javascript:alert(1))",
            "![Drawing](data:text/html;base64,PHNjcmlwdD4=)",
            "![Drawing](data:image/svg+xml,<svg onload=alert(1)>)",
            "![Drawing](data:image/svg+xml;base64,PH\"onerror=alert(1))",
            "![x](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)",
            "a ![Drawing](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)",
        ] {
            let h = to_html(bad);
            assert!(
                !h.contains("=\"data:") && !h.contains("=\"javascript:") && !h.contains("figure"),
                "{bad} → {h}"
            );
        }
    }

    #[test]
    fn split_note_takes_the_first_line_as_title() {
        assert_eq!(
            super::split_note("Vendor call\r\nwants 4 h\n#vendor\n"),
            Some(("Vendor call", "wants 4 h\n#vendor"))
        );
        assert_eq!(
            super::split_note("\n  only a title  "),
            Some(("only a title", ""))
        );
        assert_eq!(super::split_note(" \n \t"), None);
    }
}
