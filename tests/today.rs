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
async fn delete_removes_and_offers_undo() {
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
    assert!(
        r.body.contains(r#"action="/entries/restore""#),
        "{}",
        r.body
    );
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
        &form(&[("line", "> x by 32 Nov")]),
        true,
    )
    .await;
    assert!(
        r.body.contains("Can&#39;t read date") || r.body.contains("Can't read date"),
        "{}",
        r.body
    );
    // D44: words after `by` that aren't a date save as text, with a soft note instead of an error.
    let r = post(
        &st,
        "/parse-preview",
        &form(&[("line", "> refactor by hand")]),
        true,
    )
    .await;
    assert!(
        r.body.contains("“refactor by hand”")
            && r.body.contains("‘by hand’ isn't a date · saved as text")
            && !r.body.contains("class=\"err\""),
        "{}",
        r.body
    );
    // Blank box: blank hint; the (empty) `#` menu still comes along so a stale one closes.
    let r = post(&st, "/parse-preview", &form(&[("line", "  ")]), true).await;
    assert_eq!(
        r.body.trim(),
        r#"<div id="tagsug" hx-swap-oob="innerHTML"></div>"#
    );
}

/// The Undo form's hidden snapshot, unescaped the way a browser would post it back.
fn snap(body: &str) -> String {
    let at = body.find("name=\"snap\" value=\"").expect("undo form") + 19;
    let raw = &body[at..at + body[at..].find('"').unwrap()];
    raw.replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&#38;", "&")
}

#[tokio::test]
async fn undo_restores_the_exact_row() {
    let st = state();
    exec(
        &st,
        "INSERT INTO epics(key) VALUES ('OPS-7');
         INSERT INTO meetings(id, day, name, ended_at) VALUES (3, '2026-11-09', 'oms-sync', '2026-11-09 10:00:00');
         INSERT INTO meetings(id, day, name) VALUES (4, '2026-11-11', 'now-running');
         INSERT INTO entries(id, day, epic_key, meeting_id, kind, text, plan_state, closed_day, pinned, due, author)
           VALUES (42, '2026-11-09', 'OPS-7', 3, 'plan', 'ship it', 'done', '2026-11-10', 1, '2026-11-12', 'claude');",
    );
    let row = "SELECT json_array(id, day, epic_key, meeting_id, kind, topic, text, plan_state,
                 closed_day, pinned, due, link, created_at, author) FROM entries";
    let before: String = st.conn().query_row(row, [], |r| r.get(0)).unwrap();

    let r = request(&st, "DELETE", "/entries/42", "", true).await;
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 0);
    let r = post(
        &st,
        "/entries/restore",
        &form(&[("snap", &snap(&r.body))]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    let after: String = st.conn().query_row(row, [], |r| r.get(0)).unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn editing_a_missing_line_leaves_no_stub_epic() {
    let st = state();
    let r = post(&st, "/entries/99", &form(&[("line", "NEW-1 ghost")]), true).await;
    assert_eq!(r.status, 404);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM epics"), 0);
}

#[tokio::test]
async fn group_by_type_puts_decisions_first_and_keeps_the_epic() {
    let st = state();
    for line in [
        "OPS-1 shipped it",
        "OPS-1 ! keep the queue",
        "> call vendor",
        "x waiting on DBA",
    ] {
        post(&st, "/entries", &form(&[("line", line)]), false).await;
    }
    let b = get(&st, "/?group=type").await.body;
    let at = |s: &str| b.find(s).unwrap_or_else(|| panic!("missing {s}"));
    assert!(at("<h4>Decisions <span") < at("<h4>Blockers <span"));
    assert!(at("<h4>Blockers <span") < at("<h4>To-dos <span"));
    assert!(at("<h4>To-dos <span") < at("<h4>Done <span"));
    assert!(b.contains("aria-pressed=\"true\" type=\"submit\" name=\"group\" value=\"type\""));
}
