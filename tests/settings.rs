mod common;
use common::*;

const PAGES: [&str; 9] = [
    "/",
    "/todos",
    "/calendar",
    "/epics",
    "/report",
    "/meetings",
    "/learn",
    "/notes",
    "/settings",
];

#[tokio::test]
async fn look_persists_and_every_page_wears_it() {
    let st = state();
    for p in PAGES {
        let b = get(&st, p).await.body;
        assert!(
            b.contains("<html lang=\"en\" data-look=\"sumi\" style=\"--glass-a:60%\">"),
            "default on {p}"
        );
    }
    let r = post(&st, "/settings", &form(&[("look", "glass")]), false).await;
    assert_eq!(r.status, 303);
    for p in PAGES {
        let b = get(&st, p).await.body;
        assert!(b.contains("data-look=\"glass\""), "{p}");
        assert!(
            b.contains("href=\"/static/icon/glass.svg\""),
            "favicon on {p}"
        );
    }
    let s = get(&st, "/settings").await.body;
    assert!(s.contains("value=\"glass\" checked"));
    assert_eq!(
        post(&st, "/settings", &form(&[("look", "neon")]), false)
            .await
            .status,
        422
    );
    // A hand-edited value can't break pages: unknown → default.
    exec(&st, "UPDATE settings SET value = 'neon' WHERE key = 'look'");
    assert!(get(&st, "/").await.body.contains("data-look=\"sumi\""));
}

#[tokio::test]
async fn favicon_per_look() {
    let st = state();
    for l in atharu::domain::look::LOOKS {
        let r = get(&st, &format!("/static/icon/{}.svg", l.slug)).await;
        assert_eq!(r.status, 200, "{}", l.slug);
        assert_eq!(r.headers["content-type"], "image/svg+xml");
        assert!(r.body.starts_with("<svg "));
    }
}

#[tokio::test]
async fn glass_settings_persist_and_render_on_html() {
    let st = state();
    // Plain form POST (no JS): switch on, slider at 40 on Sumi (floor 18).
    let r = post(
        &st,
        "/settings/glass",
        &form(&[("glass_all", "1"), ("glass_opacity", "40")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    for p in PAGES {
        let b = get(&st, p).await.body;
        assert!(
            b.contains("data-glass=\"all\" style=\"--glass-a:40%\""),
            "{p}"
        );
    }
    let s = get(&st, "/settings").await.body;
    assert!(s.contains("name=\"glass_all\" value=\"1\" checked"));
    assert!(s.contains("min=\"18\" max=\"100\" value=\"40\""));

    // htmx save: 204, unchecked box = off. The Glass look's floor (83) lifts the stored 40 on read.
    post(&st, "/settings", &form(&[("look", "glass")]), false).await;
    let r = post(
        &st,
        "/settings/glass",
        &form(&[("glass_opacity", "40")]),
        true,
    )
    .await;
    assert_eq!(r.status, 204);
    let b = get(&st, "/").await.body;
    assert!(
        b.contains("data-look=\"glass\" style=\"--glass-a:83%\""),
        "{b:.200}"
    );

    // Presets: Tinted = 85 + tint, Clear = the look's floor, no tint; slider moves keep the tint.
    post(&st, "/settings/glass", &form(&[("preset", "tinted")]), true).await;
    assert!(
        get(&st, "/")
            .await
            .body
            .contains("data-tint=\"tinted\" style=\"--glass-a:85%\"")
    );
    post(
        &st,
        "/settings/glass",
        &form(&[("glass_opacity", "95")]),
        true,
    )
    .await;
    assert!(
        get(&st, "/")
            .await
            .body
            .contains("data-tint=\"tinted\" style=\"--glass-a:95%\"")
    );
    post(&st, "/settings/glass", &form(&[("preset", "clear")]), true).await;
    let b = get(&st, "/").await.body;
    assert!(b.contains("data-look=\"glass\" style=\"--glass-a:83%\""));
    assert!(!b.contains("data-tint"));
    assert_eq!(
        post(&st, "/settings/glass", &form(&[("preset", "neon")]), true)
            .await
            .status,
        422
    );
}
