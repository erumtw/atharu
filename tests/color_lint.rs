//! 0.2.9 (D3): the CUBE token layer holds: no raw colour in static/css outside tokens.css and the per-look blocks of looks.css.

#[test]
fn no_raw_colour_outside_the_tokens() {
    let out = std::process::Command::new("scripts/color-lint.sh")
        .output()
        .expect("scripts/color-lint.sh runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// What the lint cannot see (the colour is a token, but the wrong role): drawings are dark ink on transparent, so their paper
/// must be `--paper` (white in every look), not a themed surface that turns dark in Yoru and hides the strokes.
#[test]
fn drawings_sit_on_paper_in_every_look() {
    let css = std::fs::read_to_string("static/css/components.css").unwrap();
    for rule in [".note-body .drawing{", ".drawsheet{"] {
        let at = css.find(rule).unwrap_or_else(|| panic!("{rule} rule"));
        let end = css[at..].find('}').unwrap();
        assert!(
            css[at..at + end].contains("background:var(--paper)"),
            "{rule} must use --paper"
        );
    }
}
