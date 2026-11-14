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
