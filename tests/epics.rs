mod common;
use common::*;

fn seed(st: &atharu::app::AppState) {
    exec(
        st,
        "INSERT INTO epics(key, title, okr, target) VALUES
           ('OPS-101', 'Checkout latency', 'O1 · Faster ordering', '2026-11-14'),
           ('OPS-092', 'Log retention', '', '2026-11-05');
         INSERT INTO epics(key) VALUES ('OPS-140');
         INSERT INTO epics(key, title, status, done_on) VALUES ('OPS-087', 'Cart autosave', 'done', '2026-10-20');
         INSERT INTO entries(day, epic_key, kind, text) VALUES ('2026-11-02', 'OPS-092', 'done', 'old work'),
           ('2026-11-11', 'OPS-101', 'impact', 'p95 down');",
    );
}

#[tokio::test]
async fn list_groups_by_okr_with_stubs_and_closed() {
    let st = state();
    seed(&st);
    let b = get(&st, "/epics").await.body;
    let pos = |s: &str| b.find(s).unwrap_or_else(|| panic!("missing {s}"));
    assert!(
        b.contains(r#"<p class="fact">2 active · 1 unnamed</p>"#),
        "{b}"
    );
    assert!(pos("O1 · Faster ordering") < pos("No OKR"));
    assert!(pos("No OKR") < pos("Done &amp; dropped"));
    // Stubs last, named in place.
    assert!(pos("Done &amp; dropped") < pos("Unnamed <span"));
    assert!(b.contains("Title for OPS-140"));
    assert!(b.contains("9d ago"));
    assert!(b.contains("done 20 Oct"));
}

#[tokio::test]
async fn inline_title_sets_title() {
    let st = state();
    seed(&st);
    let r = post(
        &st,
        "/epics/OPS-140/title",
        &form(&[("title", " Staging access ")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("✓ saved"));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key='OPS-140' AND title='Staging access'"
        ),
        1
    );
    let r = post(&st, "/epics/OPS-140/title", &form(&[("title", "x")]), false).await;
    assert_eq!(r.status, 303);
    assert_eq!(
        post(&st, "/epics/NOPE-1/title", &form(&[("title", "x")]), true)
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn field_patch_persists_only_that_field() {
    let st = state();
    seed(&st);
    let r = request(
        &st,
        "PATCH",
        "/epics/OPS-101",
        &form(&[("target", "2026-11-20")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("✓ saved"));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key='OPS-101' AND target='2026-11-20' AND okr='O1 · Faster ordering'"
        ),
        1
    );

    request(
        &st,
        "PATCH",
        "/epics/OPS-101",
        &form(&[("target", "")]),
        true,
    )
    .await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key='OPS-101' AND target IS NULL"
        ),
        1
    );

    let r = request(
        &st,
        "PATCH",
        "/epics/OPS-101",
        &form(&[("role", "boss")]),
        true,
    )
    .await;
    assert_eq!(r.status, 422);
    assert!(
        r.body.contains("Couldn&#39;t save. Try again.")
            || r.body.contains("Couldn't save. Try again.")
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key='OPS-101' AND role='owner'"
        ),
        1
    );

    let r = post(
        &st,
        "/epics/OPS-101",
        &form(&[("status", "done"), ("done_on", "2026-11-11")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key='OPS-101' AND status='done'"
        ),
        1
    );
}

#[tokio::test]
async fn done_without_a_date_is_stamped_today_so_the_report_counts_it() {
    let st = state();
    seed(&st);
    let shipped = |st: &atharu::app::AppState, key: &str, on: &str| {
        count(
            st,
            &format!(
                "SELECT COUNT(*) FROM epics WHERE key='{key}' AND status='done' AND done_on='{on}'"
            ),
        )
    };
    // htmx autosave posts the whole form: status flips, "done on" is still empty.
    let r = request(
        &st,
        "PATCH",
        "/epics/OPS-101",
        &form(&[("status", "done"), ("done_on", "")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(shipped(&st, "OPS-101", TODAY), 1);
    // The input sits outside the swapped block, so the answer carries the filled date.
    assert!(
        r.body
            .contains("id=\"done_on\" name=\"done_on\" value=\"2026-11-11\""),
        "{}",
        r.body
    );
    assert!(
        get(&st, "/report")
            .await
            .body
            .contains("- Did Checkout latency")
    );

    // A date that was typed stays; status alone (no done_on field) stamps too.
    post(
        &st,
        "/epics/OPS-092",
        &form(&[("status", "done"), ("done_on", "2026-11-03")]),
        false,
    )
    .await;
    assert_eq!(shipped(&st, "OPS-092", "2026-11-03"), 1);
    post(&st, "/epics/OPS-140", &form(&[("status", "done")]), false).await;
    assert_eq!(shipped(&st, "OPS-140", TODAY), 1);

    // Not done, or already done with a date: nothing is stamped or moved.
    request(
        &st,
        "PATCH",
        "/epics/OPS-087",
        &form(&[("status", "done"), ("done_on", "2026-10-20")]),
        true,
    )
    .await;
    assert_eq!(shipped(&st, "OPS-087", "2026-10-20"), 1);
    exec(&st, "INSERT INTO epics(key) VALUES ('OPS-200')");
    post(&st, "/epics/OPS-200", &form(&[("okr", "O2")]), false).await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key='OPS-200' AND done_on IS NULL"
        ),
        1
    );
}

#[tokio::test]
async fn target_before_start_is_refused() {
    let st = state();
    seed(&st);
    let msg = "Target can&#39;t be before the start.";
    // Start alone after the stored target (14 Nov) is backwards too.
    let r = request(
        &st,
        "PATCH",
        "/epics/OPS-101",
        &form(&[("start", "2026-11-20")]),
        true,
    )
    .await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains(msg), "{}", r.body);
    let r = post(
        &st,
        "/epics/OPS-101",
        &form(&[("start", "2026-11-01"), ("target", "2026-10-30")]),
        false,
    )
    .await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains(msg));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key='OPS-101' AND start IS NULL AND target='2026-11-14'"
        ),
        1
    );
    // Same day is fine; the picker gets the bounds.
    let r = request(
        &st,
        "PATCH",
        "/epics/OPS-101",
        &form(&[("start", "2026-11-14"), ("target", "2026-11-14")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    let b = get(&st, "/epics/OPS-101").await.body;
    assert!(
        b.contains(r#"name="start" value="2026-11-14" max="2026-11-14""#),
        "{b}"
    );
    assert!(b.contains(r#"name="target" value="2026-11-14" min="2026-11-14""#));
}

#[tokio::test]
async fn evidence_add_and_delete() {
    let st = state();
    seed(&st);
    let r = post(
        &st,
        "/epics/OPS-101/evidence",
        &form(&[("line", "https://github.com/o/r/pull/412 index migration")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains(r#"href="https://github.com/o/r/pull/412""#));
    assert!(r.body.contains("index migration"));

    let r = post(
        &st,
        "/epics/OPS-101/evidence",
        &form(&[("line", "javascript:alert(1)")]),
        true,
    )
    .await;
    assert_eq!(r.status, 422);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM evidence"), 1);

    let id = count(&st, "SELECT id FROM evidence");
    assert_eq!(
        request(
            &st,
            "DELETE",
            &format!("/epics/OPS-092/evidence/{id}"),
            "",
            true
        )
        .await
        .status,
        404
    );
    let r = request(
        &st,
        "DELETE",
        &format!("/epics/OPS-101/evidence/{id}"),
        "",
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM evidence"), 0);
}

#[tokio::test]
async fn impact_line_text_exact() {
    let st = state();
    seed(&st);
    let b = get(&st, "/epics/OPS-101").await.body;
    assert!(b.contains("Fill Metric, Before and After on this page to build your impact line."));
    assert!(b.contains("p95 down"));
    assert!(b.contains("<h4 class=\"muted small\">Today</h4>"));

    exec(&st, "UPDATE epics SET metric='checkout p95', before='1.8s', after='0.6s' WHERE key='OPS-101';
               INSERT INTO evidence(epic_key, url) VALUES ('OPS-101', 'https://gh.io/412'), ('OPS-101', 'https://graf.io/p95');");
    let b = get(&st, "/epics/OPS-101").await.body;
    assert!(
        b.contains(r#"<div id="impact-line" class="mono muted">Did Checkout latency → checkout p95 1.8s→0.6s → https://gh.io/412, https://graf.io/p95</div>"#),
        "{b}"
    );
    assert_eq!(get(&st, "/epics/NOPE-1").await.status, 404);
}

#[tokio::test]
async fn jira_refresh_says_it_ran_and_how_old_the_copy_is() {
    let mut st = state();
    // Refresh only shows with Jira set up: a throwaway config file in the temp dir.
    let path = std::env::temp_dir().join(format!("atharu-jira-{}.toml", std::process::id()));
    std::fs::write(
        &path,
        "jira_base_url = \"https://acme.atlassian.net\"\njira_email = \"me@example.com\"\njira_token = \"tok\"\n",
    )
    .unwrap();
    st.settings = std::sync::Arc::new(atharu::config::FileConfig::load(&path));
    let _ = std::fs::remove_file(&path);
    exec(
        &st,
        "INSERT INTO jira_issues(key, summary, status) VALUES ('OPS-1', 'One', 'Done')",
    );
    let b = get(&st, "/epics?jira=ok").await.body;
    assert!(b.contains("✓ Refreshed from Jira"), "{b}");
    assert!(b.contains("updated just now"));
    assert!(
        !get(&st, "/epics")
            .await
            .body
            .contains("Refreshed from Jira")
    );
}
