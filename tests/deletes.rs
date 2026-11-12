mod common;
use common::*;

/// The Undo form's hidden snapshot, unescaped the way a browser would post it back.
fn snap(body: &str) -> String {
    let at = body.find("name=\"snap\" value=\"").expect("undo form") + 19;
    let raw = &body[at..at + body[at..].find('"').unwrap()];
    // askama writes `&#34;`-style numeric entities.
    raw.replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&#60;", "<")
        .replace("&#62;", ">")
        .replace("&#38;", "&")
}

#[tokio::test]
async fn note_meeting_topic_epic_delete_and_undo() {
    let st = state();
    exec(
        &st,
        "INSERT INTO notes(id, title, body, author) VALUES (4, 'Plan & ideas', 'body \"q\"', 'claude');
         INSERT INTO meetings(id, day, name, notes, ended_at) VALUES (2, '2026-11-10', 'oms-sync', 'n', '2026-11-10 10:00:00');
         INSERT INTO entries(day, kind, text, meeting_id) VALUES ('2026-11-10', 'decision', 'keep it', 2);
         INSERT INTO topics(slug, summary) VALUES ('vector-search', 'HNSW');
         INSERT INTO depth_ratings VALUES ('vector-search', '2026-11-01', 2), ('vector-search', '2026-11-08', 3);
         INSERT INTO entries(day, kind, topic, text) VALUES ('2026-11-10', 'learn', 'vector-search', 'recall vs speed');
         INSERT INTO epics(key, okr, metric) VALUES ('OPS-5', 'O1', 'p95');
         INSERT INTO evidence(epic_key, url) VALUES ('OPS-5', 'https://x.io/pr/1');",
    );

    // Note: gone, toast names it, Undo brings it back whole under the same id.
    let r = post(&st, "/notes/4/delete", "", true).await;
    assert!(
        r.body.contains("Note “Plan &#38; ideas” deleted"),
        "{}",
        r.body
    );
    assert_eq!(count(&st, "SELECT COUNT(*) FROM notes"), 0);
    let r = post(
        &st,
        "/notes/restore",
        &form(&[("snap", &snap(&r.body))]),
        true,
    )
    .await;
    assert_eq!(r.headers["hx-redirect"], "/notes/4");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE id = 4 AND body = 'body \"q\"' AND author = 'claude'"
        ),
        1
    );

    // Meeting: its line stays (meeting_id NULL), Undo re-attaches it.
    let r = post(&st, "/meetings/2/delete", "", true).await;
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meetings"), 0);
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE text = 'keep it' AND meeting_id IS NULL"
        ),
        1
    );
    let r = post(
        &st,
        "/meetings/restore",
        &form(&[("snap", &snap(&r.body))]),
        false,
    )
    .await;
    assert_eq!(
        (r.status.as_u16(), r.headers["location"].to_str().unwrap()),
        (303, "/meetings/2")
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE text = 'keep it' AND meeting_id = 2"
        ),
        1
    );

    // Topic: ratings cascade, its ? line goes back to the inbox; Undo restores both ratings.
    let r = post(&st, "/learn/vector-search/delete", "", true).await;
    assert_eq!(count(&st, "SELECT COUNT(*) FROM depth_ratings"), 0);
    assert!(get(&st, "/learn").await.body.contains("Make topic"));
    let r = post(
        &st,
        "/learn/vector-search/restore",
        &form(&[("snap", &snap(&r.body))]),
        true,
    )
    .await;
    assert_eq!(r.headers["hx-redirect"], "/learn/vector-search");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM depth_ratings WHERE depth IN (2, 3)"
        ),
        2
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM topics WHERE summary = 'HNSW'"),
        1
    );

    // Epic with no lines: deletes with its evidence; Undo restores both.
    let r = post(&st, "/epics/OPS-5/delete", "", true).await;
    assert_eq!(count(&st, "SELECT COUNT(*) FROM evidence"), 0);
    post(
        &st,
        "/epics/OPS-5/restore",
        &form(&[("snap", &snap(&r.body))]),
        true,
    )
    .await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key = 'OPS-5' AND okr = 'O1' AND metric = 'p95'"
        ),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM evidence WHERE epic_key = 'OPS-5'"
        ),
        1
    );

    // Plain form delete: 303 back to the list.
    let r = post(&st, "/notes/4/delete", "", false).await;
    assert_eq!(r.headers["location"], "/notes");
}

#[tokio::test]
async fn epic_with_lines_stays_and_undo_conflicts_say_why() {
    let st = state();
    exec(
        &st,
        "INSERT INTO epics(key) VALUES ('OPS-1');
         INSERT INTO entries(day, epic_key, kind, text) VALUES ('2026-11-11', 'OPS-1', 'done', 'x');
         INSERT INTO meetings(id, day, name) VALUES (3, '2026-11-11', 'standup');",
    );
    let r = post(&st, "/epics/OPS-1/delete", "", true).await;
    assert_eq!(r.status, 422);
    assert_eq!(r.body, "Mark it Done or Dropped instead.");
    let r = post(&st, "/epics/OPS-1/delete", "", false).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("Mark it Done or Dropped instead."));
    assert_eq!(count(&st, "SELECT COUNT(*) FROM epics"), 1);

    // A running meeting deleted, another started: Undo can't make two run at once.
    let r = post(&st, "/meetings/3/delete", "", true).await;
    exec(
        &st,
        "INSERT INTO meetings(day, name) VALUES ('2026-11-11', 'oms-sync')",
    );
    let r = post(
        &st,
        "/meetings/restore",
        &form(&[("snap", &snap(&r.body))]),
        true,
    )
    .await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("End oms-sync first."));
}

#[tokio::test]
async fn line_edit_reparses_and_keeps_identity() {
    let st = state();
    exec(
        &st,
        "INSERT INTO entries(id, day, kind, text, author, created_at)
           VALUES (7, '2026-11-10', 'done', 'shipped it', 'claude', '2026-11-10 09:00:00');",
    );
    let b = get(&st, "/entries/7/edit").await.body;
    assert!(b.contains("value=\"shipped it\"") && b.contains("id=\"entry-7\""));

    // Done → plan on a new epic with a due date; id, day, author, created_at stay.
    let r = post(
        &st,
        "/entries/7",
        &form(&[("line", "OPS-9 > ship it by fri")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("id=\"log\""));
    let row: (String, String, Option<String>, String, String, String, String) = st
        .conn()
        .query_row(
            "SELECT kind, epic_key, plan_state, due, day, author, created_at FROM entries WHERE id = 7",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        (
            "plan".into(),
            "OPS-9".into(),
            Some("open".into()),
            "2026-11-13".into(),
            "2026-11-10".into(),
            "claude".into(),
            "2026-11-10 09:00:00".into()
        )
    );
    // Back to an impact: plan fields cleared (the CHECK would refuse otherwise).
    post(
        &st,
        "/entries/7",
        &form(&[("line", "OPS-9 + p95 down")]),
        false,
    )
    .await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE id = 7 AND kind = 'impact' AND plan_state IS NULL AND due IS NULL"
        ),
        1
    );

    // Bad grammar: 422, input kept, row untouched; htmx retargets just the row.
    let r = post(&st, "/entries/7", &form(&[("line", "OPS-9 +")]), true).await;
    assert_eq!(r.status, 422);
    assert_eq!(r.headers["hx-retarget"], "#entry-7");
    assert!(r.body.contains("value=\"OPS-9 +\"") && r.body.contains("class=\"err\""));
    let r = post(
        &st,
        "/entries/7",
        &form(&[("line", "> x by someday")]),
        false,
    )
    .await;
    assert_eq!(r.status, 422);
    assert!(
        r.body.contains("x by someday\"") && r.body.contains("read date ‘someday’"),
        "{}",
        r.body
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE id = 7 AND text = 'p95 down'"
        ),
        1
    );
    // No-JS edit box.
    assert!(
        get(&st, "/?day=2026-11-10&edit=7")
            .await
            .body
            .contains("aria-label=\"Edit line\"")
    );
}

#[test]
fn wal_on_and_one_rating_per_day() {
    let path = std::env::temp_dir().join(format!("atharu-wal-{}.db", std::process::id()));
    let conn = atharu::db::open(&path).unwrap();
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    conn.execute_batch("INSERT INTO topics(slug) VALUES ('t'); INSERT INTO depth_ratings VALUES ('t', '2026-11-11', 2);")
        .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO depth_ratings VALUES ('t', '2026-11-11', 3)",
            []
        )
        .is_err(),
        "UNIQUE(topic, day)"
    );
    drop(conn);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}

#[test]
fn migration_005_keeps_the_latest_duplicate() {
    // A DB at version 4 with duplicate ratings, as older builds could leave it.
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    for (i, sql) in [
        include_str!("../migrations/001_init.sql"),
        include_str!("../migrations/002_author.sql"),
        include_str!("../migrations/003_jira.sql"),
        include_str!("../migrations/004_settings.sql"),
    ]
    .iter()
    .enumerate()
    {
        conn.execute_batch(&format!("{sql}\nPRAGMA user_version = {};", i + 1))
            .unwrap();
    }
    conn.execute_batch(
        "INSERT INTO topics(slug) VALUES ('t');
         INSERT INTO depth_ratings VALUES ('t', '2026-11-11', 1), ('t', '2026-11-11', 3), ('t', '2026-11-12', 2);",
    )
    .unwrap();
    conn.execute_batch(include_str!("../migrations/005_hardening.sql"))
        .unwrap();
    let got: Vec<(String, i64)> = conn
        .prepare("SELECT day, depth FROM depth_ratings ORDER BY day")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(got, [("2026-11-11".into(), 3), ("2026-11-12".into(), 2)]);
}
