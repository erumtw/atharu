//! Settings › About › Database file (T85). One test: the pointer env is process-wide, so it lives in its own binary.
mod common;
use common::*;

#[tokio::test]
async fn database_file_row_and_change() {
    let st = state();
    let s = get(&st, "/settings").await.body;
    assert!(s.contains("<b>Database file</b>") && s.contains("value=\"In memory\" readonly"));
    assert!(
        !s.contains("/settings/db\""),
        "no Change form outside the Mac app"
    );
    assert_eq!(
        post(&st, "/settings/db", &form(&[("path", "/x.db")]), true)
            .await
            .status,
        404
    );

    let dir = std::env::temp_dir().join(format!("atharu-dbptr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ptr = dir.join("app/db-path");
    // SAFETY: the only test in this binary, set before any other thread reads the env.
    unsafe { std::env::set_var(atharu::config::DB_POINTER_ENV, &ptr) };

    let s = get(&st, "/settings").await.body;
    assert!(s.contains("hx-post=\"/settings/db\"") && s.contains("value=\"In memory\""));
    assert!(s.contains("<b>Old Mac app data</b>"), "one-time note");
    assert!(!s.contains("Restart Atharu to switch"));

    let r = post(&st, "/settings/db", &form(&[("path", "data/a.db")]), true).await;
    assert_eq!(r.status, 422);
    assert!(
        r.body.contains("Use a full path, starting with / or ~/.")
            && r.body.contains("value=\"data/a.db\"")
    );
    assert!(
        r.body.trim_start().starts_with("<div id=\"db\""),
        "htmx gets the partial"
    );
    let r = post(&st, "/settings/db", &form(&[("path", "/nope/x.db")]), false).await;
    assert_eq!(r.status, 422);
    assert!(
        r.body.contains("No folder /nope. Create it first.") && r.body.contains("<h1"),
        "plain POST gets the page"
    );
    assert!(!ptr.exists());

    let db = dir.join("shared.db");
    let r = post(
        &st,
        "/settings/db",
        &form(&[("path", db.to_str().unwrap())]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(std::fs::read_to_string(&ptr).unwrap(), db.to_str().unwrap());
    let r = post(
        &st,
        "/settings/db",
        &form(&[("path", db.to_str().unwrap())]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(
        r.body.contains("Restart Atharu to switch") && r.body.contains("href=\"/app/restart\"")
    );

    assert_eq!(post(&st, "/settings/db/note", "", true).await.body, "");
    assert_eq!(post(&st, "/settings/db/note", "", false).await.status, 303);
    assert!(
        !get(&st, "/settings")
            .await
            .body
            .contains("Old Mac app data")
    );
}
