mod common;
use common::*;

#[tokio::test]
async fn one_active_meeting_at_a_time() {
    let st = state();
    let r = post(
        &st,
        "/meetings",
        &form(&[("name", "OMS Sync"), ("back", "/")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(
        r.body
            .contains("In <a href=\"/meetings/1\"><b>oms-sync</b></a>")
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meetings WHERE name = 'oms-sync' AND ended_at IS NULL"
        ),
        1
    );

    let r = post(&st, "/meetings", &form(&[("name", "planning")]), true).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("End oms-sync first."));
    let r = post(&st, "/meetings", &form(&[("name", "planning")]), false).await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("End oms-sync first."));
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meetings"), 1);
    // The index is the backstop even if a handler skipped the check.
    assert!(
        st.conn()
            .execute(
                "INSERT INTO meetings(day, name) VALUES ('2026-11-11', 'x')",
                []
            )
            .is_err()
    );

    let r = post(&st, "/meetings/1/end", &form(&[("back", "/")]), false).await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/");
    let r = post(&st, "/meetings", &form(&[("name", "planning")]), false).await;
    assert_eq!(r.status, 303);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meetings"), 2);

    let r = post(&st, "/meetings", &form(&[("name", "  !! ")]), true).await;
    assert_eq!(r.status, 422);
}

#[tokio::test]
async fn lines_logged_during_a_meeting_get_its_id() {
    let st = state();
    post(&st, "/entries", &form(&[("line", "before it")]), false).await;
    post(&st, "/meetings", &form(&[("name", "oms-sync")]), false).await;
    let b = get(&st, "/").await.body;
    assert!(b.contains("lines you log now are saved to this meeting"));
    post(
        &st,
        "/entries",
        &form(&[("line", "OPS-101 ! keep sync path")]),
        false,
    )
    .await;
    post(
        &st,
        "/entries",
        &form(&[("line", "? kafka: lag is rebalance")]),
        false,
    )
    .await;
    post(
        &st,
        "/todos",
        &form(&[("line", "OPS-101 profile join")]),
        false,
    )
    .await;
    post(&st, "/meetings/1/end", "", false).await;
    post(&st, "/entries", &form(&[("line", "after it")]), false).await;

    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM entries WHERE meeting_id = 1"),
        3
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM entries WHERE meeting_id IS NULL"),
        2
    );

    let b = get(&st, "/meetings/1").await.body;
    let pos = |s: &str| b.find(s).unwrap_or_else(|| panic!("missing {s}"));
    assert!(pos("Action items") < pos("profile join"));
    assert!(pos("profile join") < pos("<h3>Decisions"));
    assert!(pos("<h3>Decisions") < pos("keep sync path"));
    assert!(pos("<h3>Learned") < pos("lag is rebalance"));
    assert!(b.contains("ended "));

    let r = post(
        &st,
        "/meetings/1/notes",
        &form(&[("notes", "Revisit queue design\nif traffic doubles")]),
        true,
    )
    .await;
    assert!(r.body.contains("✓ saved"));
    let b = get(&st, "/meetings").await.body;
    assert!(b.contains("<summary>oms-sync"));
    assert!(b.contains("<td>1 open</td>"));
    assert!(b.contains("Revisit queue design"));
    assert!(!b.contains("if traffic doubles"));
    assert!(b.contains("<span class=\"ell\">oms-sync</span><span>· 1</span>"));
    assert!(
        get(&st, "/")
            .await
            .body
            .contains(r#"<span class="chip">oms-sync</span>"#)
    );
}

#[tokio::test]
async fn series_filter_and_report_numbers() {
    let st = state();
    exec(
        &st,
        "INSERT INTO meetings(id, day, name, ended_at) VALUES
           (1, '2026-11-04', 'oms-sync', '2026-11-04 10:42:00'),
           (2, '2026-11-11', 'oms-sync', '2026-11-11 10:42:00'),
           (3, '2026-11-10', 'planning', '2026-11-10 11:00:00');
         INSERT INTO entries(day, kind, text, meeting_id, plan_state, closed_day) VALUES
           ('2026-11-11', 'decision', 'keep sync path', 2, NULL, NULL),
           ('2026-11-10', 'decision', 'outside a meeting', NULL, NULL, NULL),
           ('2026-11-09', 'plan', 'a', NULL, 'done', '2026-11-10'),
           ('2026-11-09', 'plan', 'b', NULL, 'done', '2026-11-10'),
           ('2026-11-10', 'plan', 'c', NULL, 'done', '2026-11-11'),
           ('2026-11-10', 'plan', 'd', NULL, 'done', '2026-11-11'),
           ('2026-11-11', 'plan', 'e', NULL, 'open', NULL),
           ('2026-11-11', 'plan', 'f', NULL, 'dropped', '2026-11-11'),
           ('2026-11-02', 'plan', 'last week', NULL, 'open', NULL);",
    );
    let b = get(&st, "/meetings?name=planning").await.body;
    assert!(b.contains("<summary>planning"));
    assert!(!b.contains("<summary>oms-sync"));

    let b = get(&st, "/meetings/2").await.body;
    assert!(b.contains("ended 10:42"));
    assert!(b.contains(
        r#"<a href="/meetings/1">Wed 4 Nov</a>·<a href="/meetings?name=oms-sync">all 2</a>"#
    ));

    // This week (9–15 Nov): 4 of 6 plans done = 67%; 3 meetings minus last week's = 2.
    let b = get(&st, "/report").await.body;
    assert!(b.contains("<b>67%</b><span class=\"small muted\">4 of 6 · 1 open · 1 dropped</span>"));
    assert!(b.contains("<b>2</b><span class=\"small muted\">1 in meetings</span>"));
    assert!(b.contains(r#"<span class="small muted">Meetings</span><b>2</b>"#));
    assert!(b.contains("- **Decision** keep sync path (oms-sync)"));
}

#[tokio::test]
async fn rename_moves_the_whole_series_and_its_tags() {
    let st = state();
    exec(
        &st,
        "INSERT INTO meetings(id, day, name, ended_at) VALUES
           (1, '2026-11-10', 'comps-add-on-cs-hub-from-comops', '2026-11-10 10:00:00'),
           (2, '2026-11-11', 'comps-add-on-cs-hub-from-comops', '2026-11-11 10:00:00'),
           (3, '2026-11-11', 'oms-sync', '2026-11-11 11:00:00');",
    );
    exec(
        &st,
        "INSERT INTO taggings(tag, item_type, item_id) VALUES ('cshub', 'series', 'comps-add-on-cs-hub-from-comops')",
    );

    let r = post(
        &st,
        "/meetings/1/rename",
        &form(&[("name", "CS Hub sync")]),
        true,
    )
    .await;
    assert_eq!(r.headers["hx-redirect"], "/meetings/1");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meetings WHERE name = 'cs-hub-sync'"
        ),
        2
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM taggings WHERE item_type = 'series' AND item_id = 'cs-hub-sync'"
        ),
        1
    );

    // Taken by another series, or nothing usable: refused, nothing moves.
    let r = post(
        &st,
        "/meetings/1/rename",
        &form(&[("name", "oms-sync")]),
        true,
    )
    .await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("another series"));
    assert_eq!(
        post(&st, "/meetings/1/rename", &form(&[("name", " ?! ")]), true)
            .await
            .status,
        422
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meetings WHERE name = 'cs-hub-sync'"
        ),
        2
    );
}

#[tokio::test]
async fn migration_010_moves_notes_text_into_a_note() {
    // A pre-010 database: meetings with a notes text column only.
    let dir = std::env::temp_dir().join(format!("atharu-010-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("atharu.db");
    {
        let c = atharu::db::open(&path).unwrap();
        c.execute_batch(
            "ALTER TABLE meetings DROP COLUMN note_id; PRAGMA user_version = 9;
             INSERT INTO meetings(id, day, name, notes, ended_at) VALUES
               (1, '2026-11-10', 'oms-sync', 'ship friday #cshub', '2026-11-10 10:00:00'),
               (2, '2026-11-11', 'oms-sync', '', '2026-11-11 10:00:00');",
        )
        .unwrap();
    }
    let st = atharu::app::AppState::new(atharu::db::open(&path).unwrap());
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE title = 'oms-sync · 2026-11-10' AND body LIKE 'ship friday #cshub%[[oms-sync]]' AND author = 'me'"
        ),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meetings WHERE id = 1 AND note_id IS NOT NULL"
        ),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meetings WHERE id = 2 AND note_id IS NULL"
        ),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM taggings WHERE tag = 'cshub' AND item_type = 'note'"
        ),
        1
    );
    // The page edits it in place; an empty meeting offers "Start notes", which makes one once.
    let page = get(&st, "/meetings/1").await.body;
    assert!(page.contains("id=\"note-md\"") && page.contains("ship friday #cshub"));
    assert!(
        !page.contains("Mentioned in"),
        "its own note isn't a mention"
    );
    assert!(get(&st, "/meetings/2").await.body.contains("Start notes"));
    post(&st, "/meetings/2/note", "", false).await;
    post(&st, "/meetings/2/note", "", false).await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE title = 'oms-sync · 2026-11-11'"
        ),
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}
