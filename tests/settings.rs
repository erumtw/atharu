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
    // True 0–100 scale; the floor rides along for the hatch + clamp, not as `min`.
    assert!(
        s.contains("min=\"0\" max=\"100\" value=\"40\" data-floor=\"18\" style=\"--v:40;--f:18\"")
    );

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

#[tokio::test]
async fn text_blur_round_trips_htmx_and_plain_post() {
    let st = state();
    let s = get(&st, "/settings").await.body;
    assert!(
        s.contains("role=\"switch\" name=\"text_blur\" value=\"1\">"),
        "off by default"
    );
    assert!(!s.contains("data-blur"));
    // htmx: 204, the switch is the whole state of the form, so the other settings ride along unchanged.
    let r = post(
        &st,
        "/settings/glass",
        &form(&[("text_blur", "1"), ("glass_opacity", "60")]),
        true,
    )
    .await;
    assert_eq!(r.status, 204);
    for p in PAGES {
        assert!(
            get(&st, p)
                .await
                .body
                .contains(" data-blur=\"text\" style="),
            "{p}"
        );
    }
    assert!(
        get(&st, "/settings")
            .await
            .body
            .contains("name=\"text_blur\" value=\"1\" checked")
    );
    // Plain form POST without the checkbox = off, 303 back.
    let r = post(
        &st,
        "/settings/glass",
        &form(&[("glass_opacity", "60")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert!(!get(&st, "/").await.body.contains("data-blur"));
}

#[tokio::test]
async fn slider_sits_on_the_true_scale() {
    let st = state();
    let s = get(&st, "/settings").await.body;
    // Default 60 on Sumi: thumb at 60% of a 0–100 track (the T24 slider put it near the left, min = floor).
    assert!(s.contains(
        "type=\"range\" name=\"glass_opacity\" min=\"0\" max=\"100\" value=\"60\" data-floor=\"18\" style=\"--v:60;--f:18\""
    ));
    assert!(s.contains("The slider stops at 18%."));
    assert!(
        s.contains("role=\"switch\" name=\"glass_all\" value=\"1\">"),
        "switch off, still a form field"
    );
    assert!(s.contains("value=\"sumi\" checked"));
    assert!(s.contains("name=\"preset\" value=\"clear\" aria-pressed=\"true\""));
}

#[tokio::test]
async fn connections_show_status_from_config_never_secrets() {
    let dir = std::env::temp_dir().join(format!("atharu-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");

    let mut st = state();
    let off = get(&st, "/settings").await.body;
    assert_eq!(
        off.matches("<span class=\"pill\">Not set up</span>")
            .count(),
        2
    );

    // Jira half-filled = off; iCal set = on.
    std::fs::write(
        &path,
        "ical_url = \"https://cal.example/secret-ical\"\njira_base_url = \"https://acme.atlassian.net\"\njira_email = \"me@example.com\"\n",
    )
    .unwrap();
    st.settings = std::sync::Arc::new(atharu::config::FileConfig::load(&path));
    let half = get(&st, "/settings").await.body;
    assert!(half.contains("Google Calendar</b>"));
    assert_eq!(half.matches("<span class=\"pill ok\">On</span>").count(), 1);
    assert_eq!(
        half.matches("<span class=\"pill\">Not set up</span>")
            .count(),
        1
    );

    std::fs::write(
        &path,
        "ical_url = \"https://cal.example/secret-ical\"\njira_base_url = \"https://acme.atlassian.net\"\njira_email = \"me@example.com\"\njira_token = \"tok-SECRET\"\n",
    )
    .unwrap();
    st.settings = std::sync::Arc::new(atharu::config::FileConfig::load(&path));
    let on = get(&st, "/settings").await.body;
    assert_eq!(on.matches("<span class=\"pill ok\">On</span>").count(), 2);
    for secret in [
        "secret-ical",
        "acme.atlassian",
        "me@example.com",
        "tok-SECRET",
    ] {
        assert!(!on.contains(secret), "{secret} leaked");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn about_shows_the_running_version_and_the_update_command() {
    let b = get(&state(), "/settings").await.body;
    let v = atharu::build_info::TAG.map_or_else(
        || format!("{} · dev build", atharu::build_info::VERSION),
        str::to_string,
    );
    assert!(b.contains(&v), "version {v}");
    assert!(b.contains(atharu::build_info::SHA));
    assert!(b.contains("<code id=\"upd-cmd\">atharu update</code>"));
}
