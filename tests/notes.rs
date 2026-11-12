mod common;
use common::*;

#[tokio::test]
async fn new_note_autosave_and_render() {
    let st = state();
    let b = get(&st, "/notes").await.body;
    assert!(b.contains("No notes yet."));

    let r = post(&st, "/notes", "", false).await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/notes/1");
    assert!(
        get(&st, "/notes/1")
            .await
            .body
            .contains("/static/js/editor.js")
    );
    assert!(
        !get(&st, "/notes").await.body.contains("editor.js"),
        "editor JS only on the note page"
    );

    exec(
        &st,
        "INSERT INTO topics(slug) VALUES ('vector-search');
         INSERT INTO meetings(day, name, ended_at) VALUES ('2026-11-10', 'oms-sync', '2026-11-10 10:00:00');",
    );
    let body = "### What happened\n\np95 on [[OPS-101]] <script>x</script>, see [[vector-search]], [[oms-sync]], [[chekout]].";
    let r = post(
        &st,
        "/notes/1",
        &form(&[("title", "Checkout perf retro"), ("body", body)]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("✓ saved"));
    // Title-only autosave leaves the body alone.
    let r = post(
        &st,
        "/notes/1",
        &form(&[("title", "Checkout retro")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE title = 'Checkout retro' AND body LIKE '%[[OPS-101]]%'"
        ),
        1
    );
    assert_eq!(
        post(&st, "/notes/9", &form(&[("title", "x")]), false)
            .await
            .status,
        404
    );

    let b = get(&st, "/notes/1").await.body;
    assert!(b.contains("<h3>What happened</h3>"));
    assert!(b.contains("<a href=\"/epics/OPS-101\">OPS-101</a>"));
    assert!(b.contains("<a href=\"/learn/vector-search\">vector-search</a>"));
    assert!(b.contains("<a href=\"/meetings?name=oms-sync\">oms-sync</a>"));
    assert!(b.contains("&lt;script&gt;") && !b.contains("<script>x"));
    assert!(
        b.contains(">chekout</span>"),
        "dead link shown plain in Links"
    );
    assert!(b.contains("<span class=\"chip\">you</span>"));

    // Backlinks on the linked pages.
    exec(&st, "INSERT INTO epics(key) VALUES ('OPS-101')");
    assert!(
        get(&st, "/epics/OPS-101")
            .await
            .body
            .contains("Mentioned in 1 note</h3>")
    );
    assert!(
        get(&st, "/learn/vector-search")
            .await
            .body
            .contains("href=\"/notes/1\">Checkout retro</a>")
    );
    assert!(
        get(&st, "/meetings/1")
            .await
            .body
            .contains("Mentioned in 1 note</h3>")
    );
}

#[tokio::test]
async fn search_groups_and_page_picker() {
    let st = state();
    exec(
        &st,
        "INSERT INTO notes(title, body, author, updated_at) VALUES
           ('Checkout perf retro', 'p95 hit 1.8s on [[OPS-101]] after the promo', 'me', '2026-11-11 09:00:00'),
           ('Q4 OKR draft', 'ordering faster', 'me', '2026-11-11 10:00:00'),
           ('Lint · 10 Nov', '[[chekout]] resolves to nothing, did you mean checkout', 'llm', '2026-11-10 09:00:00');
         INSERT INTO epics(key, title) VALUES ('OPS-101', 'Checkout latency');
         INSERT INTO topics(slug) VALUES ('checkout-cache');",
    );
    let b = get(&st, "/notes?q=checkout").await.body;
    assert!(b.contains("2 results"));
    assert!(b.contains("<h3>Mine</h3>") && b.contains("<h3>Written by LLM</h3>"));
    assert!(b.contains("<span class=\"chip llm\">llm</span><b>Lint · 10 Nov</b>"));
    assert!(b.contains("did you mean <b style=\"color: #1d1d1f\">checkout</b>"));
    assert!(
        !b.contains("Q4 OKR draft</b>"),
        "non-matching note left out of results"
    );
    let b = get(&st, "/notes?q=xyz").await.body;
    assert!(b.contains("No notes match ‘xyz’."));
    // Empty search: newest first.
    let b = get(&st, "/notes").await.body;
    assert!(b.find("<b>Q4 OKR draft</b>").unwrap() < b.find("<b>Checkout perf retro</b>").unwrap());

    let r = get(&st, "/api/pages?q=check").await;
    assert_eq!(r.headers["content-type"], "application/json");
    assert_eq!(
        r.body,
        r#"{"data":{"pages":[{"id":"OPS-101","name":"Checkout latency","type":"epic"},{"id":"checkout-cache","name":"","type":"topic"}]}}"#
    );
}
