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
    assert!(list.contains("Faster checkout · In Progress") && list.contains("Title for OPS-101"));
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
    let chip = r#"<a class="lchip" href="https://acme.atlassian.net/browse/OPS-101" target="_blank" rel="noopener noreferrer" title="Open OPS-101 in Jira">In Progress ↗</a>"#;
    assert!(b.contains(chip), "{b}");
    assert!(get(&st, "/epics/OPS-101").await.body.contains(chip));
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
