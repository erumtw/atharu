//! T84 line notes (D49) and D51 to-do rows that edit and delete like log lines.
mod common;
use common::*;

/// The first Undo form's hidden snapshot, unescaped the way a browser would post it back.
fn snap(body: &str) -> String {
    let at = body.find("name=\"snap\" value=\"").expect("undo form") + 19;
    let raw = &body[at..at + body[at..].find('"').unwrap()];
    raw.replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&#60;", "<")
        .replace("&#62;", ">")
        .replace("&#38;", "&")
}

fn note_of(st: &atharu::app::AppState, line: i64) -> Option<i64> {
    st.conn()
        .query_row("SELECT note_id FROM entries WHERE id = ?1", [line], |r| {
            r.get(0)
        })
        .unwrap()
}

#[tokio::test]
async fn first_click_makes_the_note_and_links_survive_deletes() {
    let st = state();
    exec(
        &st,
        "INSERT INTO entries(id, day, epic_key, kind, text) VALUES
           (1, '2026-11-10', NULL, 'decision', 'keep 2 h windows for #hotel accounts'),
           (2, '2026-11-11', NULL, 'blocker', 'waiting on the vendor audit');
         INSERT INTO epics(key) VALUES ('OPS-140');
         UPDATE entries SET epic_key = 'OPS-140' WHERE id = 1;",
    );
    // Today: no note yet = the add form; it is the row's first right-hand control.
    let today = get(&st, "/?day=2026-11-10").await.body;
    assert!(today.contains(
        r#"<form class="inline" method="post" action="/entries/1/note"><button class="ib note""#
    ));
    assert!(today.contains(">Add a note</button>"), "phone sheet item");

    // First click (plain form): note titled after the line, author me, linked, 303 to it.
    let r = post(&st, "/entries/1/note", "", false).await;
    assert_eq!(r.status, 303);
    let n = note_of(&st, 1).expect("linked");
    assert_eq!(r.headers["location"], format!("/notes/{n}").as_str());
    assert_eq!(
        count(
            &st,
            &format!(
                "SELECT COUNT(*) FROM notes WHERE id = {n} AND title = 'keep 2 h windows for #hotel accounts' AND author = 'me'"
            )
        ),
        1
    );
    // Second click opens the same note (htmx answers HX-Redirect); nothing new is made.
    let r = post(&st, "/entries/1/note", "", true).await;
    assert_eq!(r.headers["hx-redirect"], format!("/notes/{n}").as_str());
    assert_eq!(count(&st, "SELECT COUNT(*) FROM notes"), 1);
    assert_eq!(post(&st, "/entries/99/note", "", false).await.status, 404);

    // The row now shows the filled link; the sheet says "Open note".
    let today = get(&st, "/?day=2026-11-10").await.body;
    assert!(today.contains(&format!(
        r#"<a class="ib note has" href="/notes/{n}" aria-label="Open note: keep 2 h windows"#
    )));
    assert!(today.contains(&format!(
        r#"<a class="btn ghost s m-only" href="/notes/{n}">Open note</a>"#
    )));

    // Many lines → one note.
    exec(
        &st,
        &format!("UPDATE entries SET note_id = {n} WHERE id = 2"),
    );
    let page = get(&st, &format!("/notes/{n}")).await.body;
    assert!(page.contains(r#"<h2 id="lnh-h">Lines · 2</h2>"#), "{page}");
    assert!(page.contains(r#"<a class="chip tag" href="/tags/hotel">hotel</a>"#));
    assert!(page.contains(r#"<span class="chip key">OPS-140</span>"#));
    assert!(page.contains(r#"<a class="day" href="/?day=2026-11-11">logged today</a>"#));
    assert!(page.contains(r#"<a class="day" href="/?day=2026-11-10">logged Tue 10 Nov</a>"#));
    // The title input feeds #note from outside it (forms can't nest around the header's own forms).
    assert!(page.contains(r#"id="note-title" form="note" name="title""#));

    // Deleting a line keeps the note; the header offers its Undo; Undo restores line + link.
    let r = request(&st, "DELETE", "/entries/2", "", true).await;
    assert_eq!(r.status, 200);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM notes"), 1);
    let head = get(&st, &format!("/notes/{n}/lines")).await.body;
    assert!(
        head.contains("Line deleted · “waiting on the vendor audit”"),
        "{head}"
    );
    assert!(head.contains(&format!(r#"name="back" value="/notes/{n}""#)));
    let r = post(
        &st,
        "/entries/restore",
        &form(&[("snap", &snap(&head)), ("back", &format!("/notes/{n}"))]),
        false,
    )
    .await;
    assert_eq!(r.headers["location"], format!("/notes/{n}").as_str());
    assert_eq!(note_of(&st, 2), Some(n));
    let head = get(&st, &format!("/notes/{n}/lines")).await.body;
    assert!(!head.contains("Line deleted"), "{head}");

    // Deleting the note clears every link; its Undo links them again.
    let r = post(&st, &format!("/notes/{n}/delete"), "", true).await;
    assert_eq!(note_of(&st, 1), None);
    assert_eq!(note_of(&st, 2), None);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 2);
    post(
        &st,
        "/notes/restore",
        &form(&[("snap", &snap(&r.body))]),
        true,
    )
    .await;
    assert_eq!(note_of(&st, 1), Some(n));
    assert_eq!(note_of(&st, 2), Some(n));
}

#[tokio::test]
async fn header_checks_and_dates_todos_and_folds_past_five() {
    let st = state();
    exec(
        &st,
        "INSERT INTO notes(id, title) VALUES (7, 'Cold-chain SLA'), (8, 'Plain note');
         INSERT INTO entries(id, day, kind, text, plan_state, due, note_id) VALUES
           (1, '2026-11-09', 'plan', 'reply to vendor', 'open', '2026-11-13', 7);",
    );
    let page = get(&st, "/notes/7").await.body;
    assert!(page.contains(r#"<h2 id="lnh-h">Line</h2>"#));
    assert!(page.contains(
        r#"hx-get="/notes/7/lines" hx-trigger="plans-changed from:body, atharu-sync from:body""#
    ));
    assert!(page.contains(r#"aria-label="Mark done""#));
    assert!(page.contains(r#"name="due" value="2026-11-13""#));
    // An unlinked note has no header at all.
    assert!(!get(&st, "/notes/8").await.body.contains("lnh"));

    // Tick from the header: the plan action answers plans-changed, the header re-fetch shows it done.
    let r = post(
        &st,
        "/plans/1",
        &form(&[("state", "done"), ("back", "/notes/7")]),
        true,
    )
    .await;
    assert_eq!(r.headers["hx-trigger"], "plans-changed");
    let head = get(&st, "/notes/7/lines").await.body;
    assert!(
        head.contains(r#"<span class="when">done today</span>"#),
        "{head}"
    );
    assert!(head.contains("ltext done-text"));
    assert!(head.contains(r#"aria-label="Reopen""#));
    // Plain POST from the header comes back to the note.
    let r = post(
        &st,
        "/plans/1",
        &form(&[("state", "open"), ("back", "/notes/7")]),
        false,
    )
    .await;
    assert_eq!(r.headers["location"], "/notes/7");
    post(
        &st,
        "/plans/1",
        &form(&[("due", "2026-11-20"), ("back", "/notes/7")]),
        true,
    )
    .await;
    assert!(
        get(&st, "/notes/7/lines")
            .await
            .body
            .contains(r#"value="2026-11-20""#)
    );

    // More than five: the newest five, the rest folded.
    for i in 2..=8 {
        exec(
            &st,
            &format!(
                "INSERT INTO entries(id, day, kind, text, note_id) VALUES ({i}, '2026-11-1{}', 'done', 'step {i}', 7)",
                i % 2
            ),
        );
    }
    let head = get(&st, "/notes/7/lines").await.body;
    assert!(head.contains("Lines · 8"));
    assert!(head.contains("3 more lines</summary>"), "{head}");
}

#[tokio::test]
async fn notes_list_badges_and_linked_filter() {
    let st = state();
    exec(
        &st,
        "INSERT INTO notes(id, title) VALUES (1, 'Vendor SLA'), (2, 'Retention plan');
         INSERT INTO entries(day, kind, text, plan_state, note_id) VALUES
           ('2026-11-09', 'plan', 'reply', 'open', 1), ('2026-11-10', 'decision', 'keep', NULL, 1);",
    );
    let all = get(&st, "/notes").await.body;
    assert!(all.contains(r#"<span class="k" title="Linked to a to-do line">&#62;</span><b>Vendor SLA</b><span class="small muted">2 lines</span>"#), "{all}");
    assert!(all.contains("Retention plan"));
    assert!(all.contains(r#"<a href="/notes?linked=1" aria-pressed="false"><span class="chip">Linked to lines</span></a>"#));
    let linked = get(&st, "/notes?linked=1").await.body;
    assert!(linked.contains("Vendor SLA"));
    assert!(!linked.contains("Retention plan</b>"));
    assert!(linked.contains(r#"<span class="chip fl">Linked to lines<a class="rm" href="/notes" aria-label="Show all notes">×</a></span>"#));
    // The tag filter keeps the linked filter.
    assert!(linked.contains(r#"<input type="hidden" name="linked" value="1">"#));
}

#[tokio::test]
async fn cli_links_and_nests_the_note() {
    let st = state();
    exec(
        &st,
        "INSERT INTO notes(id, title) VALUES (5, 'DBA window');",
    );
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let run = |a: &[&str]| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = atharu::cli::run(
            &mut st.conn(),
            TODAY.parse().unwrap(),
            &args(a),
            Some("claude"),
            &mut "".as_bytes(),
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    };
    let (code, out, _) = run(&["log", "OPS-1 x waiting on DBA", "--note", "5"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["note"],
        serde_json::json!({"id": 5, "title": "DBA window"})
    );
    let (code, _, err) = run(&["log", "something", "--note", "99"]);
    assert_eq!(code, 1);
    assert!(err.contains("No note 99"), "{err}");
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM entries"),
        1,
        "a bad --note files nothing"
    );

    let (_, out, _) = run(&["today"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["entries"][0]["note"]["id"], 5);
    let (_, out, _) = run(&["show", "OPS-1"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["timeline"][0]["note"]["title"], "DBA window");
}

#[tokio::test]
async fn todo_rows_edit_and_delete_like_log_lines() {
    let st = state();
    exec(
        &st,
        "INSERT INTO entries(id, day, kind, text, plan_state) VALUES (1, '2026-11-10', 'plan', 'ask for the #expense CSV', 'open');",
    );
    let page = get(&st, "/todos").await.body;
    // Done only from the check: the text is no longer inside the check's label.
    assert!(
        !page.contains(r#"<label class="row" style="gap: 10px"><input class="check""#),
        "{page}"
    );
    assert!(page.contains(r#"aria-label="Mark done">"#));
    assert!(page.contains(r##"<a class="edit-line" href="/?day=2026-11-10&#38;edit=1" hx-get="/entries/1/edit?plan=1" hx-target="#plan-1""##), "{page}");
    assert!(page.contains(r#"<a class="chip tag" href="/tags/expense">expense</a>"#));
    assert!(page.contains(r#"hx-delete="/entries/1?plan=1""#));

    // Edit: the box stands in for the row; saving answers plans-changed (the list regroups itself).
    let ed = get(&st, "/entries/1/edit?plan=1").await.body;
    assert!(ed.contains(r#"id="plan-1""#) && ed.contains(r#"name="plan" value="1""#));
    let r = post(
        &st,
        "/entries/1",
        &form(&[("line", "> ask Nok for the CSV by fri"), ("plan", "1")]),
        true,
    )
    .await;
    assert_eq!(r.headers["hx-trigger"], "plans-changed");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE id = 1 AND text = 'ask Nok for the CSV' AND due = '2026-11-13' AND plan_state = 'open'"
        ),
        1
    );
    // A parse error keeps the box, retargeted at the plan row.
    let r = post(
        &st,
        "/entries/1",
        &form(&[("line", "+"), ("plan", "1")]),
        true,
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.headers["hx-retarget"], "#plan-1");

    // Delete: the row becomes its own Undo; Undo brings it back and re-fetches the lists.
    let r = request(&st, "DELETE", "/entries/1?plan=1", "", true).await;
    assert!(
        r.body.contains(r#"<div class="blk" id="plan-1">"#) && r.body.contains("Line deleted"),
        "{}",
        r.body
    );
    assert!(r.headers.get("hx-trigger").is_none());
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 0);
    let r = post(
        &st,
        "/entries/restore",
        &form(&[("snap", &snap(&r.body)), ("plan", "1")]),
        true,
    )
    .await;
    assert_eq!(r.headers["hx-trigger"], "plans-changed");
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries WHERE id = 1"), 1);
    // Plain POST delete goes back to the page it came from.
    let r = post(&st, "/entries/1/delete?back=%2Ftodos", "", false).await;
    assert_eq!(r.headers["location"], "/todos");
}
