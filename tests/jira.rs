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
    let b = get(&st, "/epics").await.body;
    assert!(b.contains("action=\"/jira/refresh\"") && !b.contains("t0k"));
    assert!(
        get(&st, "/epics?jira=failed")
            .await
            .body
            .contains("Couldn't reach Jira.")
    );
}
