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
    // True 0–100 scale; the floor rides along for the hatch only (owner may go below it).
    assert!(
        s.contains("min=\"0\" max=\"100\" value=\"40\" data-floor=\"18\" style=\"--v:40;--f:18\"")
    );

    // htmx save: 204, unchecked box = off. Below the Glass look's floor (83) is allowed: 40 renders as 40.
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
        b.contains("data-look=\"glass\" style=\"--glass-a:40%\""),
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
    assert!(s.contains("Below 18% (hatched), text can get hard to read on Sumi."));
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
    // Google Calendar, Mac calendars (browser view), Jira.
    assert_eq!(
        off.matches("<span class=\"pill\">Not set up</span>")
            .count(),
        3
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
    // Jira and Mac calendars.
    assert_eq!(
        half.matches("<span class=\"pill\">Not set up</span>")
            .count(),
        2
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

#[tokio::test]
async fn launch_page_wears_the_look_and_off_skips_it() {
    let st = state();
    let r = get(&st, "/launch").await;
    assert_eq!(r.status, 200);
    // Standalone: the same <html> look + glass attributes as every page, the sheet + logo shell, no sidebar.
    assert!(
        r.body
            .contains("<html lang=\"en\" data-look=\"sumi\" style=\"--glass-a:60%\">"),
        "{:.300}",
        r.body
    );
    assert!(
        r.body.contains("data-launch=\"ink\" data-then=\"/\""),
        "ink is the default"
    );
    for asset in ["/static/css/launch.css", "/static/js/launch.js"] {
        assert!(r.body.contains(asset), "{asset}");
        assert_eq!(get(&st, asset).await.status, 200, "{asset}");
    }
    assert!(!r.body.contains("class=\"side"));

    post(&st, "/settings", &form(&[("look", "wakaba")]), false).await;
    post(
        &st,
        "/settings/glass",
        &form(&[("glass_opacity", "40"), ("preset", "tinted")]),
        true,
    )
    .await;
    post(&st, "/settings/launch", &form(&[("launch", "glass")]), true).await;
    let b = get(&st, "/launch").await.body;
    assert!(b.contains("data-look=\"wakaba\""), "{b:.300}");
    assert!(
        b.contains("data-tint=\"tinted\" style=\"--glass-a:85%\""),
        "{b:.300}"
    );
    assert!(b.contains("href=\"/static/icon/wakaba.svg\""));
    assert!(b.contains("data-launch=\"glass\""));

    // Off: straight to Today, no page.
    post(&st, "/settings/launch", &form(&[("launch", "off")]), true).await;
    let r = get(&st, "/launch").await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/");
    // A hand-edited value can't break the launch: unknown → Ink.
    exec(
        &st,
        "UPDATE settings SET value = 'spin' WHERE key = 'launch'",
    );
    assert_eq!(get(&st, "/launch").await.status, 200);
}

#[tokio::test]
async fn launch_setting_saves_validates_and_shows_in_settings() {
    let st = state();
    let pressed = |b: &str, v: &str, on: bool| {
        b.contains(&format!(
            "name=\"launch\" value=\"{v}\" aria-pressed=\"{on}\""
        ))
    };
    let s = get(&st, "/settings").await.body;
    assert!(pressed(&s, "ink", true) && pressed(&s, "glass", false) && pressed(&s, "off", false));
    assert!(s.contains("Launch animation</b>"));
    assert!(
        s.contains("class=\"gi lprow\"><button class=\"launch mini keep\""),
        "preview shows"
    );

    // htmx autosave: 204, nothing else.
    let r = post(&st, "/settings/launch", &form(&[("launch", "glass")]), true).await;
    assert_eq!(r.status, 204);
    let s = get(&st, "/settings").await.body;
    assert!(pressed(&s, "glass", true) && pressed(&s, "ink", false));
    assert!(s.contains("data-launch=\"glass\""));

    // No JS: a plain POST goes back to Settings.
    let r = post(&st, "/settings/launch", &form(&[("launch", "off")]), false).await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/settings");
    let s = get(&st, "/settings").await.body;
    assert!(pressed(&s, "off", true));
    assert!(
        s.contains("class=\"gi lprow\" hidden>"),
        "no preview while Off"
    );

    // Unknown value: refused, the stored one stays.
    for bad in ["spin", "", "INK"] {
        let r = post(&st, "/settings/launch", &form(&[("launch", bad)]), true).await;
        assert_eq!(r.status, 422, "{bad:?}");
    }
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM settings WHERE key = 'launch' AND value = 'off'"
        ),
        1
    );
}

#[tokio::test]
async fn mac_calendars_row() {
    use atharu::recording::{Access, MacCalendar, MacCalendars};
    // A browser: status only, the choice of calendars is the app's.
    let st = state();
    let b = get(&st, "/settings").await.body;
    assert!(
        b.contains("<b>Mac calendars</b>")
            && b.contains("Set up in the Mac app.")
            && b.contains("Not set up")
    );
    let r = post(&st, "/settings/maccal", "on=1&cal=x", false).await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/settings#maccal");
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM settings WHERE key = 'mac_cals'"),
        0
    );

    // The Mac app: switch + one box per calendar; a save stores the checked ids and kicks a sync.
    let mut st = state();
    let mc = std::sync::Arc::new(MacCalendars::default());
    mc.set_access(Access::Granted);
    mc.set_list(vec![
        MacCalendar {
            id: "W1".into(),
            title: "Work".into(),
            account: "me@work.example".into(),
            color: "#1a73e8".into(),
        },
        MacCalendar {
            id: "H2".into(),
            title: "Home".into(),
            account: "iCloud".into(),
            color: "red;x:y".into(),
        },
    ]);
    st.mac_cals = Some(mc.clone());
    let r = post(&st, "/settings/maccal", "on=1&cal=W1", true).await;
    assert_eq!(r.status, 200);
    assert!(r.body.starts_with("<form id=\"maccal\""));
    assert!(r.body.contains("name=\"on\" value=\"1\" checked"));
    assert!(
        r.body.contains("name=\"cal\" value=\"W1\" checked>")
            && r.body.contains("name=\"cal\" value=\"H2\">")
    );
    assert!(
        r.body.contains("background: #1a73e8") && r.body.contains("background: #8e8e93"),
        "{}",
        r.body
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM settings WHERE key = 'mac_cals' AND value = 'W1'"
        ),
        1
    );
    // The kick is waiting for the app's sync loop.
    tokio::time::timeout(std::time::Duration::from_millis(100), mc.kick.notified())
        .await
        .unwrap();

    mc.set_access(Access::Denied);
    let b = get(&st, "/settings").await.body;
    assert!(b.contains(
        "Calendar access is off. Open System Settings › Privacy &amp; Security › Calendars."
    ));

    // Granted but not synced yet: the row polls itself; once synced it stops.
    mc.set_access(Access::Granted);
    let b = get(&st, "/settings/maccal").await.body;
    assert!(
        b.contains("hx-get=\"/settings/maccal\" hx-trigger=\"every 2s\""),
        "{b}"
    );
    exec(
        &st,
        "INSERT INTO settings(key, value) VALUES ('mac_cal_synced', '2026-11-14 09:00:00')",
    );
    assert!(!get(&st, "/settings/maccal").await.body.contains("every 2s"));

    // Off (D7): the synced events go, the ticked calendars stay.
    exec(
        &st,
        "INSERT INTO events(source, ext_id, title, starts_at) VALUES ('mac', 'm@1', 'Standup', '2026-11-12T09:30'), ('own', NULL, 'mine', '2026-11-12')",
    );
    post(&st, "/settings/maccal", "", true).await;
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM events WHERE source = 'mac'"),
        0
    );
    assert_eq!(count(&st, "SELECT COUNT(*) FROM events"), 1);
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM settings WHERE key = 'mac_cals' AND value = 'W1'"
        ),
        1
    );
}

// ---- About › Update, the Mac app's button (0.2.6). Never drives `check` (a real `git fetch`) or a real `start` (it would
// run this checkout's scripts/install.sh): those are unit-tested in updater.rs against a fake script.

use atharu::updater::{self, UpdateState};
use std::time::Instant;

#[tokio::test]
async fn update_button_is_the_apps_and_never_started_from_a_browser() {
    let st = state();
    // The browser page keeps the Terminal row; the app's row is in the page (CSS shows it by data-app).
    let b = get(&st, "/settings").await.body;
    assert!(b.contains("<code id=\"upd-cmd\">atharu update</code>"));
    assert!(b.contains("<div id=\"update\" class=\"mac-only\">"));
    assert!(b.contains("Check for updates</button>"));

    // Start from a browser UA: refused, and nothing starts even with a tag found.
    *updater::lock(&st.update) = UpdateState::Available("v9.9.9".into());
    let r = post(&st, "/settings/update/start", "", true).await;
    assert_eq!(r.status, 403);
    assert!(r.body.contains("Update from Terminal: atharu update"));
    assert_eq!(
        updater::snapshot(&st.update),
        UpdateState::Available("v9.9.9".into())
    );

    // From the app but with nothing found yet (Idle): a no-op that answers with the row.
    *updater::lock(&st.update) = UpdateState::Idle;
    let req = axum::http::Request::post("/settings/update/start")
        .header("user-agent", "Mozilla/5.0 AppleWebKit/605.1.15 AtharuMac")
        .header("hx-request", "true")
        .body(axum::body::Body::empty())
        .unwrap();
    let r = send(atharu::app::router(st.clone()), req).await;
    assert_eq!(r.status, 200);
    assert_eq!(updater::snapshot(&st.update), UpdateState::Idle);
    assert!(r.body.contains("Check for updates"));
}

#[tokio::test]
async fn update_row_says_each_state_in_plain_words() {
    let st = state();
    let row = |s: UpdateState| {
        *updater::lock(&st.update) = s;
        let st = st.clone();
        async move { get(&st, "/settings/update").await.body }
    };
    let b = row(UpdateState::UpToDate("v0.2.6".into())).await;
    assert!(b.contains("✓ v0.2.6 is the newest") && b.contains("Check again"));
    assert!(!b.contains("hx-trigger=\"every 2s\""));

    let b = row(UpdateState::Available("v0.2.7".into())).await;
    assert!(b.contains("v0.2.7 is available") && b.contains(">Update</button>"));
    assert!(b.contains("action=\"/settings/update/start\""));

    // Running: the title, the log's last line, and the poll (only here). In-memory DB → the log is `<temp>/update.log`.
    std::fs::write(
        std::env::temp_dir().join("update.log"),
        "Compiling a\nCompiling atharu v0.2.7 <x>\n\n",
    )
    .unwrap();
    let b = row(UpdateState::Running {
        to: "v0.2.7".into(),
        started: Instant::now(),
    })
    .await;
    assert!(b.contains("Updating to v0.2.7 · 0 s"), "{b}");
    assert!(b.contains("<span class=\"mono upd-last\">Compiling atharu v0.2.7 &#60;x&#62;</span>"));
    assert!(b.contains("hx-get=\"/settings/update\" hx-trigger=\"every 2s\""));
    assert!(!b.contains("action=\"/settings/update/start\""));

    let b = row(UpdateState::Done("v0.2.7".into())).await;
    assert!(b.contains("✓ v0.2.7 is installed"));
    assert!(b.contains("<a class=\"btn\" href=\"/app/restart\">Restart Atharu</a>"));

    let b = row(UpdateState::Failed {
        to: "v0.2.7".into(),
        why: "install.sh exited 3; v0.2.6 is still installed".into(),
    })
    .await;
    assert!(
        b.contains("Couldn’t install v0.2.7")
            && b.contains("install.sh exited 3; v0.2.6 is still installed")
    );
    assert!(b.contains("update.log") && b.contains("Try again"));

    let b = row(UpdateState::Failed {
        to: String::new(),
        why: "git fetch failed in /x: offline".into(),
    })
    .await;
    assert!(b.contains("Couldn’t check for updates") && !b.contains("Log:"));
}

#[tokio::test]
async fn a_timed_out_row_hides_try_again_while_the_install_still_runs() {
    // A fake install.sh that outlives a 200 ms deadline: the row gives up, the process (and the slot) does not.
    let st = state();
    let dir = std::env::temp_dir().join(format!("atharu-settings-slow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::write(dir.join("scripts/install.sh"), "sleep 2\n").unwrap();
    assert!(updater::start_with(
        &st.update,
        dir.clone(),
        "v0.2.7".into(),
        dir.join("update.log"),
        std::time::Duration::from_millis(200)
    ));
    for _ in 0..100 {
        if !matches!(updater::snapshot(&st.update), UpdateState::Running { .. }) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(updater::busy(&st.update));
    let b = get(&st, "/settings/update").await.body;
    assert!(b.contains("still running in the background"), "{b}");
    assert!(!b.contains("Try again"));
    assert!(b.contains("hx-get=\"/settings/update\" hx-trigger=\"every 2s\""));

    // When it ends the row says how it went, and a Failed row (not busy) offers Try again again.
    for _ in 0..100 {
        if !updater::busy(&st.update) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(
        updater::snapshot(&st.update),
        UpdateState::Done("v0.2.7".into())
    );
    *updater::lock(&st.update) = UpdateState::Failed {
        to: "v0.2.7".into(),
        why: "install.sh exited 3; v0.2.6 is still installed".into(),
    };
    let b = get(&st, "/settings/update").await.body;
    assert!(b.contains("Try again") && !b.contains("hx-trigger=\"every 2s\""));
    let _ = std::fs::remove_dir_all(&dir);
}
