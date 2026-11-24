//! Page sync (0.2.13): /api/sync tells a page what changed elsewhere since its last look.
mod common;
use axum::{body::Body, http::Request};
use common::*;
use serde_json::Value;

async fn ask(st: &atharu::app::AppState, q: &str) -> Value {
    serde_json::from_str(&get(st, &format!("/api/sync?{q}")).await.body).unwrap()
}

async fn post_as(st: &atharu::app::AppState, uri: &str, body: &str, tab: Option<&str>) {
    let mut req = Request::post(uri)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true");
    if let Some(t) = tab {
        req = req.header("x-atharu-tab", t);
    }
    let r = send(
        atharu::app::router(st.clone()),
        req.body(Body::from(body.to_string())).unwrap(),
    )
    .await;
    assert!(r.status.is_success(), "{uri}: {}", r.status);
}

#[tokio::test]
async fn another_tabs_write_is_other_its_own_is_not() {
    let st = state();
    let first = ask(&st, "tab=a").await;
    assert_eq!(first["other"], false);
    let seq = first["seq"].as_u64().unwrap();

    post_as(&st, "/entries", &form(&[("line", "shipped it")]), Some("a")).await;
    let own = ask(&st, &format!("tab=a&since={seq}")).await;
    assert_eq!(own["other"], false, "a page's own write is not news to it");
    assert_eq!(own["seq"].as_u64(), Some(seq + 1));

    let b = ask(&st, &format!("tab=b&since={seq}")).await;
    assert_eq!(b["other"], true, "tab b sees tab a's write");

    // A write with no page id (plain form, the Mac popover) is everyone's news.
    post_as(
        &st,
        "/entries",
        &form(&[("line", "from the popover")]),
        None,
    )
    .await;
    assert_eq!(
        ask(&st, &format!("tab=a&since={}", seq + 1)).await["other"],
        true
    );

    // Typing previews change nothing.
    let now = ask(&st, "tab=a").await["seq"].as_u64().unwrap();
    post_as(
        &st,
        "/parse-preview",
        &form(&[("line", "x waiting")]),
        Some("b"),
    )
    .await;
    assert_eq!(
        ask(&st, &format!("tab=a&since={now}")).await["other"],
        false
    );
}

#[tokio::test]
async fn a_write_from_another_process_moves_the_version() {
    let path = std::env::temp_dir().join(format!("atharu-sync-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let st = atharu::app::AppState::new(atharu::db::open(&path).unwrap());
    st.conn()
        .execute(
            "INSERT INTO notes(title, body, author) VALUES ('n', 'b', 'me')",
            [],
        )
        .unwrap();
    let before = ask(&st, "tab=a&note=1").await;
    assert_eq!(before["note"]["id"], 1);
    assert_eq!(
        ask(&st, "tab=a").await["version"],
        before["version"],
        "own writes keep it"
    );

    // The CLI / an agent / the other server: its own connection.
    let cli = atharu::db::open(&path).unwrap();
    cli.execute(
        "UPDATE notes SET body = 'new', updated_at = '2099-01-01 00:00:00' WHERE id = 1",
        [],
    )
    .unwrap();
    let after = ask(&st, "tab=a&note=1").await;
    assert_ne!(after["version"], before["version"]);
    assert_eq!(after["note"]["updatedAt"], "2099-01-01 00:00:00");
    assert_eq!(ask(&st, "tab=a&note=99").await["note"], Value::Null);
    drop(cli);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}
