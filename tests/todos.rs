mod common;
use common::*;

/// Today = Wed 11 Nov 2026; this week ends Sun 15 Nov.
fn seed(st: &atharu::app::AppState) {
    exec(
        st,
        "INSERT INTO epics(key, title) VALUES ('OPS-101', 'Checkout latency');
         INSERT INTO entries(day, epic_key, kind, text, plan_state, due, link) VALUES
           ('2026-11-02', NULL, 'plan', 'late runbook', 'open', '2026-11-09', NULL),
           ('2026-11-10', 'OPS-101', 'plan', 'ship index', 'open', '2026-11-11', NULL),
           ('2026-11-10', NULL, 'plan', 'reply vendor', 'open', '2026-11-13', 'https://acme.slack.com/archives/C1'),
           ('2026-11-10', NULL, 'plan', 'plan q1', 'open', '2026-12-01', NULL),
           ('2026-11-03', NULL, 'plan', 'renew parking', 'open', NULL, NULL);",
    );
}

#[tokio::test]
async fn groups_by_due_against_fixed_today() {
    let st = state();
    seed(&st);
    let b = get(&st, "/todos").await.body;
    let pos = |s: &str| b.find(s).unwrap_or_else(|| panic!("missing {s}"));
    assert!(pos("<h3>Overdue") < pos("late runbook"));
    assert!(pos("late runbook") < pos("<h3>Today"));
    assert!(pos("<h3>Today") < pos("ship index"));
    assert!(pos("ship index") < pos("<h3>This week"));
    assert!(pos("<h3>This week") < pos("reply vendor"));
    assert!(pos("reply vendor") < pos("<h3>Later"));
    assert!(pos("<h3>Later") < pos("plan q1"));
    assert!(pos("plan q1") < pos("<h3>No date"));
    assert!(pos("<h3>No date") < pos("renew parking"));
    assert!(b.contains("Slack ↗"));
    assert!(b.contains(r#"<span class="due late">due Mon 9 Nov</span>"#));
}

#[tokio::test]
async fn open_plans_carry_over_to_today() {
    let st = state();
    seed(&st);
    // Logged on 2 Nov, still open: shows on today's page without being copied.
    let b = get(&st, "/").await.body;
    assert!(b.contains("Other open plans · 5"));
    assert!(b.contains("renew parking"));
    assert!(b.contains("8 days old"));
    assert!(b.contains("No plans pinned. Pin an open plan to see it here."));
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM entries WHERE kind = 'plan'"),
        5
    );

    // A `>` line logged on Today lands in the plans section out-of-band.
    let r = post(&st, "/entries", &form(&[("line", "> new thing")]), true).await;
    assert!(
        r.body
            .contains(r#"<section id="plans" hx-swap-oob="true">"#)
    );
    assert!(r.body.contains("Other open plans · 6"));
}

#[tokio::test]
async fn pin_cap_is_three_open_pins() {
    let st = state();
    seed(&st);
    for id in 1..=3 {
        let r = post(
            &st,
            &format!("/plans/{id}"),
            &form(&[("pinned", "1")]),
            true,
        )
        .await;
        assert_eq!(r.status, 200);
        assert!(r.body.contains("Unpin"));
    }
    let r = post(&st, "/plans/4", &form(&[("pinned", "1")]), true).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("Top 3 is full. Unpin one first."));
    let r = post(&st, "/plans/4", &form(&[("pinned", "1")]), false).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("Top 3 is full. Unpin one first."));
    assert_eq!(count(&st, "SELECT pinned FROM entries WHERE id = 4"), 0);

    // Finishing a pinned plan frees its slot.
    post(&st, "/plans/1", &form(&[("state", "done")]), true).await;
    let r = post(
        &st,
        "/plans/4",
        &form(&[("pinned", "1"), ("back", "/")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/");
    let b = get(&st, "/").await.body;
    assert!(!b.contains("No plans pinned"));
}

#[tokio::test]
async fn done_and_drop_set_closed_day() {
    let st = state();
    seed(&st);
    let r = post(&st, "/plans/2", &form(&[("state", "done")]), true).await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("done-text"));
    post(&st, "/plans/3", &form(&[("state", "dropped")]), true).await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE closed_day = '2026-11-11' AND plan_state IN ('done', 'dropped')"
        ),
        2
    );
    let b = get(&st, "/todos").await.body;
    assert!(b.contains("Done this week · 1"));
    assert!(!b.contains("reply vendor"));

    // Undo a drop: back to open, closed_day cleared.
    let r = post(&st, "/plans/3", &form(&[("state", "open")]), false).await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/todos");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE id = 3 AND plan_state = 'open' AND closed_day IS NULL"
        ),
        1
    );
}

#[tokio::test]
async fn add_box_and_due_picker() {
    let st = state();
    let r = post(
        &st,
        "/todos",
        &form(&[("line", "OPS-7 renew parking by fri")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains(r#"<section id="todos">"#));
    assert!(r.body.contains("renew parking"));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE kind = 'plan' AND due = '2026-11-13' AND epic_key = 'OPS-7'"
        ),
        1
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM epics WHERE key = 'OPS-7'"),
        1
    );

    let r = post(
        &st,
        "/todos/preview",
        &form(&[("line", "> ask https://github.com/o/r/pull/1 by fri")]),
        false,
    )
    .await;
    assert!(
        r.body
            .contains("→ will save as <b>to-do</b> · link GitHub · due Fri 13 Nov")
    );

    let r = post(
        &st,
        "/plans/1",
        &form(&[("due", ""), ("back", "/todos")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE id = 1 AND due IS NULL"
        ),
        1
    );
    let r = post(&st, "/plans/1", &form(&[("due", "nope")]), true).await;
    assert_eq!(r.status, 422);
    let r = post(&st, "/plans/99", &form(&[("state", "done")]), true).await;
    assert_eq!(r.status, 404);
}

/// A due date or pin change can move a row to another group, so the htmx answer tells the page to re-fetch
/// its list; a refused pin (cap full) changes nothing and must not.
#[tokio::test]
async fn due_or_pin_change_asks_the_page_to_regroup() {
    let st = state();
    seed(&st);
    let r = post(
        &st,
        "/plans/2",
        &form(&[("due", "2026-11-18"), ("back", "/todos")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.headers["hx-trigger"], "plans-changed");
    let r = post(
        &st,
        "/plans/2",
        &form(&[("pinned", "1"), ("back", "/")]),
        true,
    )
    .await;
    assert_eq!(r.headers["hx-trigger"], "plans-changed");
    for id in [1, 3] {
        post(
            &st,
            &format!("/plans/{id}"),
            &form(&[("pinned", "1")]),
            true,
        )
        .await;
    }
    let r = post(&st, "/plans/4", &form(&[("pinned", "1")]), true).await;
    assert_eq!(r.status, 422);
    assert!(r.headers.get("hx-trigger").is_none());
    let b = get(&st, "/todos").await.body;
    assert!(b.contains(r#"hx-trigger="plans-changed from:body, atharu-sync from:body""#));
}
