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

    // `[[OPS-101]]` links only once the epic exists (a dead key stays plain).
    assert!(
        !get(&st, "/notes/1")
            .await
            .body
            .contains("href=\"/epics/OPS-101\"")
    );
    exec(&st, "INSERT INTO epics(key) VALUES ('OPS-101')");
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
    assert!(b.contains("<dt>Written by</dt><dd>you</dd>"));

    // Linked references on the linked pages (`[[X]]` lands the same everywhere). A series' mentions live on the series
    // page (T5), not on each meeting.
    let e = get(&st, "/epics/OPS-101").await.body;
    assert!(
        e.contains("Linked references")
            && e.contains("href=\"/notes/1\"><span>Checkout retro</span></a>"),
        "{e}"
    );
    assert!(e.contains("Mentioned in · 1"));
    assert!(
        get(&st, "/learn/vector-search")
            .await
            .body
            .contains("href=\"/notes/1\"><span>Checkout retro</span></a>")
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
    // One list (D1): the agent's note carries its @chip; All · Mine · Agent narrows it.
    assert!(
        b.contains(
            r#"<span class="chip tag at" title="Written by llm">llm</span><b>Lint · 10 Nov</b>"#
        ),
        "{b}"
    );
    let m = get(&st, "/notes?q=checkout&by=mine").await.body;
    assert!(
        m.contains("1 result") && !m.contains("Lint · 10 Nov</b>"),
        "{m}"
    );
    let a = get(&st, "/notes?by=agent").await.body;
    assert!(a.contains("Lint · 10 Nov</b>") && !a.contains("Q4 OKR draft</b>"));
    assert!(b.contains("did you mean <b class=\"ink\">checkout</b>"));
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

#[tokio::test]
async fn note_links_show_the_title_and_the_target_lists_its_backlinks() {
    let st = state();
    exec(
        &st,
        "INSERT INTO notes(id, title, body, author) VALUES
           (7, 'Checkout [perf] *retro*', 'the target', 'me'),
           (12, 'Follow-up', 'see [[note:7]] and [[note:99]] and [[note:7]]', 'me'),
           (13, 'Loner', 'no links', 'me'),
           (14, '', 'self [[note:14]] and [[note:7]]', 'claude');",
    );
    // A link shows the note's title (escaped, so `[perf]` and `*retro*` stay literal); a dead id is plain text.
    let b = get(&st, "/notes/12").await.body;
    assert!(
        b.contains("<a href=\"/notes/7\">Checkout [perf] *retro*</a>"),
        "{b}"
    );
    assert!(!b.contains("<em>retro</em>"), "the title's * is not markup");
    assert!(b.contains("note:99") && !b.contains("href=\"/notes/99\""));
    // The links prop lists each target once, with the title as its label.
    assert!(b.contains(">Checkout [perf] *retro*</a>"));

    // Note 7: 12 and 14 mention it; both appear once under "Mentioned in", newest edit first, with the agent's badge on 14.
    let t = get(&st, "/notes/7").await.body;
    assert!(t.contains("Linked references"), "{t}");
    assert!(t.contains("<h4>Mentioned in · 2</h4>"), "{t}");
    assert!(t.contains("href=\"/notes/12\"") && t.contains("href=\"/notes/14\""));
    assert!(t.contains("Untitled"), "an untitled note is named");
    // Nobody links to 12 or 13, and 14's link to itself is not a reference: no panel at all.
    for id in [12, 13, 14] {
        let p = get(&st, &format!("/notes/{id}")).await.body;
        assert!(
            !p.contains("Linked references"),
            "note {id} has no backlinks"
        );
    }
}

#[tokio::test]
async fn cli_links_flag_accepts_a_note_number() {
    let st = state();
    exec(
        &st,
        "INSERT INTO notes(id, title, body, author) VALUES (7, 'Target', 'x', 'me')",
    );
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args: Vec<String> = [
        "note",
        "Pointer",
        "--links",
        "note:7,note:99",
        "--as",
        "claude",
    ]
    .map(String::from)
    .to_vec();
    let code = atharu::cli::run(
        &mut st.conn(),
        TODAY.parse().unwrap(),
        &args,
        None,
        &mut "body".as_bytes(),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        v["deadLinks"],
        serde_json::json!(["note:99"]),
        "7 resolves, 99 does not"
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE body LIKE '%Links: [[note:7]] · [[note:99]]'"
        ),
        1
    );
}
