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
         INSERT INTO meetings(day, name, started_at, ended_at) VALUES
           ('2026-11-12', 'checkout-sync', '2026-11-12 10:00:00', '2026-11-12 10:35:00');
         INSERT INTO events(title, starts_at) VALUES ('dentist', '2026-11-11T16:00'), ('far', '2026-12-01'),
           ('early', '2026-11-13T06:30');
         INSERT INTO events(source, ext_id, title, starts_at, ends_at) VALUES
           ('google', 'g1', 'Supplier call', '2026-11-12T10:15', '2026-11-12T11:00');",
    );
}

#[tokio::test]
async fn week_assembles_what_we_have() {
    let st = state();
    seed(&st);
    let b = get(&st, "/calendar").await.body;
    assert!(b.contains("<h1 class=\"ptitle\">8 – 14 Nov</h1>"));
    // Today's header opens its Day view; "N lines" opens its log.
    assert!(b.contains(
        "<div class=\"now\"><a href=\"/calendar?d=2026-11-11&#38;view=day\">Wed <b>11</b></a><a class=\"lines\" href=\"/?day=2026-11-11\">2 lines</a></div>"
    ), "{b}");
    assert!(b.contains(">1 line</a>"));
    // All-day row: overdue to-do in red; done and out-of-range ones absent.
    assert!(
        b.contains("<a class=\"ev todo late\" href=\"/todos\"><b>send retention runbook</b></a>")
    );
    assert!(b.contains("<b>ship checkout index</b>"));
    assert!(!b.contains("already done") && !b.contains("next week thing"));
    assert!(b.contains("href=\"/epics/OPS-101\"><b>target · OPS-101</b></a>"));
    assert!(!b.contains("OPS-9"), "closed epics have no target to hit");
    // A meeting from before start times were kept sits in the all-day row.
    assert!(b.contains("<b>oms-sync</b> <small>ended 10:42</small></button>"));
    // A meeting's chip opens its card: the meeting, its series, that day's log.
    assert!(b.contains("<button class=\"ev meet\" type=\"button\" popovertarget=\"evc1\" style=\"anchor-name: --evc1\">"), "{b}");
    assert!(
        b.contains("<h4>oms-sync</h4>\n<span class=\"small muted\">Wed 11 · ended 10:42</span>")
    );
    assert!(b.contains("<a class=\"btn s\" href=\"/meetings/1\">Open meeting</a><a class=\"btn ghost s\" href=\"/meetings?name=oms-sync\">Open series</a>"));
    assert!(b.contains("<a class=\"btn ghost s\" href=\"/?day=2026-11-11\">Day's log</a>"));
    // An empty hour is a link that opens "+ Event" on that day and hour, keeping the range.
    assert!(b.contains(
        "<a class=\"slot\" href=\"/calendar?d=2026-11-11&#38;new=2026-11-12T16:00\" tabindex=\"-1\""
    ));
    // Timed: the grid widens to 06:00 for "early", so 16:00 is 600 min down; an own event runs an hour.
    assert!(b.contains("top: calc(var(--px) * 1)\">07:00</span>") && !b.contains(">06:00<"));
    assert!(b.contains("<div class=\"tev\" style=\"top: calc(var(--px) * 600 / 60); height: calc(var(--px) * 60 / 60 - 2px); left: calc(0 * 100% / 1 + 2px); width: calc(100% / 1 - 4px)\">"));
    // An event's card starts meeting mode with its name; an own one can be deleted there.
    assert!(b.contains("<input type=\"hidden\" name=\"name\" value=\"dentist\"><input type=\"hidden\" name=\"back\" value=\"/\"><button class=\"btn s\" type=\"submit\">Start meeting</button>"));
    assert!(b.contains("· 16:00 · your event</span>"));
    assert!(b.contains("<span class=\"t\">16:00</span> <b>dentist</b>"));
    // Thu: the meeting (10:00–10:35) and the Google call (10:15–11:00) overlap, so they split the column.
    assert!(b.contains("left: calc(0 * 100% / 2 + 2px); width: calc(100% / 2 - 4px)\"><button class=\"ev meet\" type=\"button\" popovertarget=\"evc2\" style=\"anchor-name: --evc2\"><span class=\"t\">10:00</span> <b>checkout-sync</b> <small>10:00 – 10:35</small>"));
    assert!(
        b.contains("left: calc(1 * 100% / 2 + 2px)") && b.contains("<small>10:15 – 11:00</small>")
    );
    assert!(b.contains("/calendar/events/1/delete"));
    assert!(!b.contains(">far<"));
    // Header: Today · ‹ › · range · view switch (no view in the URL = N days) · + Event.
    assert!(b.contains("<a class=\"btn ghost s\" href=\"/calendar\">Today</a>"));
    assert!(b.contains("<nav class=\"seg\" aria-label=\"View\"><a href=\"/calendar?view=day\">Day</a><a href=\"/calendar\" class=\"on\" aria-current=\"page\">N days</a><a href=\"/calendar?view=week\">Week</a><a href=\"/calendar?view=month\">Month</a></nav>"), "{b}");
    // No legend, no setup paragraph (it lives in Settings), no Refresh without ical_url.
    assert!(!b.contains("Secret address in iCal format") && !b.contains("lines logged that day"));
    assert!(
        get(&st, "/settings")
            .await
            .body
            .contains("Secret address in iCal format")
    );
    assert!(!b.contains("/calendar/refresh"));

    // `?week=` (pre-0.2.1 links) still points the range.
    let b = get(&st, "/calendar?week=2026-11-18").await.body;
    assert!(b.contains("15 – 21 Nov") && b.contains("next week thing"));
    assert!(b.contains("href=\"/calendar?d=2026-11-11\" aria-label=\"Previous week\""));

    let b = get(&st, "/calendar?d=2026-11-30&view=month").await.body;
    assert!(b.contains("<h1 class=\"ptitle\">November 2026</h1>"));
    // Whole weeks, Sunday first: 1 Nov is a Sunday, so Sun 1 Nov .. Sat 5 Dec; 1 Dec's event is in the trailing week.
    assert!(b.contains("<div>Sun</div><div>Mon</div>"));
    // A day number opens that day's Day view.
    assert!(
        b.contains(
            "href=\"/calendar?d=2026-11-01&#38;view=day\" aria-label=\"Sun 1: open the day\">1</a>"
        ) && b.contains("href=\"/calendar?d=2026-12-05&#38;view=day\"")
            && !b.contains("2026-10-31")
            && !b.contains("2026-12-06")
    );
    assert!(b.contains("<b>far</b></button>") && b.contains("mc out"));
    // Month cells list timed items with their time.
    assert!(b.contains("<span class=\"t\">10:00</span> <b>checkout-sync</b>"));
    assert!(b.contains("href=\"/calendar?d=2026-12-01&#38;view=month\" aria-label=\"Next month\""));
}

#[tokio::test]
async fn own_event_add_and_delete() {
    let st = state();
    let add = |t: &str, d: &str, tm: &str| {
        form(&[
            ("title", t),
            ("date", d),
            ("time", tm),
            ("d", "2026-11-11"),
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
    assert!(r.body.starts_with("<section id=\"cal\">") && r.body.contains("<b>dentist</b>"));
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
    assert_eq!(r.headers["location"], "/calendar?d=2026-12-02&view=week");
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

    // An empty slot's link opens the form with its day and hour.
    let b = get(&st, "/calendar?d=2026-11-11&new=2026-11-12T16:00")
        .await
        .body;
    assert!(b.contains("<details class=\"evadd\" open>"));
    assert!(
        b.contains("name=\"date\" value=\"2026-11-12\"")
            && b.contains("name=\"time\" value=\"16:00\"")
    );
    assert!(b.contains("required autofocus>"));

    let r = post(
        &st,
        "/calendar/events/1/delete",
        &form(&[("d", "2026-11-11")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(!r.body.contains("dentist"));
    let r = post(
        &st,
        "/calendar/events/2/delete",
        &form(&[("d", "2026-12-02"), ("view", "month")]),
        false,
    )
    .await;
    assert_eq!(r.headers["location"], "/calendar?d=2026-12-02&view=month");
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
        b.contains("<span class=\"t\">10:00</span> <b>oms-sync, weekly</b>")
            && b.contains("<b>Team offsite (moved)</b>")
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

#[tokio::test]
async fn day_and_n_day_views() {
    let st = state();
    // Today is Wed 11 Nov. Day view: one day, ‹ › step a day, the view stays in the URL.
    let b = get(&st, "/calendar?view=day&d=2026-11-13").await.body;
    assert!(b.contains("Fri 13 Nov"));
    assert!(b.contains("href=\"/calendar?d=2026-11-12&#38;view=day\" aria-label=\"Previous day\""));
    // N days without `d`: this week's last N days that still hold today (Tue 10 – Sat 14 for 5).
    let b = get(&st, "/calendar?n=5").await.body;
    assert!(b.contains("10 – 14 Nov"), "{b}");
    assert!(b.contains("href=\"/calendar?d=2026-11-15&#38;n=5\" aria-label=\"Next 5 days\""));
    // Pointed: `d` is the first day.
    let b = get(&st, "/calendar?n=3&d=2026-11-29").await.body;
    assert!(b.contains("29 Nov – 1 Dec") && b.contains("--n: 3"));
    assert!(b.contains("<a href=\"/calendar?d=2026-11-29&#38;n=3\" class=\"on\" aria-current=\"page\">N days</a><a href=\"/calendar?d=2026-11-29&#38;view=week\">Week</a>"));
    // Out-of-range n falls back into 1–7.
    assert!(get(&st, "/calendar?n=40").await.body.contains("8 – 14 Nov"));
}

#[tokio::test]
async fn crowded_cells_show_three_then_more() {
    let st = state();
    exec(
        &st,
        "INSERT INTO entries(day, kind, text, plan_state, due) VALUES
           ('2026-11-09', 'plan', 'a1', 'open', '2026-11-12'), ('2026-11-09', 'plan', 'a2', 'open', '2026-11-12'),
           ('2026-11-09', 'plan', 'a3', 'open', '2026-11-12'), ('2026-11-09', 'plan', 'a4', 'open', '2026-11-12'),
           ('2026-11-09', 'plan', 'b1', 'open', '2026-11-13'), ('2026-11-09', 'plan', 'b2', 'open', '2026-11-13'),
           ('2026-11-09', 'plan', 'b3', 'open', '2026-11-13');",
    );
    // Week: 4 → 2 + "+2 more" (to that Day view); exactly 3 all show.
    let b = get(&st, "/calendar").await.body;
    assert!(
        b.contains("<a class=\"more\" href=\"/calendar?d=2026-11-12&#38;view=day\">+2 more</a>")
    );
    assert!(!b.contains("<b>a3</b>") && b.contains("<b>b3</b>"));
    // The Day view lists them all.
    let b = get(&st, "/calendar?view=day&d=2026-11-12").await.body;
    assert!(b.contains("<b>a4</b>") && !b.contains("more</a>"));
    let b = get(&st, "/calendar?view=month").await.body;
    assert!(b.contains(">+2 more</a>"));
}

#[tokio::test]
async fn events_across_midnight_and_several_days() {
    let st = state();
    exec(
        &st,
        "INSERT INTO events(source, ext_id, title, starts_at, ends_at) VALUES
           ('google', 'a', 'Offsite', '2026-11-09', '2026-11-12'),
           ('google', 'b', 'Night deploy', '2026-11-12T22:00', '2026-11-13T02:00'),
           ('google', 'c', 'Late call', '2026-11-13T23:00', '2026-11-14T00:00'),
           ('google', 'd', 'Long before', '2026-10-01', '2026-11-08');",
    );
    let b = get(&st, "/calendar").await.body;
    // All-day 9–12 Nov (iCal end is exclusive): Mon 9, Tue 10, Wed 11; not on Thu 12.
    assert_eq!(b.matches("<h4>Offsite</h4>").count(), 3, "{b}");
    assert!(b.contains("<h4>Offsite</h4>\n<span class=\"small muted\">Wed 11"));
    assert!(!b.contains("<h4>Offsite</h4>\n<span class=\"small muted\">Thu 12"));
    // Ends (exclusively) on Sun 8: its last day is Sat 7, so not in this week at all.
    assert!(!b.contains("Long before"));
    // 22:00 Thu → 02:00 Fri: clipped to 22:00–24:00 on Thu and 00:00–02:00 on Fri; the grid widens to 0–24.
    assert_eq!(b.matches("<h4>Night deploy</h4>").count(), 2);
    assert!(
        b.contains("top: calc(var(--px) * 1320 / 60); height: calc(var(--px) * 120 / 60 - 2px)")
    );
    assert!(b.contains("top: calc(var(--px) * 0 / 60); height: calc(var(--px) * 120 / 60 - 2px)"));
    assert!(b.contains("<small>22:00 – 02:00</small>"));
    // Ends exactly at midnight: nothing on the next day.
    assert_eq!(b.matches("<h4>Late call</h4>").count(), 1);
    // A week that starts mid-event still shows it.
    let b = get(&st, "/calendar?d=2026-11-13&view=day").await.body;
    assert!(b.contains("<h4>Night deploy</h4>"));
}

#[tokio::test]
async fn tag_filter_keeps_days_auto_and_out_of_range_add_moves() {
    let st = state();
    // N days: the filter form carries n, never view (a view would stop the width pick).
    let b = get(&st, "/calendar?n=5").await.body;
    assert!(b.contains("<input type=\"hidden\" name=\"n\" value=\"5\">"));
    assert!(!b.contains("<input type=\"hidden\" name=\"view\" value=\"days\"><input name=\"add\""));
    let b = get(&st, "/calendar?view=month").await.body;
    assert!(
        b.contains("<input type=\"hidden\" name=\"view\" value=\"month\"><input name=\"add\""),
        "{b}"
    );
    // htmx add outside the shown range: the whole page moves there (title and ‹ › too).
    let r = post(
        &st,
        "/calendar/events",
        &form(&[
            ("title", "offsite"),
            ("date", "2026-12-02"),
            ("view", "week"),
            ("d", "2026-11-11"),
        ]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.headers["hx-location"], "/calendar?d=2026-12-02&view=week");
}

#[test]
fn migration_012_lets_events_come_from_mac_and_keeps_rows() {
    // `events` is unchanged from 001 to 011: an 001 table with rows stands in for an 0.2.3 DB.
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../migrations/001_init.sql"))
        .unwrap();
    conn.execute_batch(
        "INSERT INTO events(id, source, ext_id, title, starts_at) VALUES
           (4, 'own', NULL, 'dentist', '2026-11-11T16:00'), (9, 'google', 'g1', 'sync', '2026-11-12');",
    )
    .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO events(source, title, starts_at) VALUES ('mac', 'x', '2026-11-12')",
            []
        )
        .is_err()
    );
    conn.execute_batch(include_str!("../migrations/012_mac_events.sql"))
        .unwrap();
    let rows: Vec<String> = conn
        .prepare("SELECT id || ':' || source || ':' || title FROM events ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, ["4:own:dentist", "9:google:sync"]);
    conn.execute(
        "INSERT INTO events(source, ext_id, title, starts_at) VALUES ('mac', 'm1', 'Standup', '2026-11-12T09:30')",
        [],
    )
    .unwrap();
    // UNIQUE (source, ext_id) and the start index survive the rebuild.
    assert!(conn
        .execute("INSERT INTO events(source, ext_id, title, starts_at) VALUES ('mac', 'm1', 'again', '2026-11-12')", [])
        .is_err());
    let idx: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'events_start'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(idx, 1);
}

#[tokio::test]
async fn mac_events_look_like_google_ones() {
    let st = state();
    exec(
        &st,
        "INSERT INTO events(source, ext_id, title, starts_at, ends_at) VALUES
           ('mac', 'm1@2026-11-12T09:30', 'Standup', '2026-11-12T09:30', '2026-11-12T09:45')",
    );
    let b = get(&st, "/calendar").await.body;
    assert!(
        b.contains("<button class=\"ev meet\" type=\"button\" popovertarget=\"evc1\""),
        "{b}"
    );
    assert!(b.contains("<h4>Standup</h4>") && b.contains("<small>09:30 – 09:45</small>"));
    // Read-only like Google: its card starts a meeting, no Delete.
    assert!(b.contains("value=\"Standup\"><input type=\"hidden\" name=\"back\" value=\"/\"><button class=\"btn s\" type=\"submit\">Start meeting</button>"));
    assert!(!b.contains("/calendar/events/1/delete"));
}
