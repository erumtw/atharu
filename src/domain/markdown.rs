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
/// drawn. A caption rides in the alt, `![Drawing · caption](…)`, only on a drawn one: the drawing stays editable (the SVG
/// carries its scene) and still says what it shows. `Some((caption, src))` only for those exact forms: base64 has no `"`, `<`
/// or `:`, so the src can't break out of the attribute, and an SVG shown through `<img>` runs no script and fetches nothing
/// (D24). The caption has no `]` or newline and is escaped by the caller. Anything else stays an ordinary image.
fn drawing(line: &str) -> Option<(Option<&str>, &str)> {
    let (alt, src) = line.strip_prefix("![")?.split_once("](")?;
    let src = src.strip_suffix(')')?;
    let caption = match alt.strip_prefix("Drawing") {
        Some("") => None,
        Some(rest) => Some(rest.strip_prefix(" · ").filter(|c| !c.trim().is_empty())?),
        None => return None,
    };
    let b64 = src
        .strip_prefix("data:image/svg+xml;base64,")
        .unwrap_or(src);
    let ok = (src.is_empty() || b64.len() < src.len())
        && (caption.is_none() || !src.is_empty())
        && !alt.contains([']', '\n'])
        && b64
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='));
    ok.then_some((caption, src))
}

/// Tracks a fenced code block (``` or ~~~) line by line. Inside one, an image or drawing line is code the author is showing, not a
/// block to render, and a `:::` line is just text.
#[derive(Default)]
struct CodeFence(Option<(char, usize)>);

impl CodeFence {
    /// True when `line` belongs to a fenced block (its fences included): nothing special applies to it.
    fn takes(&mut self, line: &str) -> bool {
        let t = line.trim_start_matches(' ');
        let run = |c: char| t.chars().take_while(|&x| x == c).count();
        let marker = if line.len() - t.len() <= 3 {
            ['`', '~'].into_iter().find(|&c| run(c) >= 3)
        } else {
            None
        };
        match (self.0, marker) {
            // The closing fence: the same character, at least as long, nothing after it.
            (Some((c, n)), Some(m)) if m == c && run(m) >= n && t[run(m)..].trim().is_empty() => {
                self.0 = None;
                true
            }
            (Some(_), _) => true,
            // An opening fence (a backtick fence's info string can't hold a backtick: that is inline code).
            (None, Some(m)) if m != '`' || !t[run(m)..].contains('`') => {
                self.0 = Some((m, run(m)));
                true
            }
            _ => false,
        }
    }
}

/// An image on its own line, `![alt](src)`: `Some((alt, src))` only for the two forms we can serve safely. `src` is a base64
/// `data:image/{png,jpeg,gif,webp,svg+xml}` URI (agents write these; an SVG shown through `<img>` runs no script, D24) or a
/// pasted file under `/attachments/notes/<id>/<name>`. Both charsets rule out `"`, `<`, `)` and `:` in the src, so it can't
/// break out of the attribute; alt is escaped by the caller. Anything else stays an ordinary (dead) image.
fn image(line: &str) -> Option<(&str, &str)> {
    let (alt, src) = line.strip_prefix("![")?.split_once("](")?;
    let src = src.strip_suffix(')')?;
    let ok = match src.strip_prefix("data:image/") {
        Some(rest) => {
            let (kind, b64) = rest.split_once(";base64,")?;
            matches!(kind, "png" | "jpeg" | "gif" | "webp" | "svg+xml")
                && b64
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        }
        None => src
            .strip_prefix("/attachments/notes/")
            .and_then(|r| r.split_once('/'))
            .is_some_and(|(id, name)| {
                !id.is_empty()
                    && id.bytes().all(|b| b.is_ascii_digit())
                    && !name.starts_with('.')
                    && !name.contains("..")
                    && !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            }),
    };
    (ok && !alt.contains([']', '\n'])).then_some((alt, src))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Why an agent's note body is refused (0.2.10, owner: diagrams are always Excalidraw boards): the first place a diagram is drawn some
/// other way, as `(line number, what it is)`. A ```mermaid / ~~~mermaid opener counts anywhere; an SVG image counts unless it is an
/// editable board line (`Drawing` / `Drawing · caption`), whole-line or inline. Code the note is only showing (an SVG line inside a
/// fence) and PNG / JPEG / GIF / WebP images pass. The web editor is not checked: it is the owner's own hand.
pub fn diagram_problem(body: &str) -> Option<(usize, &'static str)> {
    let mut code = CodeFence::default();
    for (i, line) in body.lines().enumerate() {
        let opener = line.trim_start().trim_start_matches(['`', '~']);
        let fenced = line.trim_start().len() - opener.len() >= 3;
        if fenced
            && opener
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("mermaid")
        {
            return Some((i + 1, "a mermaid block"));
        }
        if code.takes(line) {
            continue;
        }
        if drawing(line.trim()).is_none() && line.contains("](data:image/svg+xml") {
            return Some((i + 1, "an SVG image that is not an Excalidraw board"));
        }
    }
    None
}

/// An image or drawing line of a note body, as `atharu note show` lists it.
pub struct ImageInfo {
    pub alt: String,
    /// `drawing` or `image`.
    pub kind: &'static str,
    /// The src of a served file (`/attachments/notes/…`), empty for a `data:` image.
    pub file: String,
    /// Decoded size in KB (rounded up) of a `data:` image; `None` for a served file (the caller stats it).
    pub kb: Option<usize>,
}

/// The body for an agent to read: each `data:` image / drawing line keeps its alt and loses its base64 (`![alt](data:… 54 KB)`),
/// since one diagram is 50 KB of characters that mean nothing as text. The images are listed alongside, in order.
pub fn summarize_images(md: &str) -> (String, Vec<ImageInfo>) {
    let mut out = Vec::new();
    let mut images = Vec::new();
    let mut code = CodeFence::default();
    for line in md.lines() {
        let t = line.trim();
        if code.takes(line) {
            out.push(line.to_string());
            continue;
        }
        let (alt, src, kind) = match (drawing(t), image(t)) {
            (Some((caption, src)), _) if !src.is_empty() => (
                if caption.is_some() {
                    &t[2..t.find("](").unwrap_or(2)]
                } else {
                    "Drawing"
                },
                src,
                "drawing",
            ),
            (None, Some((alt, src))) => (alt, src, "image"),
            _ => {
                out.push(line.to_string());
                continue;
            }
        };
        if let Some((_, b64)) = src.split_once(";base64,") {
            let kb = (b64.len() * 3 / 4).div_ceil(1024).max(1);
            out.push(format!("![{alt}](data:… {kb} KB)"));
            images.push(ImageInfo {
                alt: alt.into(),
                kind,
                file: String::new(),
                kb: Some(kb),
            });
        } else {
            out.push(line.to_string());
            images.push(ImageInfo {
                alt: alt.into(),
                kind,
                file: src.into(),
                kb: None,
            });
        }
    }
    (out.join("\n"), images)
}

pub fn to_html(md: &str) -> String {
    let mut out = String::new();
    let mut chunk = String::new();
    let mut open: Vec<&'static str> = Vec::new();
    let mut code = CodeFence::default();
    for line in md.lines() {
        if code.takes(line) {
            chunk.push_str(line);
            chunk.push('\n');
            continue;
        }
        let t = line.trim();
        if let Some((caption, src)) = drawing(t) {
            render(&std::mem::take(&mut chunk), &mut out);
            if src.is_empty() {
                out.push_str("<figure class=\"drawing empty\"><span class=\"small muted\">Empty drawing</span></figure>");
            } else {
                out.push_str("<figure class=\"drawing\"><img src=\"");
                out.push_str(src);
                out.push_str("\" alt=\"Drawing\">");
                if let Some(c) = caption {
                    out.push_str(&format!("<figcaption>{}</figcaption>", escape(c)));
                }
                out.push_str("</figure>");
            }
            continue;
        }
        if let Some((alt, src)) = image(t) {
            render(&std::mem::take(&mut chunk), &mut out);
            let alt = escape(alt);
            let img = format!("<img src=\"{src}\" alt=\"{alt}\" loading=\"lazy\">");
            out.push_str("<figure class=\"image\">");
            // Browsers block top-level navigation to a data: URL, so only a served file gets the click-to-open link.
            if src.starts_with("data:") {
                out.push_str(&img);
            } else {
                out.push_str(&format!(
                    "<a href=\"{src}\" target=\"_blank\" rel=\"noopener\">{img}</a>"
                ));
            }
            if !alt.is_empty() && alt != "image" {
                out.push_str(&format!("<figcaption>{alt}</figcaption>"));
            }
            out.push_str("</figure>");
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

/// `n …` at the start of the popover's line box is a quick note (D55); the rest is the note's text. Needs the space,
/// like `x `, so `nginx fixed` stays a line.
pub fn quick_note(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix("n ")
}

#[cfg(test)]
mod tests {
    use super::{diagram_problem, quick_note, to_html, to_text};

    #[test]
    fn n_space_starts_a_quick_note() {
        assert_eq!(quick_note("  n vendor call #ops"), Some("vendor call #ops"));
        assert_eq!(quick_note("n "), Some(""));
        assert_eq!(quick_note("nginx fixed"), None);
        assert_eq!(quick_note("n"), None);
    }

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
    fn drawing_and_image_blocks_only_on_their_own_line() {
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
            "a ![Drawing](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)",
            "![x](data:text/html;base64,PHNjcmlwdD4=)",
            "![x](data:image/svg+xml,<svg onload=alert(1)>)",
            "![x](data:image/png;base64,AA\"onerror=1)",
            "![x](data:image/bmp;base64,AA==)",
            "a ![x](data:image/png;base64,AA==) b",
        ] {
            let h = to_html(bad);
            assert!(
                !h.contains("=\"data:") && !h.contains("=\"/attachments") && !h.contains("figure"),
                "{bad} → {h}"
            );
        }
    }

    #[test]
    fn a_caption_rides_in_the_drawings_alt_and_the_drawing_stays_a_drawing() {
        let svg = "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=";
        let h = to_html(&format!("![Drawing · รูป 1 &\"x\"]({svg})"));
        assert_eq!(
            h,
            format!(
                "<figure class=\"drawing\"><img src=\"{svg}\" alt=\"Drawing\"><figcaption>รูป 1 &amp;&quot;x&quot;</figcaption></figure>"
            )
        );
        // Raw HTML in a caption is text, never markup.
        let h = to_html(&format!("![Drawing · <script>alert(1)</script>]({svg})"));
        assert!(
            h.contains("&lt;script&gt;") && !h.contains("<script>"),
            "{h}"
        );
        // Not a captioned drawing: `]` in the caption, an empty caption, a caption on the empty drawing, another word.
        for bad in [
            format!("![Drawing · a]b]({svg})"),
            format!("![Drawing · ]({svg})"),
            "![Drawing · c]()".to_string(),
            format!("![Drawing: c]({svg})"),
        ] {
            let h = to_html(&bad);
            assert!(!h.contains("class=\"drawing\""), "{bad} → {h}");
        }
        // Agents read the caption in `note show`.
        let (body, imgs) = super::summarize_images(&format!("![Drawing · flow]({svg})"));
        assert_eq!(
            (body.as_str(), imgs[0].alt.as_str(), imgs[0].kind),
            (
                "![Drawing · flow](data:… 1 KB)",
                "Drawing · flow",
                "drawing"
            )
        );
    }

    #[test]
    fn an_image_line_inside_a_code_fence_stays_code() {
        let png = "![x](data:image/png;base64,AA==)";
        let drawing = "![Drawing](data:image/svg+xml;base64,AA==)";
        for fence in ["```", "~~~", "````"] {
            let md = format!(
                "before\n\n{fence}md\n{png}\n{drawing}\n:::callout\n{fence}\n\n![y](data:image/png;base64,BB==)\n\nafter"
            );
            let h = to_html(&md);
            // The shown lines are text in a <pre>; only the real one after the fence is a figure; the prose is not swallowed.
            assert!(
                h.contains("<pre><code class=\"language-md\">"),
                "{fence}: {h}"
            );
            assert!(
                h.contains(&format!("{png}\n{drawing}\n:::callout\n")),
                "{fence}: {h}"
            );
            assert_eq!(h.matches("<figure").count(), 1, "{fence}: {h}");
            assert!(h.contains("src=\"data:image/png;base64,BB==\""), "{h}");
            assert!(
                h.starts_with("<p>before</p>") && h.ends_with("<p>after</p>\n"),
                "{h}"
            );
            let (body, imgs) = super::summarize_images(&md);
            assert!(
                body.contains(png) && body.contains(drawing),
                "code lines are kept verbatim"
            );
            assert_eq!(imgs.len(), 1, "{fence}: only the real image is listed");
        }
        // A shorter closing run doesn't close it; an unclosed fence runs to the end (as in markdown); inline backticks aren't a fence.
        let h = to_html(&format!("````\n```\n{png}\nstill code"));
        assert!(!h.contains("<figure"), "{h}");
        let h = to_html(&format!("```code``` here\n\n{png}"));
        assert_eq!(h.matches("<figure").count(), 1, "{h}");
    }

    #[test]
    fn agents_diagrams_must_be_boards() {
        let svg = "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=";
        let refused = |md: &str| diagram_problem(md);
        // Refused, with the line number.
        assert_eq!(
            refused("intro\n\n```mermaid\ngraph TD\n```"),
            Some((3, "a mermaid block"))
        );
        assert_eq!(
            refused("~~~ Mermaid\nA-->B\n~~~").map(|p| p.0),
            Some(1),
            "~~~ and case don't matter"
        );
        assert_eq!(refused("  ```mermaid").map(|p| p.0), Some(1));
        assert_eq!(
            refused(&format!("a\n![flow]({svg})")),
            Some((2, "an SVG image that is not an Excalidraw board"))
        );
        assert_eq!(
            refused(&format!("see ![flow]({svg}) here")).map(|p| p.0),
            Some(1),
            "inline SVG"
        );
        assert_eq!(
            refused(&format!("![Drawing · x]({svg}) trailing")).map(|p| p.0),
            Some(1),
            "a board line with junk is not a board"
        );
        assert_eq!(
            refused("![Drawing](data:image/svg+xml;base64,PH\"onerror=1)").map(|p| p.0),
            Some(1)
        );
        assert_eq!(
            refused("![x](data:image/svg+xml,<svg onload=alert(1)>)").map(|p| p.0),
            Some(1)
        );
        // A mermaid fence is refused even inside another fence (the agent is still handing us a mermaid block).
        assert_eq!(refused("````md\n```mermaid\n````").map(|p| p.0), Some(2));
        // Allowed.
        for ok in [
            format!("![Drawing]({svg})"),
            format!("![Drawing · a flow]({svg})"),
            "![Drawing]()".to_string(),
            "![shot](data:image/png;base64,AA==)".to_string(),
            "![shot](data:image/jpeg;base64,AA==)\n\n![](data:image/webp;base64,AA==)".to_string(),
            "Mermaid is what we avoid; a diagram is a board.".to_string(),
            "```rust\nlet mermaid = 1;\n```".to_string(),
            "```\nnot mermaid\n```".to_string(),
            format!("```md\n![flow]({svg})\n```"),
        ] {
            assert_eq!(refused(&ok), None, "{ok}");
        }
    }

    #[test]
    fn only_clean_attachment_paths_become_figures() {
        // The fallback is the old behaviour (a plain relative <img>); what matters is no figure and no link to a traversal.
        for bad in [
            "![x](/attachments/notes/1/../x.png)",
            "![x](/attachments/notes/1/.hidden.png)",
            "![x](/attachments/notes/a/x.png)",
            "![x](/attachments/notes/1/a/b.png)",
            "![x](/attachments/notes/1/)",
            "![x](https://x.io/a.png)",
        ] {
            let h = to_html(bad);
            assert!(!h.contains("figure") && !h.contains("<a "), "{bad} → {h}");
        }
    }

    #[test]
    fn image_block_renders_data_and_attachment_sources() {
        // note 14's shape: agent diagram, Thai alt, svg+xml
        let h = to_html("![รูป 1 · flow](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)");
        assert_eq!(
            h,
            "<figure class=\"image\"><img src=\"data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=\" alt=\"รูป 1 · flow\" loading=\"lazy\"><figcaption>รูป 1 · flow</figcaption></figure>"
        );
        let h = to_html("![x](data:image/png;base64,iVBORw0KGgo=)");
        assert!(
            h.contains("<img src=\"data:image/png;base64,iVBORw0KGgo=\""),
            "{h}"
        );
        // a pasted file: linked to full size, and `image` (the editor's default alt) gets no caption
        let h = to_html("![image](/attachments/notes/14/20261117-101500.png)");
        assert_eq!(
            h,
            "<figure class=\"image\"><a href=\"/attachments/notes/14/20261117-101500.png\" target=\"_blank\" rel=\"noopener\"><img src=\"/attachments/notes/14/20261117-101500.png\" alt=\"image\" loading=\"lazy\"></a></figure>"
        );
        // alt can't break out of its attribute
        let h = to_html("![x\"onerror=1](data:image/png;base64,AA==)");
        assert!(h.contains("alt=\"x&quot;onerror=1\""), "{h}");
        assert!(!h.contains("\"onerror"), "{h}");
        // prose around it still renders
        let h = to_html("before\n![x](data:image/gif;base64,R0lG)\nafter");
        assert!(
            h.starts_with("<p>before</p>") && h.ends_with("<p>after</p>\n"),
            "{h}"
        );
    }

    #[test]
    fn summary_drops_the_base64_and_lists_the_images() {
        let b64 = "A".repeat(2800); // 2100 bytes → 3 KB
        let md = format!(
            "intro\n\n![Drawing](data:image/svg+xml;base64,{b64})\n![รูป 1](data:image/png;base64,{b64})\n![image](/attachments/notes/7/20261117-101500.png)\n![Drawing]()\ntail"
        );
        let (body, imgs) = super::summarize_images(&md);
        assert_eq!(
            body,
            "intro\n\n![Drawing](data:… 3 KB)\n![รูป 1](data:… 3 KB)\n![image](/attachments/notes/7/20261117-101500.png)\n![Drawing]()\ntail"
        );
        let got: Vec<_> = imgs
            .iter()
            .map(|i| (i.alt.as_str(), i.kind, i.kb, i.file.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("Drawing", "drawing", Some(3), ""),
                ("รูป 1", "image", Some(3), ""),
                (
                    "image",
                    "image",
                    None,
                    "/attachments/notes/7/20261117-101500.png"
                ),
            ],
            "an empty drawing is not an image"
        );
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
