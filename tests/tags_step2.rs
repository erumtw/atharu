//! Tags step 2 (T73, D29/D36): series / topic / event tags, one-name hub, [[X]] → hub, rename · merge ·
//! delete with Undo, Make tag + "Tag them", report grouping, filters, chip fields.
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

fn text(st: &atharu::app::AppState, id: i64) -> String {
    st.conn()
        .query_row("SELECT text FROM entries WHERE id = ?1", [id], |r| r.get(0))
        .unwrap()
}

/// The value of the first hidden `snap` field in a response, HTML-unescaped.
fn snap(body: &str) -> String {
    let at = body.find("name=\"snap\" value=\"").unwrap() + 19;
    body[at..at + body[at..].find('"').unwrap()]
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

async fn log(st: &atharu::app::AppState, line: &str) {
    let r = post(st, "/entries", &form(&[("line", line)]), false).await;
    assert_eq!(r.status, 303, "{line}");
}

#[tokio::test]
async fn series_topic_and_event_tags() {
    let st = state();
    // A line logged during a meeting inherits the series' tags at read time.
    post(&st, "/meetings", &form(&[("name", "expense-sync")]), false).await;
    log(&st, "approve claims over 5k by hand").await;
    post(&st, "/meetings/1/end", &form(&[("back", "/")]), false).await;
    let r = post(
        &st,
        "/meetings/1/tags",
        &form(&[("tags", ""), ("tags_add", "#Expense, hiring")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains(r#"id="series-tags-form""#), "{}", r.body);
    assert_eq!(
        tags_of(&st, "series"),
        ["expense:expense-sync", "hiring:expense-sync"]
    );
    assert_eq!(tags_of(&st, "entry"), Vec::<String>::new());
    let r = post(
        &st,
        "/meetings/1/tags",
        &form(&[("tags", "expense,hiring"), ("tags_drop", "hiring")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(tags_of(&st, "series"), ["expense:expense-sync"]);
    assert!(
        get(&st, "/meetings/1")
            .await
            .body
            .contains(r#"<a href="/tags/expense">expense</a>"#)
    );

    // Topic Tags prop; delete clears it, Undo brings it back.
    exec(&st, "INSERT INTO topics(slug) VALUES ('vat-rules')");
    let r = post(
        &st,
        "/learn/vat-rules/tags",
        &form(&[("tags", ""), ("tags_add", "expense")]),
        true,
    )
    .await;
    assert!(r.body.contains(r#"id="topic-tags-form""#));
    assert_eq!(tags_of(&st, "topic"), ["expense:vat-rules"]);
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM topics WHERE tags = 'expense'"),
        1
    );
    let r = post(&st, "/learn/vat-rules/delete", "", true).await;
    assert!(tags_of(&st, "topic").is_empty());
    post(
        &st,
        "/learn/vat-rules/restore",
        &form(&[("snap", &snap(&r.body))]),
        false,
    )
    .await;
    assert_eq!(tags_of(&st, "topic"), ["expense:vat-rules"]);

    // Own events take #tags from the title; Google ones are never stored, only matched by word.
    post(
        &st,
        "/calendar/events",
        &form(&[
            ("title", "Expense e2e demo #expense"),
            ("date", "2026-11-13"),
            ("time", "14:00"),
        ]),
        false,
    )
    .await;
    post(
        &st,
        "/calendar/events",
        &form(&[("title", "dentist"), ("date", "2026-11-13")]),
        false,
    )
    .await;
    exec(
        &st,
        "INSERT INTO events(source, ext_id, title, starts_at) VALUES
           ('google', 'g1', 'Expenses sync with finance', '2026-11-16T10:00'),
           ('google', 'g2', 'Hiring loop', '2026-11-16T11:00')",
    );
    assert_eq!(tags_of(&st, "event"), ["expense:1"]);

    let hub = get(&st, "/tags/expense").await.body;
    assert!(
        hub.contains(r#"via <span class="chip" title="expense-sync">expense-sync</span>"#),
        "{hub}"
    );
    assert!(hub.contains("approve claims over 5k by hand"));
    assert!(hub.contains(
        r#"<a href="/meetings?name=expense-sync" title="expense-sync"><span class="chip">expense-sync</span></a>"#
    ));
    assert!(
        hub.contains("series · tags it · 1 meeting, last Wed 11 Nov"),
        "{hub}"
    );
    assert!(hub.contains(r#"<a class="mono" href="/learn/vat-rules">vat-rules</a>"#));
    assert!(hub.contains("<h3>Topics"));
    assert!(
        hub.contains("Coming up <span class=\"small muted normal\">· 2</span>"),
        "{hub}"
    );
    assert!(hub.contains("Fri 13 · 14:00"));
    assert!(hub.contains("your event"));
    assert!(hub.contains("Expenses sync with finance"));
    assert!(hub.contains("matched by word"));
    assert!(!hub.contains("Hiring loop"));
    assert!(!hub.contains("dentist"));

    // Calendar filter: tagged own event + word-matched Google event, nothing else.
    let cal = get(&st, "/calendar?week=2026-11-16&tag=expense").await.body;
    assert!(cal.contains("Expenses sync with finance"));
    assert!(!cal.contains("Hiring loop"));
    let cal = get(&st, "/calendar?week=2026-11-11&add=expense").await.body;
    assert!(cal.contains("Expense e2e demo"));
    assert!(!cal.contains(">dentist<"), "{cal}");
    assert!(cal.contains(r#"name="tag" value="expense""#));
    let r = post(
        &st,
        "/calendar/events/1/delete",
        &form(&[("week", "2026-11-11")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert!(tags_of(&st, "event").is_empty());
}

#[tokio::test]
async fn one_name_one_hub_and_links() {
    let st = state();
    exec(
        &st,
        "INSERT INTO topics(slug, own_words) VALUES ('cms-profile', 'Reads go through the read model.');
         INSERT INTO depth_ratings(topic, day, depth) VALUES ('cms-profile', '2026-11-04', 2);",
    );
    log(
        &st,
        "? cms-profile: cursor paging beats offset past 10k rows",
    )
    .await;
    post(&st, "/meetings", &form(&[("name", "cms-profile")]), false).await;
    log(&st, "walked QA through the profile merge rules").await;
    post(&st, "/meetings/1/end", &form(&[("back", "/")]), false).await;
    log(&st, "! profile v2 keeps nested supplier #cms-profile").await;
    log(&st, "unrelated line").await;
    let b = get(&st, "/tags/cms-profile").await.body;
    assert!(b.contains("One name, one page"), "{b}");
    assert!(b.contains("<dt>Topic</dt>"));
    assert!(b.contains("can explain"));
    assert!(b.contains("Open topic page →"));
    assert!(b.contains("<dt>Series</dt>"));
    assert!(b.contains("All cms-profile meetings →"));
    assert!(b.contains("Reads go through the read model."));
    assert!(b.contains("today · topic line"), "{b}");
    assert!(b.contains("today · in cms-profile"));
    assert!(b.contains("profile v2 keeps nested supplier"));
    assert!(!b.contains("unrelated line"));
    assert!(b.contains("3 lines · 1 meeting · last used today"), "{b}");
    assert!(b.contains("in a note lands here."));

    // [[X]] → epic → tag hub → topic → meeting (D45: a shared name lands on the hub).
    log(&st, "booked #expense review").await;
    post(&st, "/notes", "", false).await;
    post(
        &st,
        "/notes/1",
        &form(&[("body", "see [[expense]], [[cms-profile]] and [[nothing]]")]),
        false,
    )
    .await;
    let n = get(&st, "/notes/1").await.body;
    assert!(n.contains(r#"href="/tags/expense""#), "{n}");
    assert!(n.contains(r#"href="/tags/cms-profile""#), "{n}");
    assert!(!n.contains(r#"href="/learn/cms-profile""#));
    assert!(!n.contains(r#"href="/tags/nothing""#));
    // Notes filter.
    post(&st, "/notes", "", false).await;
    post(
        &st,
        "/notes/2",
        &form(&[("title", "Plain"), ("body", "no tags")]),
        false,
    )
    .await;
    post(
        &st,
        "/notes/1",
        &form(&[("title", "Tagged"), ("body", "about #expense")]),
        false,
    )
    .await;
    let l = get(&st, "/notes?tag=expense").await.body;
    assert!(l.contains(r#"href="/notes/1" style"#), "{l}");
    assert!(!l.contains(r#"href="/notes/2" style"#));
    assert!(l.contains(r#"class="chip tag fl">expense"#));
}

#[tokio::test]
async fn rename_merge_delete_with_undo() {
    let st = state();
    log(&st, "OPS-1 booked #Expense review").await;
    log(&st, "#hiring loop done").await;
    post(&st, "/notes", "", false).await;
    post(&st, "/notes/1", &form(&[("body", "plan #expense")]), false).await;
    // Links the hub without the tag: a rename follows `[[expense]]` too.
    exec(
        &st,
        "INSERT INTO notes(id, title, body) VALUES (2, 'n2', 'see [[expense]]')",
    );
    exec(
        &st,
        "INSERT INTO topics(slug, tags) VALUES ('vat', 'expense')",
    );
    exec(
        &st,
        "INSERT INTO taggings VALUES ('expense', 'topic', 'vat'), ('expense', 'series', 'sync')",
    );
    request(
        &st,
        "PATCH",
        "/epics/OPS-1",
        &form(&[("tags", "expense,ops")]),
        true,
    )
    .await;

    // The ⋯ menu's confirm step is a GET; a bad name is refused with 422 and changes nothing.
    let c = get(&st, "/tags/expense?act=rename").await.body;
    assert!(c.contains(r#"action="/tags/expense/rename""#));
    assert!(c.contains("Delete tag…"));
    let r = post(&st, "/tags/expense/rename", &form(&[("to", "a_b")]), true).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("Type a tag name"));
    assert_eq!(text(&st, 1), "booked #Expense review");

    let r = post(
        &st,
        "/tags/expense/rename",
        &form(&[("to", "#Spend")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(
        r.body
            .contains("Renamed #expense to #spend · 6 places rewritten"),
        "{}",
        r.body
    );
    assert_eq!(text(&st, 1), "booked #spend review");
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM notes WHERE body = 'plan #spend'"),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE body = 'see [[spend]]'"
        ),
        1
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM epics WHERE tags = 'spend,ops'"),
        1
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM topics WHERE tags = 'spend'"),
        1
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM taggings WHERE tag = 'expense'"),
        0
    );
    assert_eq!(tags_of(&st, "series"), ["spend:sync"]);

    // Undo: one POST puts every source back.
    let r = post(
        &st,
        "/tags-undo",
        &form(&[("snap", &snap(&r.body)), ("back", "/tags/expense")]),
        true,
    )
    .await;
    assert_eq!(r.headers.get("hx-redirect").unwrap(), "/tags/expense");
    assert_eq!(text(&st, 1), "booked #Expense review");
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM epics WHERE tags = 'expense,ops'"),
        1
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM taggings WHERE tag = 'spend'"),
        0
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM taggings WHERE tag = 'expense'"),
        5
    );

    // Merge into an existing tag (plain form → 303 to the target hub).
    let r = post(
        &st,
        "/tags/expense/rename",
        &form(&[("to", "hiring")]),
        false,
    )
    .await;
    assert_eq!(r.headers.get("location").unwrap(), "/tags/hiring");
    assert_eq!(text(&st, 1), "booked #hiring review");
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM taggings WHERE tag = 'hiring'"),
        6
    );

    // Delete keeps the word, drops the # and the field entries.
    let r = post(&st, "/tags/hiring/delete", "", true).await;
    assert!(r.body.contains("Deleted #hiring"));
    assert_eq!(text(&st, 1), "booked hiring review");
    assert_eq!(text(&st, 2), "hiring loop done");
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM epics WHERE tags = 'ops'"),
        1
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM taggings WHERE tag = 'hiring'"),
        0
    );
    post(&st, "/tags-undo", &form(&[("snap", &snap(&r.body))]), false).await;
    assert_eq!(text(&st, 2), "#hiring loop done");
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM taggings WHERE tag = 'hiring'"),
        6
    );
}

#[tokio::test]
async fn make_tag_then_tag_older_lines() {
    let st = state();
    log(&st, "> send expense policy draft to finance").await;
    log(&st, "paired with Nok on the Expenses export bug").await;
    log(&st, "expensive lunch, not a tag").await;
    log(&st, "approve expense claims by hand").await;
    // Make tag: the edit box saves the word as `#expense` and says so; the toast offers the rest.
    let r = post(
        &st,
        "/entries/4",
        &form(&[
            ("line", "approve #expense claims by hand"),
            ("made", "expense"),
        ]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(
        r.body.contains("2 older lines mention ‘expense’"),
        "{}",
        r.body
    );
    assert!(r.body.contains(r#"action="/tags/expense/older""#));
    // Its Undo re-saves the line as it was.
    assert!(
        r.body
            .contains(r#"name="line" value="approve expense claims by hand""#)
    );

    let r = post(
        &st,
        "/tags/expense/older",
        &form(&[("except", "4"), ("back", "/")]),
        true,
    )
    .await;
    assert_eq!(r.headers.get("hx-trigger").unwrap(), "tagged");
    assert!(r.body.contains("Tagged 2 older lines"));
    assert_eq!(text(&st, 1), "send #expense policy draft to finance");
    assert_eq!(text(&st, 2), "paired with Nok on the #expense export bug");
    assert_eq!(text(&st, 3), "expensive lunch, not a tag");
    assert_eq!(
        tags_of(&st, "entry"),
        ["expense:1", "expense:2", "expense:4"]
    );
    // One Undo restores both, and stays a toast.
    let u = post(
        &st,
        "/tags-undo",
        &form(&[("snap", &snap(&r.body)), ("back", "/"), ("inplace", "1")]),
        true,
    )
    .await;
    assert!(u.body.contains("Undone · 2 lines"));
    assert_eq!(text(&st, 2), "paired with Nok on the Expenses export bug");
    assert_eq!(tags_of(&st, "entry"), ["expense:4"]);
    // Today's log refetches itself on `tagged`.
    assert!(get(&st, "/").await.body.contains(
        r#"hx-trigger="tagged from:body, plans-changed from:body, atharu-sync from:body""#
    ));
}

#[tokio::test]
async fn report_by_tag_filters_and_chip_field() {
    let st = state();
    log(&st, "OPS-1 ! approve claims by hand").await;
    log(&st, "+ claim to payout 6 days → 2 days #expense").await;
    log(&st, "! pairing session, not a take-home #hiring #expense").await;
    log(&st, "OPS-2 + p95 0.6s → 0.4s").await;
    // Chip field: the add box and × fold into the epic's tags; the prop comes back out-of-band.
    let r = request(
        &st,
        "PATCH",
        "/epics/OPS-1",
        &form(&[("tags", ""), ("tags_add", "#Expense")]),
        true,
    )
    .await;
    assert!(r.body.contains(r#"id="tagf" hx-swap-oob="true""#));
    assert!(r.body.contains(r#"name="tags_drop" value="expense""#));
    request(
        &st,
        "PATCH",
        "/epics/OPS-1",
        &form(&[("tags", "expense"), ("tags_add", "ops")]),
        true,
    )
    .await;
    request(
        &st,
        "PATCH",
        "/epics/OPS-1",
        &form(&[("tags", "expense,ops"), ("tags_drop", "ops")]),
        true,
    )
    .await;
    assert_eq!(tags_of(&st, "epic"), ["expense:OPS-1"]);

    let b = get(&st, "/report?group=tag").await.body;
    let e = b.find("<h3>#expense</h3>").expect(&b);
    let h = b.find("<h3>#hiring</h3>").unwrap();
    let n = b.find("<h3>No tag</h3>").unwrap();
    assert!(e < h && h < n);
    assert!(
        b.contains("<b>Impact</b> claim to payout 6 days → 2 days</li>"),
        "{b}"
    );
    assert!(
        b.contains("<b>Decision</b> approve claims by hand <span class=\"muted\">(OPS-1)</span>")
    );
    assert!(b.contains("<b>Decision</b> pairing session, not a take-home #hiring</li>"));
    assert!(b.contains("<b>Impact</b> p95 0.6s → 0.4s <span class=\"muted\">(OPS-2)</span>"));
    assert!(b.contains("## #expense\n- **Impact** claim to payout 6 days → 2 days"));
    let f = get(&st, "/report?tag=hiring").await.body;
    assert!(f.contains("pairing session") && !f.contains("approve claims by hand"));
    assert!(f.contains(r#"class="chip tag fl">hiring"#));

    // To-dos: Group by tag.
    log(&st, "OPS-1 > fix receipt upload").await;
    log(&st, "> book flights").await;
    let t = get(&st, "/todos?group=tag").await.body;
    assert!(
        t.contains(r#"<h3><a class="chip tag" href="/tags/expense">expense</a>"#),
        "{t}"
    );
    assert!(t.contains("No tag <span class=\"small muted normal\">· 1</span>"));
    assert!(t.contains(
        r#"class="on" aria-pressed="true" type="submit" name="group" value="tag">Tag</button>"#
    ));
}
