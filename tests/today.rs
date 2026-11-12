mod common;
use common::*;

#[tokio::test]
async fn log_line_shows_row_and_toast() {
    let st = state();
    exec(
        &st,
        "INSERT INTO epics(key, title) VALUES ('OPS-101', 'Checkout latency')",
    );
    let r = post(
        &st,
        "/entries",
        &form(&[("line", "OPS-101 + p95 down"), ("day", TODAY)]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains(r#"<section id="log">"#));
    assert!(r.body.contains("p95 down"));
    assert!(r.body.contains("Checkout latency"));
    assert!(r.body.contains("Saved as <b>impact</b> on"));
    assert!(r.body.contains(r#"hx-swap-oob="true""#));

    let page = get(&st, "/").await;
    assert!(page.body.contains("p95 down"));
    assert!(!page.body.contains("Nothing logged yet"));
}

#[tokio::test]
async fn plain_post_redirects_back_to_the_day() {
    let st = state();
    let r = post(
        &st,
        "/entries",
        &form(&[("line", "reviewed PRs"), ("day", "2026-11-10")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/?day=2026-11-10");
    let page = get(&st, "/?day=2026-11-10").await;
    assert!(page.body.contains("reviewed PRs"));
    assert!(page.body.contains("Not filed under an epic"));
    assert!(page.body.contains("Logging for Tue 10 Nov"));
}

#[tokio::test]
async fn parse_error_is_422_and_keeps_input() {
    let st = state();
    let r = post(&st, "/entries", &form(&[("line", "OPS-101 +")]), false).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains(r#"value="OPS-101 +""#));
    assert!(r.body.contains("Nothing after “+”. Type what happened."));

    let r = post(&st, "/entries", &form(&[("line", "OPS-101 +")]), true).await;
    assert_eq!(r.status, 422);
    assert_eq!(r.headers["hx-retarget"], "#hint");
    assert!(r.body.contains("Nothing after"));
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 0);
}

#[tokio::test]
async fn delete_removes_and_offers_relog() {
    let st = state();
    post(
        &st,
        "/entries",
        &form(&[("line", "> ship it by fri")]),
        false,
    )
    .await;
    let r = request(&st, "DELETE", "/entries/1", "", true).await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("Line deleted"));
    assert!(r.body.contains("ship it by 2026-11-13\""), "{}", r.body);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 0);

    post(&st, "/entries", &form(&[("line", "again")]), false).await;
    let id = count(&st, "SELECT id FROM entries");
    let r = post(&st, &format!("/entries/{id}/delete"), "", false).await;
    assert_eq!(r.status, 303);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 0);
    assert_eq!(
        request(&st, "DELETE", "/entries/99", "", true).await.status,
        404
    );
}

#[tokio::test]
async fn unknown_key_creates_stub_epic() {
    let st = state();
    let r = post(
        &st,
        "/entries",
        &form(&[("line", "OPS-140 x waiting on DB")]),
        true,
    )
    .await;
    assert!(r.body.contains("Saved on new epic"));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key = 'OPS-140' AND title = ''"
        ),
        1
    );
    let page = get(&st, "/").await;
    assert!(page.body.contains("no title yet"));
    assert!(page.body.contains(r#"<option value="OPS-140">"#));
}

#[tokio::test]
async fn preview_returns_kind_and_key() {
    let st = state();
    let r = post(
        &st,
        "/parse-preview",
        &form(&[("line", "OPS-101 ! keep sync")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("→ will save as <b>decision</b> on OPS-101"));
    let r = post(
        &st,
        "/parse-preview",
        &form(&[("line", "> x by someday")]),
        true,
    )
    .await;
    assert!(
        r.body.contains("Can&#39;t read date") || r.body.contains("Can't read date"),
        "{}",
        r.body
    );
    let r = post(&st, "/parse-preview", &form(&[("line", "  ")]), true).await;
    assert_eq!(r.body, "");
}
