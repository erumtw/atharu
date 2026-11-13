mod common;
use atharu::config::FileConfig;
use common::*;

const FIXTURE: &str = include_str!("fixtures/jira_search.json");

#[tokio::test]
async fn cache_upserts_and_shows_where_our_title_is_empty() {
    let st = state();
    exec(
        &st,
        "INSERT INTO epics(key) VALUES ('OPS-101'), ('OPS-9');
         INSERT INTO entries(day, kind, text, plan_state, link) VALUES
           ('2026-11-11', 'plan', 'fix rounding https://acme.atlassian.net/browse/OPS-7', 'open',
            'https://acme.atlassian.net/browse/OPS-7'),
           ('2026-11-11', 'plan', 'look at https://acme.atlassian.net/browse/OPS-70', 'open',
            'https://acme.atlassian.net/browse/OPS-70');",
    );
    assert_eq!(
        atharu::jira::known_keys(&st.conn()).unwrap(),
        ["OPS-101", "OPS-7", "OPS-70", "OPS-9"]
    );
    for _ in 0..2 {
        assert_eq!(atharu::jira::import(&mut st.conn(), FIXTURE), Ok(2));
    }
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM jira_issues"),
        2,
        "upsert, no duplicates"
    );

    let page = get(&st, "/epics/OPS-101").await.body;
    assert!(page.contains("Jira: Faster checkout · In Progress"));
    let list = get(&st, "/epics").await.body;
    // D7: no title typed → Jira's is the name, marked; OPS-9 (not in Jira) still asks for one.
    assert!(list.contains("<b>Faster checkout</b></a><span class=\"small muted\" title=\"Jira's title, until you type one\">from Jira</span>"), "{list}");
    assert!(list.contains("Title for OPS-9") && !list.contains("Title for OPS-101"));
    // Our title wins; the cache is display-only and never copied into epics.
    exec(
        &st,
        "UPDATE epics SET title = 'Checkout latency' WHERE key = 'OPS-101'",
    );
    assert!(
        !get(&st, "/epics/OPS-101")
            .await
            .body
            .contains("Faster checkout")
    );

    // Link chip: the cached key, not OPS-70 (prefix) or a bare "Jira".
    let todos = get(&st, "/todos").await.body;
    assert!(todos.contains("OPS-7 · Fix invoice rounding ↗"));
    assert_eq!(todos.matches("OPS-7 · Fix invoice rounding").count(), 1);
    assert!(
        todos.contains(">Jira ↗</a>"),
        "uncached OPS-70 keeps the host label"
    );
}

#[tokio::test]
async fn off_without_all_three_keys() {
    let st = state();
    assert_eq!(
        atharu::jira::refresh(&st).await,
        Ok(None),
        "no config → no request"
    );
    assert!(!get(&st, "/epics").await.body.contains("/jira/refresh"));

    let dir = std::env::temp_dir().join(format!("atharu-jira-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "jira_base_url = \"https://acme.atlassian.net/\"\njira_email = \"me@example.com\"\n",
    )
    .unwrap();
    assert!(
        FileConfig::load(&path).jira().is_none(),
        "token missing → off"
    );
    std::fs::write(
        &path,
        "jira_base_url = \"https://acme.atlassian.net/\"\njira_email = \"me@example.com\"\njira_token = \"t0k\"\n",
    )
    .unwrap();
    let cfg = FileConfig::load(&path);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(cfg.jira().unwrap().base_url, "https://acme.atlassian.net");

    let mut st = state();
    st.settings = std::sync::Arc::new(cfg);
    exec(
        &st,
        "INSERT INTO epics(key, title) VALUES ('OPS-101', 'Checkout');
         INSERT INTO jira_issues(key, summary, status) VALUES ('OPS-101', 'Faster checkout', 'In Progress');",
    );
    let b = get(&st, "/epics").await.body;
    assert!(b.contains("action=\"/jira/refresh\"") && !b.contains("t0k"));
    // The ↗ chip: cached status, linking the issue on the configured site (trailing / dropped).
    let chip = r#"<a class="lchip" href="https://acme.atlassian.net/browse/OPS-101" target="_blank" rel="noopener noreferrer" title="Open OPS-101 in Jira"><span class="jst-OPS-101">In Progress</span> ↗</a>"#;
    assert!(
        b.contains(&chip.replace("class=\"lchip\"", "class=\"lchip jc-\"")),
        "{b}"
    );
    assert!(
        get(&st, "/epics/OPS-101")
            .await
            .body
            .contains(&chip.replace("class=\"lchip\"", "class=\"lchip jc-\""))
    );
    assert!(
        get(&st, "/epics?jira=failed")
            .await
            .body
            .contains("Couldn't reach Jira.")
    );
}

/// A local fake Jira (never the owner's): `search/jql` answers the tracked-keys search with the full fixture and the
/// assigned-to-me search with `assigned`; `status` overrides the HTTP status (401 = expired token).
async fn fake_jira(
    assigned: &'static str,
    status: std::sync::Arc<std::sync::atomic::AtomicU16>,
) -> String {
    use axum::{extract::Query, http::StatusCode, routing::get};
    use std::collections::HashMap;
    let app = axum::Router::new().route(
        "/rest/api/3/search/jql",
        get(move |Query(q): Query<HashMap<String, String>>| {
            let status = status.clone();
            async move {
                let code = status.load(std::sync::atomic::Ordering::SeqCst);
                let body = if q.get("jql").is_some_and(|j| j.starts_with("assignee")) {
                    assigned
                } else {
                    include_str!("fixtures/jira_search_full.json")
                };
                (StatusCode::from_u16(code).unwrap(), body)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

/// State whose config points Jira at `base`.
fn jira_state(base: &str, tag: &str) -> atharu::app::AppState {
    let dir = std::env::temp_dir().join(format!("atharu-jira-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        format!(
            "jira_base_url = \"{base}\"\njira_email = \"me@example.com\"\njira_token = \"t0k\"\n"
        ),
    )
    .unwrap();
    let cfg = FileConfig::load(&path);
    std::fs::remove_dir_all(&dir).unwrap();
    let mut st = state();
    st.settings = std::sync::Arc::new(cfg);
    st
}

#[tokio::test]
async fn refresh_reads_hierarchy_flags_assigned_and_names_a_dead_token() {
    use std::sync::{
        Arc,
        atomic::{AtomicU16, Ordering},
    };
    let status = Arc::new(AtomicU16::new(200));
    let assigned = r#"{"issues": [{"key": "PAY-9", "fields": {"summary": "Refund flow",
        "status": {"name": "Ready", "statusCategory": {"key": "new"}}, "issuetype": {"name": "Task"},
        "duedate": "2026-11-20", "updated": "2026-11-14T08:00:00.000+0700"}}]}"#;
    let base = fake_jira(assigned, status.clone()).await;
    let st = jira_state(&base, "fake");
    exec(
        &st,
        "INSERT INTO epics(key) VALUES ('OPS-130'), ('OPS-131');
         INSERT INTO jira_issues(key, summary, status, assigned_me) VALUES ('OLD-1', 'was mine', 'Ready', 1);",
    );
    assert_eq!(atharu::jira::refresh(&st).await, Ok(Some(3)));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM jira_issues WHERE key = 'OPS-131' AND issue_type = 'Story'
               AND status_cat = 'done' AND parent_key = 'OPS-130' AND resolved_on = '2026-11-12'"
        ),
        1
    );
    // Assigned: exactly what the search returned (PAY-9, not tracked yet); OLD-1 lost the flag.
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM jira_issues WHERE assigned_me = 1 AND key = 'PAY-9'"
        ),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM jira_issues WHERE assigned_me = 1"
        ),
        1
    );

    // 401: an error, stored for Settings / Epics (D9); the cached rows stay.
    status.store(401, Ordering::SeqCst);
    assert_eq!(
        atharu::jira::refresh(&st).await,
        Err("Jira answered 401".into())
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM settings WHERE key = 'jira_auth_failed'"
        ),
        1
    );
    assert_eq!(count(&st, "SELECT COUNT(*) FROM jira_issues"), 4);
    // The next good answer clears it.
    status.store(200, Ordering::SeqCst);
    assert!(atharu::jira::refresh(&st).await.is_ok());
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM settings WHERE key = 'jira_auth_failed'"
        ),
        0
    );
}

/// A fake Jira workflow for one issue: Ready → In Progress → Done (needs Resolution) / Won't do; posts are recorded.
async fn fake_workflow(
    posted: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    status: std::sync::Arc<std::sync::atomic::AtomicU16>,
) -> String {
    use axum::{extract::Path, http::StatusCode, routing::get};
    let at = std::sync::Arc::new(std::sync::Mutex::new("Ready".to_string()));
    let (at_get, at_post) = (at.clone(), at.clone());
    let (st_get, st_post) = (status.clone(), status.clone());
    let app = axum::Router::new().route(
        "/rest/api/3/issue/{key}/transitions",
        get(move |Path(_k): Path<String>| {
            let at = at_get.clone();
            let code = st_get.load(std::sync::atomic::Ordering::SeqCst);
            async move {
                // From Ready: only forward. From In Progress: back to Ready, Done (needs a field), Won't do.
                let body = if *at.lock().unwrap() == "Ready" {
                    r#"{"transitions":[{"id":"21","name":"Start","to":{"name":"In Progress","statusCategory":{"key":"indeterminate"}}}]}"#
                } else {
                    r#"{"transitions":[
                      {"id":"11","name":"Back to ready","to":{"name":"Ready","statusCategory":{"key":"new"}}},
                      {"id":"31","name":"Done","to":{"name":"Done","statusCategory":{"key":"done"}},
                       "fields":{"resolution":{"required":true,"name":"Resolution"}}},
                      {"id":"41","name":"Won't do","to":{"name":"Won't do","statusCategory":{"key":"done"}}}]}"#
                };
                (StatusCode::from_u16(code).unwrap(), body)
            }
        })
        .post(move |Path(_k): Path<String>, body: String| {
            let (at, posted) = (at_post.clone(), posted.clone());
            let code = st_post.load(std::sync::atomic::Ordering::SeqCst);
            async move {
                posted.lock().unwrap().push(body.clone());
                if code != 200 {
                    return (StatusCode::from_u16(code).unwrap(), String::new());
                }
                let to = match body.as_str() {
                    b if b.contains("\"21\"") => "In Progress",
                    b if b.contains("\"11\"") => "Ready",
                    b if b.contains("\"31\"") => {
                        return (StatusCode::BAD_REQUEST, r#"{"errors":{"resolution":"Resolution is required."}}"#.into());
                    }
                    _ => "Won't do",
                };
                *at.lock().unwrap() = to.into();
                (StatusCode::NO_CONTENT, String::new())
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

#[tokio::test]
async fn status_moves_on_press_with_undo() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU16, Ordering},
    };
    let posted = Arc::new(Mutex::new(Vec::new()));
    let status = Arc::new(AtomicU16::new(200));
    let base = fake_workflow(posted.clone(), status.clone()).await;
    let st = jira_state(&base, "moves");
    exec(
        &st,
        "INSERT INTO epics(key, title) VALUES ('OPS-130', 'Cold chain');
         INSERT INTO jira_issues(key, summary, status, status_cat) VALUES ('OPS-130', 'Cold-chain alerts', 'Ready', 'new');",
    );
    // Every Jira chip has the ▾; nothing is sent to Jira until the menu opens.
    let list = get(&st, "/epics").await.body;
    assert!(
        list.contains("<span class=\"jst-OPS-130\">Ready</span>")
            && list.contains("hx-get=\"/jira/OPS-130/moves?back=/epics\""),
        "{list}"
    );
    assert!(posted.lock().unwrap().is_empty());

    // The menu: Jira's allowed moves, read live, + Open in Jira.
    let menu = get(&st, "/jira/OPS-130/moves?back=/epics").await.body;
    assert!(
        menu.contains(r##"hx-vals='{"to": "21", "back": "/epics"}' hx-target="#jtoast""##)
            && menu.contains(">In Progress <span class=\"small muted\">· Start</span>"),
        "{menu}"
    );
    assert!(menu.contains(
        "/browse/OPS-130\" target=\"_blank\" rel=\"noopener noreferrer\">Open in Jira ↗</a>"
    ));

    // Pick: one POST, the cache follows, the toast offers Undo back to Ready; chips update out-of-band.
    let r = post(
        &st,
        "/jira/OPS-130/move",
        &form(&[("to", "21"), ("back", "/epics")]),
        true,
    )
    .await;
    assert_eq!(
        posted.lock().unwrap().as_slice(),
        [r#"{"transition":{"id":"21"}}"#]
    );
    assert!(
        r.body.contains("Moved OPS-130 to In Progress in Jira"),
        "{}",
        r.body
    );
    assert!(
        r.body.contains("action=\"/jira/OPS-130/undo\"")
            && r.body.contains("name=\"to\" value=\"Ready\"")
    );
    assert!(
        r.body
            .contains("<span hx-swap-oob=\"innerHTML:.jst-OPS-130\">In Progress</span>")
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM jira_issues WHERE status = 'In Progress' AND status_cat = 'indeterminate'"
        ),
        1
    );

    // Undo = the move back to the previous status.
    let r = post(&st, "/jira/OPS-130/undo", &form(&[("to", "Ready")]), true).await;
    assert!(r.body.contains("Moved OPS-130 back to Ready in Jira") && !r.body.contains("/undo"));
    // From Ready there's no move to "Won't do": say so, send nothing.
    let n = posted.lock().unwrap().len();
    let r = post(
        &st,
        "/jira/OPS-130/undo",
        &form(&[("to", "Won't do")]),
        true,
    )
    .await;
    assert!(
        r.body.contains("Can&#39;t undo in Jira: open OPS-130"),
        "{}",
        r.body
    );
    assert_eq!(posted.lock().unwrap().len(), n);

    // A move that needs a field: hand over to Jira, never guess; the cache stays.
    post(&st, "/jira/OPS-130/move", &form(&[("to", "21")]), true).await;
    let r = post(&st, "/jira/OPS-130/move", &form(&[("to", "31")]), true).await;
    assert!(
        r.body
            .contains("Jira needs Resolution for “Done”. Open in Jira"),
        "{}",
        r.body
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM jira_issues WHERE status = 'In Progress'"
        ),
        1
    );

    // No JS: 303 back.
    let r = post(
        &st,
        "/jira/OPS-130/move",
        &form(&[("to", "11"), ("back", "/epics/OPS-130")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/epics/OPS-130");

    // 401: named (D9), stored, nothing moved.
    status.store(401, Ordering::SeqCst);
    let r = post(&st, "/jira/OPS-130/move", &form(&[("to", "21")]), true).await;
    assert!(
        r.body.contains("Token expired or revoked.")
            && r.body
                .contains("id.atlassian.com/manage-profile/security/api-tokens")
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM settings WHERE key = 'jira_auth_failed'"
        ),
        1
    );
    // Not a key: never reaches Jira.
    assert_eq!(get(&st, "/jira/..%2Fadmin/moves").await.status, 404);
}

#[tokio::test]
async fn epics_list_reads_jira_assigned_parents_and_flags() {
    // Today is Wed 11 Nov 2026. Cached rows only: nothing here talks to Jira.
    let st = jira_state("https://acme.atlassian.net", "list");
    exec(
        &st,
        "INSERT INTO epics(key, title, okr, target) VALUES
           ('OPS-101', 'Checkout latency', 'O2', NULL), ('OPS-118', 'Reorder', 'O2', NULL),
           ('OPS-142', '', '', '2026-12-20');
         INSERT INTO entries(day, epic_key, kind, text) VALUES ('2026-11-11', 'OPS-101', 'done', 'shipped'), ('2026-11-11', 'OPS-142', 'done', 'repro'), ('2026-11-09', 'OPS-118', 'done', 'spec');
         INSERT INTO jira_issues(key, summary, status, status_cat, issue_type, parent_key, parent_summary, due, updated_at, assigned_me) VALUES
           ('OPS-500', 'Checkout platform', 'In progress', 'indeterminate', 'Epic', NULL, NULL, NULL, NULL, 0),
           ('OPS-101', 'Faster checkout', 'Done', 'done', 'Story', 'OPS-500', 'Checkout platform', NULL, '2026-11-10T09:00', 0),
           ('OPS-118', 'Reorder from history', 'In review', 'indeterminate', 'Story', 'OPS-500', 'Checkout platform', NULL, '2026-11-11T09:00', 1),
           ('OPS-142', 'Driver app crash', 'To do', 'new', 'Bug', NULL, NULL, '2026-12-24', NULL, 0),
           ('OPS-160', 'Supplier price feed v2', 'To do', 'new', 'Epic', NULL, NULL, '2026-12-04', '2026-11-11T10:00', 1),
           ('OPS-161', 'Retry price feed', 'In progress', 'indeterminate', 'Story', 'OPS-160', 'Supplier price feed v2', '2026-11-13', '2026-11-11T08:00', 1);
         INSERT INTO settings(key, value) VALUES ('jira_auth_failed', '2026-11-11 08:00:00');",
    );
    let b = get(&st, "/epics").await.body;
    // D5/D6: assigned section; an Epic offers Track, a Story a filled to-do, a tracked one ✓.
    assert!(
        b.contains("In Jira, assigned to you <span class=\"muted normal\">· 3 open</span>"),
        "{b}"
    );
    assert!(b.contains("action=\"/epics/OPS-160/track\""));
    assert!(b.contains("name=\"line\" value=\"OPS-161 &#62; Retry price feed https://acme.atlassian.net/browse/OPS-161 by 2026-11-13\""));
    assert!(b.contains("<span class=\"muted\">due Fri</span>") && b.contains("✓ tracked"));
    // D4: OPS-101 and OPS-118 sit under the untracked OPS-500 header.
    let head = b
        .find("<tr class=\"jhead\"><td><span class=\"chip key\">OPS-500</span>")
        .expect("parent header");
    assert!(
        head < b
            .find("<tr class=\"child\"><td><a href=\"/epics/OPS-101\"")
            .unwrap()
    );
    assert!(b.contains(
        "<b>Checkout platform</b><span class=\"small muted\">Jira epic · not tracked</span>"
    ));
    // Category drives the chip, never the name.
    assert!(b.contains("class=\"lchip jc-done\"") && b.contains("class=\"lchip jc-new\""));
    // D7 + flags: OPS-142 named from Jira; its Jira due differs from the target; OPS-118 moved in Jira after no log.
    assert!(b.contains("<b>Driver app crash</b>"));
    assert!(
        b.contains("due dates differ")
            && b.contains("Jira due Thu 24 Dec · your target Sun 20 Dec")
    );
    assert!(b.contains("Jira moved, no log"));
    // D9: the dead token is named, the last copy stays.
    assert!(b.contains("⚠ Jira token expired or revoked.") && b.contains("api-tokens"));
    assert!(
        get(&st, "/settings")
            .await
            .body
            .contains("Token expired or revoked.")
    );
    // Today: one line, to the section.
    assert!(
        get(&st, "/")
            .await
            .body
            .contains("<a href=\"/epics#assigned\">3 Jira tickets assigned to you →</a>")
    );

    // + Track as epic: the stub, named by Jira (D7); ✓ tracked after.
    let r = post(
        &st,
        "/epics/OPS-160/track",
        &form(&[("back", "/epics#assigned")]),
        false,
    )
    .await;
    assert_eq!(r.headers["location"], "/epics#assigned");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key = 'OPS-160' AND title = ''"
        ),
        1
    );
    assert!(
        !get(&st, "/epics")
            .await
            .body
            .contains("action=\"/epics/OPS-160/track\"")
    );
}

#[tokio::test]
async fn epic_page_follows_jira_done_tickets_and_suggests_moves() {
    use std::sync::{Arc, Mutex, atomic::AtomicU16};
    let posted = Arc::new(Mutex::new(Vec::new()));
    let base = fake_workflow(posted.clone(), Arc::new(AtomicU16::new(200))).await;
    let st = jira_state(&base, "epicpage");
    exec(
        &st,
        "INSERT INTO epics(key, title) VALUES ('OPS-500', 'Checkout platform'), ('OPS-118', '');
         INSERT INTO entries(day, epic_key, kind, text) VALUES ('2026-11-10', 'OPS-118', 'done', 'spec'), ('2026-11-11', 'OPS-118', 'done', 'review');
         INSERT INTO jira_issues(key, summary, status, status_cat, issue_type, parent_key, parent_summary, resolved_on) VALUES
           ('OPS-500', 'Checkout platform', 'Done', 'done', 'Epic', NULL, NULL, '2026-11-12'),
           ('OPS-118', 'Reorder from history', 'In review', 'indeterminate', 'Story', 'OPS-500', 'Checkout platform', NULL),
           ('OPS-161', 'Retry price feed', 'To do', 'new', 'Bug', 'OPS-500', 'Checkout platform', NULL);",
    );
    // D2 banner, D3 tickets: children from Jira's parent field, logged or not.
    let b = get(&st, "/epics/OPS-500").await.body;
    assert!(
        b.contains("Jira says <span class=\"chip key\">OPS-500</span> is Done on Thu 12 Nov."),
        "{b}"
    );
    assert!(b.contains("form=\"jdone\" name=\"act\" value=\"done\">Mark done</button>"));
    assert!(
        b.contains("Tickets <span class=\"small muted normal\">· 2 in Jira · 1 logged here</span>")
    );
    assert!(
        b.contains("<a href=\"/epics/OPS-118\"><span class=\"chip key\">OPS-118</span></a>")
            && b.contains(">2 lines</span>")
    );
    assert!(
        b.contains("/browse/OPS-161\"><span class=\"chip key\">OPS-161</span></a>")
            && b.contains(">not logged</span>")
    );
    // Story head (D3): its type, and what it sits under (tracked → its page here); named by Jira (D7).
    let s = get(&st, "/epics/OPS-118").await.body;
    assert!(s.contains("<div class=\"pgk\"><span>Story</span>"));
    assert!(s.contains("under <a href=\"/epics/OPS-500\"><span class=\"chip key\">OPS-500</span> Checkout platform</a>"));
    assert!(
        s.contains("<title>Atharu · Reorder from history</title>"),
        "page title from Jira"
    );

    // Not yet: hidden for this resolution date. Mark done: Jira's date becomes done on.
    post(
        &st,
        "/epics/OPS-500/jira-done",
        &form(&[("act", "notyet")]),
        false,
    )
    .await;
    assert!(!get(&st, "/epics/OPS-500").await.body.contains("Jira says"));
    post(
        &st,
        "/epics/OPS-500/jira-done",
        &form(&[("act", "done")]),
        false,
    )
    .await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key = 'OPS-500' AND status = 'done' AND done_on = '2026-11-12'"
        ),
        1
    );

    // Done in Atharu (D2): the save asks for the suggestion; the fake starts at Ready (no done move) → no prompt.
    let r = request(
        &st,
        "PATCH",
        "/epics/OPS-118",
        &form(&[("status", "done"), ("role", "owner")]),
        true,
    )
    .await;
    assert!(
        r.body
            .contains("hx-get=\"/jira/OPS-118/suggest?for=done&back=/epics/OPS-118\""),
        "{}",
        r.body
    );
    assert_eq!(
        get(&st, "/jira/OPS-118/suggest?for=done").await.body.trim(),
        ""
    );
    // From In Progress: done → the Done move (not the cancel one); dropped → Won't do only (D10).
    post(&st, "/jira/OPS-118/move", &form(&[("to", "21")]), true).await;
    let d = get(&st, "/jira/OPS-118/suggest?for=done").await.body;
    assert!(
        d.contains("Also move <span class=\"chip key\">OPS-118</span> in Jira?")
            && d.contains(">Done</button>")
            && !d.contains("Won"),
        "{d}"
    );
    let x = get(&st, "/jira/OPS-118/suggest?for=dropped").await.body;
    assert!(
        x.contains(">Won&#39;t do</button>") && !x.contains(">Done</button>"),
        "{x}"
    );

    // A Jira-linked to-do dropped: the row asks too.
    exec(
        &st,
        &format!(
            "INSERT INTO entries(id, day, kind, text, plan_state, link) VALUES (90, '2026-11-11', 'plan', 'retry {base}/browse/OPS-161', 'open', '{base}/browse/OPS-161')"
        ),
    );
    let r = post(
        &st,
        "/plans/90",
        &form(&[("state", "dropped"), ("back", "/todos")]),
        true,
    )
    .await;
    assert!(
        r.body
            .contains("hx-get=\"/jira/OPS-161/suggest?for=dropped&back=/todos\""),
        "{}",
        r.body
    );
}
