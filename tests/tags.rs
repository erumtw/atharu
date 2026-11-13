//! Tags step 1 (T72, D29/D36): taggings kept in step with the text, inheritance at read time, hub, filters, CLI.
mod common;
use common::*;

fn tags_of(st: &atharu::app::AppState, item_type: &str) -> Vec<String> {
    let conn = st.conn();
    let mut stmt = conn
        .prepare("SELECT tag || ':' || item_id FROM taggings WHERE item_type = ?1 ORDER BY 1")
        .unwrap();
    stmt.query_map([item_type], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn cli(st: &atharu::app::AppState, args: &[&str], stdin: &str) -> (i32, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = atharu::cli::run(
        &mut st.conn(),
        TODAY.parse().unwrap(),
        &args,
        Some("claude"),
        &mut stdin.as_bytes(),
        &mut out,
        &mut err,
    );
    (
        code,
        String::from_utf8(out).unwrap() + &String::from_utf8(err).unwrap(),
    )
}

#[test]
fn migration_006_backfills_existing_text() {
    let path = std::env::temp_dir().join(format!("atharu-tags-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        // A database as it was before tags: migrations 001–005 only, text already holding #words.
        let conn = rusqlite::Connection::open(&path).unwrap();
        for sql in [
            include_str!("../migrations/001_init.sql"),
            include_str!("../migrations/002_author.sql"),
            include_str!("../migrations/003_jira.sql"),
            include_str!("../migrations/004_settings.sql"),
            include_str!("../migrations/005_hardening.sql"),
        ] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 5;
             INSERT INTO entries(id, day, kind, text) VALUES (1, '2026-11-10', 'done', 'booked #Expense review'),
                                                         (2, '2026-11-10', 'done', 'see https://x.io/#frag, issue #12');
             INSERT INTO notes(id, title, body) VALUES (1, 'n', '# Heading\n`#code` and #hiring');",
        )
        .unwrap();
    }
    let conn = atharu::db::open(&path).unwrap();
    let rows: Vec<String> = conn
        .prepare("SELECT tag || ':' || item_type || ':' || item_id FROM taggings ORDER BY 1")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, ["expense:entry:1", "hiring:note:1"]);
    let v: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    // Latest migration: bump with each new one.
    assert_eq!(v, 12);
    // No meeting_series table under D36 option A: series tags live in taggings only.
    let series: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'meeting_series'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(series, 0);
    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn saves_rebuild_taggings_and_deletes_clear_them() {
    let st = state();
    post(
        &st,
        "/entries",
        &form(&[("line", "booked #Expense e2e #hiring")]),
        false,
    )
    .await;
    assert_eq!(tags_of(&st, "entry"), ["expense:1", "hiring:1"]);
    // Inline edit rewrites the set, same transaction as the text.
    post(
        &st,
        "/entries/1",
        &form(&[("line", "booked e2e #hiring")]),
        false,
    )
    .await;
    assert_eq!(tags_of(&st, "entry"), ["hiring:1"]);
    let r = post(&st, "/entries/1/delete", "", true).await;
    assert!(tags_of(&st, "entry").is_empty());
    let at = r.body.find("name=\"snap\" value=\"").unwrap() + 19;
    let snap = r.body[at..at + r.body[at..].find('"').unwrap()]
        .replace("&#34;", "\"")
        .replace("&#39;", "'");
    post(&st, "/entries/restore", &form(&[("snap", &snap)]), false).await;
    assert_eq!(tags_of(&st, "entry"), ["hiring:1"]);
    // Notes: web body save, then delete.
    post(&st, "/notes", "", false).await;
    post(
        &st,
        "/notes/1",
        &form(&[("body", "## Plan #oncall\n```\n#nope\n```")]),
        false,
    )
    .await;
    assert_eq!(tags_of(&st, "note"), ["oncall:1"]);
    post(&st, "/notes/1/delete", "", false).await;
    assert!(tags_of(&st, "note").is_empty());
}

#[tokio::test]
async fn epic_tags_pass_down_at_read_time() {
    let st = state();
    post(
        &st,
        "/entries",
        &form(&[("line", "OPS-1 ! approve claims by hand")]),
        false,
    )
    .await;
    post(
        &st,
        "/entries",
        &form(&[("line", "OPS-1 > fix receipt upload")]),
        false,
    )
    .await;
    post(
        &st,
        "/entries",
        &form(&[("line", "paired on the #expense export bug")]),
        false,
    )
    .await;
    post(
        &st,
        "/entries",
        &form(&[("line", "deployed index migration")]),
        false,
    )
    .await;
    let r = request(
        &st,
        "PATCH",
        "/epics/OPS-1",
        &form(&[("tags", "#Expense, hiring, bad_one")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE tags = 'expense,hiring'"
        ),
        1
    );
    assert_eq!(tags_of(&st, "epic"), ["expense:OPS-1", "hiring:OPS-1"]);
    // Nothing is written to the lines themselves.
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM taggings WHERE item_type = 'entry'"
        ),
        1
    );

    let hub = get(&st, "/tags/expense").await;
    assert_eq!(hub.status, 200);
    let b = &hub.body;
    assert!(
        b.contains("3 lines · 1 open to-do · last used today"),
        "{b}"
    );
    assert!(b.contains("Open to-dos"));
    assert!(b.contains("fix receipt upload"));
    assert!(b.contains(r#"<div class="hi blocks">"#));
    assert!(b.contains("approve claims by hand"));
    assert!(b.contains("via <a href=\"/epics/OPS-1\">"));
    assert!(!b.contains("deployed index migration"));
    assert!(b.contains(r#"<a class="chip tag" href="/tags/expense">expense</a>"#));
    assert!(b.contains("tags it, 2 lines pass down"));
    // The epic timeline draws the inherited tag as a dashed chip.
    let ep = get(&st, "/epics/OPS-1").await.body;
    assert!(ep.contains(r#"class="chip tag inh" href="/tags/expense""#));
    // Untag the epic: its lines stop counting at once.
    request(&st, "PATCH", "/epics/OPS-1", &form(&[("tags", "")]), true).await;
    assert!(
        get(&st, "/tags/expense")
            .await
            .body
            .contains("1 line · last used today")
    );
    assert_eq!(get(&st, "/tags/Expense").await.status, 404);
}

#[tokio::test]
async fn filter_and_group_by_tag() {
    let st = state();
    post(
        &st,
        "/entries",
        &form(&[("line", "> send #expense policy draft")]),
        false,
    )
    .await;
    post(
        &st,
        "/entries",
        &form(&[("line", "OPS-1 > fix receipt upload")]),
        false,
    )
    .await;
    post(&st, "/entries", &form(&[("line", "> book dentist")]), false).await;
    post(
        &st,
        "/entries",
        &form(&[("line", "reviewed #expense and #hiring notes")]),
        false,
    )
    .await;
    request(
        &st,
        "PATCH",
        "/epics/OPS-1",
        &form(&[("tags", "expense")]),
        true,
    )
    .await;

    let td = get(&st, "/todos?tag=expense").await.body;
    assert!(td.contains("send"), "{td}");
    assert!(td.contains("fix receipt upload"));
    assert!(td.contains("via <a href=\"/epics/OPS-1\">"));
    assert!(!td.contains("book dentist"));
    assert!(td.contains("2 of 3 open to-dos"));
    assert!(td.contains(r#"href="/todos" aria-label="Remove filter #expense""#));
    // The add box merges into the filter (any of them).
    let td = get(&st, "/todos?tag=hiring&add=%23Expense").await.body;
    assert!(td.contains("aria-label=\"Remove filter #hiring\""));
    assert!(td.contains("fix receipt upload"));

    let today = get(&st, "/?tag=hiring").await.body;
    assert!(today.contains("reviewed"));
    // The log is filtered; the open plans above it are not (their text is an edit link too, D51).
    let log = &today[today.find("id=\"log\"").unwrap()..];
    assert!(!log.contains("book dentist</a>"));
    // Grouped: a line with two tags shows under both; untagged lines under "No tag".
    let g = get(&st, "/?group=tag").await.body;
    assert_eq!(g.matches("reviewed ").count(), 2, "{g}");
    assert!(g.contains("No tag"));
    // htmx swaps keep the page's grouping (HX-Current-URL).
    let req = axum::http::Request::post("/entries")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("hx-current-url", "http://127.0.0.1:7070/?group=tag")
        .body(axum::body::Body::from(form(&[("line", "more #hiring")])))
        .unwrap();
    let r = send(atharu::app::router(st.clone()), req).await;
    assert!(r.body.contains("No tag"));
    assert!(
        r.body
            .contains("tagged <span class=\"chip tag\">hiring</span>")
    );
}

#[tokio::test]
async fn capture_bar_suggests_and_offers() {
    let st = state();
    post(
        &st,
        "/entries",
        &form(&[("line", "booked #expense review")]),
        false,
    )
    .await;
    post(&st, "/entries", &form(&[("line", "more #expense")]), false).await;
    post(&st, "/entries", &form(&[("line", "fixed #exports")]), false).await;
    let r = post(
        &st,
        "/parse-preview",
        &form(&[("line", "OPS-1 booked #ex")]),
        true,
    )
    .await;
    let b = &r.body;
    let (e, x) = (
        b.find("data-tag=\"expense\"").unwrap(),
        b.find("data-tag=\"exports\"").unwrap(),
    );
    assert!(e < x, "most used first: {b}");
    assert!(b.contains("2 lines"));
    assert!(b.contains("New tag <b>#ex</b>"));
    let r = post(
        &st,
        "/parse-preview",
        &form(&[("line", "paid Expenses today")]),
        true,
    )
    .await;
    assert!(
        r.body
            .contains(r#"data-word="Expenses" data-tag="expense""#),
        "{}",
        r.body
    );
    let r = post(
        &st,
        "/parse-preview",
        &form(&[("line", "paid Expenses"), ("notag", "Expenses")]),
        true,
    )
    .await;
    assert!(!r.body.contains("tagask"));
}

#[tokio::test]
async fn cli_tags_from_text_and_lists_them() {
    let st = state();
    let (code, out) = cli(
        &st,
        &["log", "OPS-2 shipped #Expense export", "--as", "claude"],
        "",
    );
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["tags"], serde_json::json!(["expense"]));
    let (code, _) = cli(
        &st,
        &["note", "Finance 1:1", "--as", "claude"],
        "limits #expense #hiring",
    );
    assert_eq!(code, 0);
    let (code, out) = cli(
        &st,
        &["epic", "OPS-3", "--tags", "#Oncall, x_y", "--as", "claude"],
        "",
    );
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["tags"], serde_json::json!(["oncall"]));
    cli(&st, &["log", "OPS-3 paged at 3am", "--as", "claude"], "");
    let (code, out) = cli(&st, &["tags"], "");
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["tags"][0]["tag"], "expense");
    assert_eq!(v["tags"][0]["count"], 2);
    // The note's edit day comes from the real clock, the line's from the pinned today: the later wins.
    assert!(v["tags"][0]["lastUsed"].as_str().unwrap() >= TODAY);
    let names: Vec<&str> = v["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tag"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["expense", "hiring", "oncall"]);
    assert_eq!(v["tags"][2]["count"], 1, "inherited line counts");
    // An agent never overwrites tags I set.
    cli(
        &st,
        &["epic", "OPS-3", "--tags", "other", "--as", "claude"],
        "",
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key = 'OPS-3' AND tags = 'oncall'"
        ),
        1
    );
    let list = get(&st, "/tags").await.body;
    assert!(list.contains(r#"<a class="chip tag" href="/tags/expense">expense</a>"#));
    assert!(list.contains("3 tags."));
}

#[tokio::test]
async fn every_list_draws_a_todo_with_the_one_plan_row() {
    // The tag hub and the epic timeline used to draw their own to-do markup, while Pin / Drop / Undo answer
    // `plan_row.html`: a swapped row then looked unlike its neighbours. Now every list draws plan_row.html.
    let st = state();
    post(
        &st,
        "/entries",
        &form(&[("line", "OPS-1 > ship it #infra")]),
        false,
    )
    .await;
    post(
        &st,
        "/entries",
        &form(&[("line", "OPS-1 ! no queue #infra")]),
        false,
    )
    .await;
    for page in ["/tags/infra", "/epics/OPS-1", "/todos"] {
        let b = get(&st, page).await.body;
        assert!(
            b.contains(r#"hx-post="/plans/1" hx-trigger="change""#),
            "{page}"
        );
        assert!(b.contains("Pin to Focus on Today (top 3)"), "{page}");
    }
    // Other lines draw read-only through entry_row.html everywhere but Today.
    let hub = get(&st, "/tags/infra").await.body;
    assert!(
        hub.contains(r#"<span class="ltext">no queue <a class="chip tag" href="/tags/infra">"#)
    );
    assert!(!hub.contains(r#"id="entry-2""#));
    assert!(get(&st, "/").await.body.contains(r#"id="entry-2""#));
}
