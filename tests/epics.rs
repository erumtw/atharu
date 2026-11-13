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

// ---- 0.2.7: Epics view (filter, Group by, Show, Hide tracked), all from the Jira cache and the URL.

const VIEW_SEED: &str = "
    INSERT INTO epics(key, title, okr, status, tags, done_on) VALUES
      ('OPS-1', 'Checkout latency', 'O2', 'active', 'perf,checkout', NULL),
      ('OPS-2', 'Expense export', 'O1', 'active', 'expense', NULL),
      ('OPS-3', 'Cold chain', 'O1', 'active', '', NULL),
      ('OPS-4', 'Old thing', 'O2', 'done', '', '2026-11-01'),
      ('OPS-5', 'Dropped idea', 'O2', 'dropped', '', NULL),
      ('OPS-6', '', '', 'active', '', NULL),
      ('OPS-7', 'Plain epic', 'O1', 'active', '', NULL),
      ('OPS-8', 'Marked done in Jira', 'O2', 'active', '', NULL);
    INSERT INTO jira_issues(key, summary, status, status_cat, parent_key, parent_summary, assigned_me) VALUES
      ('OPS-1', 'Faster checkout', 'In Progress', 'indeterminate', 'OPS-90', 'Platform', 1),
      ('OPS-2', 'Expenses CSV', 'Waiting for UAT', 'indeterminate', 'OPS-90', 'Platform', 0),
      ('OPS-3', 'Cold-chain alerts', 'Backlog', 'new', NULL, NULL, 0),
      ('OPS-4', 'x', 'Done', 'done', NULL, NULL, 0),
      ('OPS-8', 'y', 'Done', 'done', NULL, NULL, 0),
      ('OPS-70', 'Assigned only', 'In Progress', 'indeterminate', 'OPS-90', 'Platform', 1),
      ('OPS-71', 'Assigned waiting', 'Waiting for UAT', 'indeterminate', NULL, NULL, 1),
      ('OPS-72', 'Assigned backlog', 'Backlog', 'new', 'OPS-91', '', 1);";

/// The group headers of the epics list, in page order (the assigned section and the folded list aren't groups).
fn group_labels(body: &str) -> Vec<String> {
    body.match_indices("<summary>")
        .filter_map(|(i, m)| {
            let rest = &body[i + m.len()..];
            let label = rest
                .split_once(" <span class=\"muted normal\">")
                .map(|(l, _)| l)?;
            (!label.starts_with("In Jira")
                && !label.starts_with("Done &amp;")
                && !label.contains('<'))
            .then(|| label.to_string())
        })
        .collect()
}

#[tokio::test]
async fn group_by_orders_groups_as_specified() {
    let st = state();
    exec(&st, VIEW_SEED);
    let labels = |q: &str| {
        let st = st.clone();
        let q = q.to_string();
        async move { group_labels(&get(&st, &format!("/epics{q}")).await.body) }
    };
    // OKR is today's page (and the default; an unknown value falls back to it).
    assert_eq!(labels("").await, ["O1", "O2"]);
    assert_eq!(labels("?group=okr").await, ["O1", "O2"]);
    assert_eq!(labels("?group=xyz").await, ["O1", "O2"]);
    // Status: in progress first (by name), then not started, then Jira's done, then epics not in Jira.
    assert_eq!(
        labels("?group=status").await,
        [
            "In Progress",
            "Waiting for UAT",
            "Backlog",
            "Done",
            "Not in Jira"
        ]
    );
    // Parent: by Jira parent (title · key); epics without one last.
    assert_eq!(
        labels("?group=parent").await,
        ["Platform · OPS-90", "No parent"]
    );
    // Tag: by tag name (an epic with two tags is in both), "No tag" last.
    assert_eq!(
        labels("?group=tag").await,
        ["checkout", "expense", "perf", "No tag"]
    );
    let b = get(&st, "/epics?group=tag").await.body;
    assert_eq!(b.matches("<td><a href=\"/epics/OPS-1\">").count(), 2);
    // None: one table for the epics (plus the Unnamed stub's own), no group header.
    let b = get(&st, "/epics?group=none").await.body;
    assert!(group_labels(&b).is_empty());
    assert_eq!(b.matches("<table class=\"etab\">").count(), 2);
    for k in ["OPS-1", "OPS-2", "OPS-3", "OPS-7", "OPS-8"] {
        assert!(b.contains(&format!("<a href=\"/epics/{k}\">")), "{k}");
    }
    // The fact line counts everything, whatever the view shows.
    assert!(
        b.contains("5 active · 1 done this month · 1 unnamed"),
        "{b}"
    );
}

#[tokio::test]
async fn filter_reads_key_title_jira_title_and_tags() {
    let st = state();
    exec(&st, VIEW_SEED);
    let keys = |b: &str| -> Vec<&'static str> {
        ["OPS-1", "OPS-2", "OPS-3", "OPS-7", "OPS-8"]
            .into_iter()
            .filter(|k| b.contains(&format!("<td><a href=\"/epics/{k}\">")))
            .collect()
    };
    for (q, want) in [
        ("OPS-3", vec!["OPS-3"]),      // key, any case
        ("latency", vec!["OPS-1"]),    // title
        ("cold-chain", vec!["OPS-3"]), // the Jira title
        ("perf", vec!["OPS-1"]),       // a tag
        ("expens", vec!["OPS-2"]),     // title + tag + Jira title, one row
        ("nothing-like-this", vec![]),
    ] {
        let b = get(&st, &format!("/epics?q={q}")).await.body;
        assert_eq!(keys(&b), want, "q={q}");
    }
    // The summary says how many of the group's epics show (O1 has 3 active).
    let b = get(&st, "/epics?q=expens").await.body;
    assert!(b.contains("· 1 of 3 active</span>"), "{b}");
    // The box keeps what was typed; the unfiltered page says "N active" plainly.
    assert!(!get(&st, "/epics").await.body.contains(" of 3 active"));
    // A D4 header row (OPS-90, not tracked) stays while one of its children matches, and goes when none does.
    let b = get(&st, "/epics?q=latency").await.body;
    assert!(b.contains("<td><span class=\"chip key\">OPS-90</span></td><td><span class=\"ettl\">"));
    let b = get(&st, "/epics?q=cold").await.body;
    assert!(!b.contains("class=\"jhead\""));
}

#[tokio::test]
async fn show_closed_lists_done_and_dropped_as_rows() {
    let st = state();
    exec(&st, VIEW_SEED);
    // Default: active only, and the folded "Done & dropped" list.
    let b = get(&st, "/epics").await.body;
    assert!(b.contains("Done &amp; dropped <span class=\"muted normal\">· 2</span>"));
    assert!(!b.contains("<td><a href=\"/epics/OPS-4\">"));
    // Closed: both as table rows with their status chip; no active rows, no folded list.
    let b = get(&st, "/epics?show=closed").await.body;
    assert!(
        b.contains("<td><a href=\"/epics/OPS-4\">") && b.contains("<td><a href=\"/epics/OPS-5\">")
    );
    assert!(
        b.contains("<span class=\"chip\">done 1 Nov</span>")
            && b.contains("<span class=\"chip\">dropped</span>")
    );
    assert!(!b.contains("<td><a href=\"/epics/OPS-1\">"));
    assert!(!b.contains("Done &amp; dropped <span"));
    // All: active and closed together.
    let b = get(&st, "/epics?show=all").await.body;
    assert!(
        b.contains("<td><a href=\"/epics/OPS-1\">") && b.contains("<td><a href=\"/epics/OPS-4\">")
    );
    assert!(
        !b.contains("· 5 active")
            && b.contains("<summary>O2 <span class=\"muted normal\">· 4</span>"),
        "{b}"
    );
    // An unknown value is the default.
    assert!(
        get(&st, "/epics?show=zzz")
            .await
            .body
            .contains("Done &amp; dropped <span")
    );
}

#[tokio::test]
async fn assigned_is_filtered_hidden_when_tracked_and_grouped() {
    let st = state();
    exec(&st, VIEW_SEED);
    let rows = |b: &str| -> Vec<String> {
        b.match_indices("<span class=\"t\" title=\"")
            .map(|(i, m)| b[i + m.len()..].split('"').next().unwrap().to_string())
            .collect()
    };
    let b = get(&st, "/epics").await.body;
    // By status, in progress first; the section says "N open".
    assert_eq!(
        rows(&b),
        [
            "Faster checkout",
            "Assigned only",
            "Assigned waiting",
            "Assigned backlog"
        ]
    );
    assert!(b.contains("In Jira, assigned to you <span class=\"muted normal\">· 4 open</span>"));
    // Hide tracked drops OPS-1 (an epic here): "3 of 4 open".
    let b = get(&st, "/epics?hide_tracked=1").await.body;
    assert_eq!(
        rows(&b),
        ["Assigned only", "Assigned waiting", "Assigned backlog"]
    );
    assert!(b.contains("· 3 of 4 open</span>"));
    // The filter reads key + Jira title.
    let b = get(&st, "/epics?q=ops-71").await.body;
    assert_eq!(rows(&b), ["Assigned waiting"]);
    let b = get(&st, "/epics?q=backlog").await.body;
    assert_eq!(rows(&b), ["Assigned backlog"]);
    // Group by Parent groups them by Jira parent ("title · key", just the key without a title), "No parent" last.
    let b = get(&st, "/epics?group=parent").await.body;
    let heads: Vec<&str> = b
        .match_indices("<summary class=\"small\">")
        .map(|(i, m)| b[i + m.len()..].split(" <span").next().unwrap())
        .collect();
    assert_eq!(heads, ["OPS-91", "Platform · OPS-90", "No parent"]);
}

#[tokio::test]
async fn the_view_row_marks_the_current_pick_and_keeps_the_other_params() {
    let st = state();
    exec(&st, VIEW_SEED);
    // Defaults: OKR and Active are pressed; no hidden params anywhere.
    let b = get(&st, "/epics").await.body;
    assert!(b.contains("<form class=\"fld s\" method=\"get\" action=\"/epics\" role=\"search\""));
    assert!(b.contains("<button class=\"on\" aria-pressed=\"true\" type=\"submit\" name=\"group\" value=\"okr\">OKR</button>"));
    assert!(b.contains("<button class=\"on\" aria-pressed=\"true\" type=\"submit\" name=\"show\" value=\"active\">Active</button>"));
    assert!(
        b.contains("placeholder=\"Filter epics\"") && !b.contains("type=\"hidden\" name=\"group\"")
    );

    // A pick elsewhere: the current one is pressed, and each form carries the other two params.
    let b = get(&st, "/epics?q=expens&group=status&show=all&hide_tracked=1")
        .await
        .body;
    assert!(b.contains(
        "aria-pressed=\"true\" type=\"submit\" name=\"group\" value=\"status\">Status</button>"
    ));
    assert!(b.contains(
        "aria-pressed=\"true\" type=\"submit\" name=\"show\" value=\"all\">All</button>"
    ));
    assert!(b.contains("name=\"q\" type=\"search\" value=\"expens\""));
    // Group by keeps q, show and hide_tracked; Show keeps q, group and hide_tracked; the filter keeps group, show, hide_tracked.
    let form = |label: &str| {
        let i = b
            .find(&format!("aria-label=\"{label}\""))
            .or_else(|| b.find(&format!("role=\"{label}\"")))
            .unwrap();
        let j = b[i..].find("</form>").unwrap();
        b[i..i + j].to_string()
    };
    let g = form("Group by");
    assert!(
        g.contains("name=\"q\" value=\"expens\"")
            && g.contains("name=\"show\" value=\"all\"")
            && g.contains("name=\"hide_tracked\" value=\"1\"")
    );
    assert!(!g.contains("type=\"hidden\" name=\"group\""));
    let s = form("Show");
    assert!(
        s.contains("name=\"q\" value=\"expens\"")
            && s.contains("name=\"group\" value=\"status\"")
            && s.contains("name=\"hide_tracked\" value=\"1\"")
    );
    assert!(!s.contains("type=\"hidden\" name=\"show\""));
    let f = form("search");
    assert!(
        f.contains("name=\"group\" value=\"status\"")
            && f.contains("name=\"show\" value=\"all\"")
            && f.contains("name=\"hide_tracked\" value=\"1\"")
    );
    // The Hide tracked box is checked and belongs to its own form (which keeps q, group, show).
    assert!(b.contains(
        "type=\"checkbox\" name=\"hide_tracked\" value=\"1\" form=\"hidetracked\" checked"
    ));
    let h = b.find("<form id=\"hidetracked\"").unwrap();
    let h = &b[h..h + b[h..].find("</form>").unwrap()];
    assert!(
        h.contains("name=\"q\" value=\"expens\"")
            && h.contains("name=\"group\" value=\"status\"")
            && h.contains("name=\"show\" value=\"all\"")
    );
}

#[tokio::test]
async fn many_assigned_rows_start_folded_unless_filtering() {
    let st = state();
    let mut sql = String::new();
    for i in 0..12 {
        sql += &format!(
            "INSERT INTO jira_issues(key, summary, status, status_cat, assigned_me) VALUES ('ZZ-{i}', 'Task {i}', 'To do', 'new', 1);"
        );
    }
    exec(&st, &sql);
    let b = get(&st, "/epics").await.body;
    assert!(
        b.contains("<details id=\"assigned\">") && b.contains("· 12 open"),
        "folded when > 10"
    );
    // Filtering (or Hide tracked) opens it.
    assert!(
        get(&st, "/epics?q=task")
            .await
            .body
            .contains("<details id=\"assigned\" open>")
    );
    assert!(
        get(&st, "/epics?hide_tracked=1")
            .await
            .body
            .contains("<details id=\"assigned\" open>")
    );
    // Few rows: open as before.
    exec(&st, "DELETE FROM jira_issues WHERE key > 'ZZ-3'");
    assert!(
        get(&st, "/epics")
            .await
            .body
            .contains("<details id=\"assigned\" open>")
    );
}
