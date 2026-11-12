mod common;
use common::*;

const ICS: &str = include_str!("fixtures/google.ics");

fn seed(st: &atharu::app::AppState) {
    exec(
        st,
        "INSERT INTO epics(key, target) VALUES ('OPS-101', '2026-11-13');
         INSERT INTO epics(key, target, status) VALUES ('OPS-9', '2026-11-13', 'done');
         INSERT INTO entries(day, kind, text, plan_state, due) VALUES
           ('2026-11-02', 'plan', 'send retention runbook', 'open', '2026-11-09'),
           ('2026-11-09', 'plan', 'ship checkout index', 'open', '2026-11-11'),
           ('2026-11-09', 'plan', 'already done', 'done', '2026-11-11'),
           ('2026-11-09', 'plan', 'next week thing', 'open', '2026-11-17');
         INSERT INTO entries(day, kind, text) VALUES
           ('2026-11-11', 'done', 'a'), ('2026-11-11', 'done', 'b'), ('2026-11-10', 'done', 'c');
         INSERT INTO meetings(day, name, ended_at) VALUES ('2026-11-11', 'oms-sync', '2026-11-11 10:42:00');
         INSERT INTO events(title, starts_at) VALUES ('dentist', '2026-11-11T16:00'), ('far', '2026-12-01');",
    );
}

#[tokio::test]
async fn week_assembles_what_we_have() {
    let st = state();
    seed(&st);
    let b = get(&st, "/calendar").await.body;
    assert!(b.contains("<h1 class=\"ptitle\">9 – 15 Nov</h1>"));
    assert!(
        b.contains("<div class=\"dh now\"><a href=\"/?day=2026-11-11\">Wed 11 · today</a></div>")
    );
    // Overdue to-do in red; done and out-of-range ones absent.
    assert!(b.contains("<a class=\"ev todo late\" href=\"/todos\">send retention runbook</a>"));
    assert!(b.contains(">ship checkout index</a>"));
    assert!(!b.contains("already done") && !b.contains("next week thing"));
    assert!(b.contains("href=\"/epics/OPS-101\">target · OPS-101</a>"));
    assert!(!b.contains("OPS-9"), "closed epics have no target to hit");
    assert!(
        b.contains("href=\"/meetings/1\">oms-sync<br><span class=\"muted\">ended 10:42</span>")
    );
    assert!(b.contains("● 2 lines") && b.contains("● 1 line<"));
    // Own event: start-meeting button + delete; out-of-range one absent.
    assert!(b.contains("<input type=\"hidden\" name=\"name\" value=\"dentist\">"));
    assert!(b.contains(">16:00 dentist</button>"));
    assert!(b.contains("/calendar/events/1/delete"));
    assert!(!b.contains(">far<"));
    // No ical_url: the callout explains how to add one; no Refresh button.
    assert!(b.contains("Secret address in iCal format"));
    assert!(!b.contains("/calendar/refresh"));

    let b = get(&st, "/calendar?week=2026-11-18").await.body;
    assert!(b.contains("16 – 22 Nov") && b.contains("next week thing"));
    assert!(b.contains("href=\"/calendar?week=2026-11-11\" aria-label=\"Previous week\""));

    let b = get(&st, "/calendar?week=2026-11-30&view=month").await.body;
    assert!(b.contains("<h1 class=\"ptitle\">November 2026</h1>"));
    // Whole weeks: Mon 26 Oct .. Sun 6 Dec, so 1 Dec's event is in the trailing week.
    assert!(
        b.contains("href=\"/?day=2026-10-26\">26</a>")
            && b.contains("href=\"/?day=2026-12-06\">6</a>")
    );
    assert!(b.contains(">far</button>") && b.contains("dc out"));
    assert!(
        b.contains("href=\"/calendar?week=2026-12-01&#38;view=month\" aria-label=\"Next month\"")
    );
}

#[tokio::test]
async fn own_event_add_and_delete() {
    let st = state();
    let add = |t: &str, d: &str, tm: &str| {
        form(&[
            ("title", t),
            ("date", d),
            ("time", tm),
            ("week", "2026-11-11"),
            ("view", "week"),
        ])
    };
    let r = post(
        &st,
        "/calendar/events",
        &add("dentist", "2026-11-12", "16:00"),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(
        r.body.starts_with("<section id=\"cal\">") && r.body.contains(">16:00 dentist</button>")
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM events WHERE source = 'own' AND starts_at = '2026-11-12T16:00'"
        ),
        1
    );

    // No JS: 303 to the range holding the new event (all-day, next month).
    let r = post(
        &st,
        "/calendar/events",
        &add("offsite", "2026-12-02", ""),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/calendar?week=2026-12-02");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM events WHERE starts_at = '2026-12-02'"
        ),
        1
    );

    let r = post(&st, "/calendar/events", &add("  ", "2026-11-12", ""), true).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("Type a title first."));
    let r = post(&st, "/calendar/events", &add("x", "someday", ""), false).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("Pick a date.") && r.body.contains("value=\"someday\""));
    let r = post(
        &st,
        "/calendar/events",
        &add("x", "2026-11-12", "4pm"),
        true,
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM events"), 2);

    let r = post(
        &st,
        "/calendar/events/1/delete",
        &form(&[("week", "2026-11-11")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(!r.body.contains("dentist"));
    let r = post(
        &st,
        "/calendar/events/2/delete",
        &form(&[("week", "2026-12-02"), ("view", "month")]),
        false,
    )
    .await;
    assert_eq!(
        r.headers["location"],
        "/calendar?week=2026-12-02&view=month"
    );
    assert_eq!(count(&st, "SELECT COUNT(*) FROM events"), 0);
    assert_eq!(
        post(&st, "/calendar/events/1/delete", "", true)
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn google_ics_imports_idempotently_and_never_lets_us_delete() {
    let st = state();
    assert_eq!(atharu::calendar::import(&mut st.conn(), ICS).unwrap(), 5);
    assert_eq!(atharu::calendar::import(&mut st.conn(), ICS).unwrap(), 5);
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM events WHERE source = 'google'"),
        5
    );
    assert_eq!(count(&st, "SELECT COUNT(DISTINCT ext_id) FROM events"), 5);

    // Upstream edits land; events gone from the feed go too.
    let edited = ICS
        .replace("SUMMARY:Team offsite", "SUMMARY:Team offsite (moved)")
        .replace(
            "UID:utc@google.com",
            "UID:utc@google.com\r\nSTATUS:CANCELLED",
        );
    assert_eq!(
        atharu::calendar::import(&mut st.conn(), &edited).unwrap(),
        4
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM events WHERE title = 'Team offsite (moved)'"
        ),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM events WHERE ext_id = 'utc@google.com'"
        ),
        0
    );

    let b = get(&st, "/calendar").await.body;
    assert!(
        b.contains(">10:00 oms-sync, weekly</button>")
            && b.contains(">Team offsite (moved)</button>")
    );
    // Read-only: no delete button on Google events, and the route refuses them.
    assert!(!b.contains("/calendar/events/1/delete"));
    assert_eq!(
        post(&st, "/calendar/events/1/delete", "", true)
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn ical_url_turns_on_refresh_and_hides_the_callout() {
    let dir = std::env::temp_dir().join(format!("atharu-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    assert!(
        atharu::config::FileConfig::load(&path).ical_url.is_none(),
        "missing file = off"
    );
    std::fs::write(
        &path,
        "ical_url = \"https://calendar.example/secret/basic.ics\"\n",
    )
    .unwrap();
    let cfg = atharu::config::FileConfig::load(&path);
    assert_eq!(
        cfg.ical_url.as_deref(),
        Some("https://calendar.example/secret/basic.ics")
    );
    std::fs::write(&path, "ical_url = 3").unwrap();
    assert!(
        atharu::config::FileConfig::load(&path).ical_url.is_none(),
        "bad file = off"
    );
    std::fs::remove_dir_all(&dir).unwrap();

    let mut st = state();
    st.settings = std::sync::Arc::new(cfg);
    let b = get(&st, "/calendar").await.body;
    assert!(b.contains("action=\"/calendar/refresh\"") && b.contains(">Refresh</button>"));
    assert!(!b.contains("Secret address in iCal format"));
    assert!(
        !b.contains("calendar.example"),
        "the URL is a credential, never on the page"
    );
}
