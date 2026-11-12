mod common;
use common::*;

/// Today = Wed 11 Nov 2026.
fn seed(st: &atharu::app::AppState) {
    exec(
        st,
        "INSERT INTO epics(key, title, target) VALUES
           ('OPS-092', 'Log retention', '2026-11-05'),
           ('OPS-101', 'Checkout latency', '2026-11-14'),
           ('OPS-118', 'Reorder from history', '2026-11-30'),
           ('OPS-120', 'Quiet but fine', NULL);
         INSERT INTO epics(key, title, status, target) VALUES ('OPS-087', 'Done one', 'done', '2026-10-01');
         INSERT INTO entries(day, epic_key, kind, text, plan_state, due) VALUES
           ('2026-11-09', 'OPS-092', 'done', 'cleanup', NULL, NULL),
           ('2026-11-11', 'OPS-101', 'plan', 'profile join', 'open', NULL),
           ('2026-11-02', 'OPS-118', 'done', 'spike', NULL, NULL),
           ('2026-11-08', 'OPS-120', 'blocker', 'blockers still count as activity', NULL, NULL),
           ('2026-11-01', NULL, 'plan', 'send runbook', 'open', '2026-11-09');",
    );
}

#[tokio::test]
async fn needs_you_lists_flags_and_overdue_todos() {
    let st = state();
    seed(&st);
    let b = get(&st, "/").await.body;
    let pos = |s: &str| b.find(s).unwrap_or_else(|| panic!("missing {s}"));
    assert!(pos("<b>Needs you</b>") < pos("Logged today"));
    assert!(
        pos("Log retention · 6 days past target")
            < pos("Checkout latency · due in 3 days, 1 plan open")
    );
    assert!(pos("due in 3 days") < pos("Reorder from history · nothing logged for 9 days"));
    assert!(b.contains("send runbook · was due Mon 9 Nov"));
    assert!(!b.contains("Quiet but fine ·"));
    assert!(!b.contains("Done one"));
}

#[tokio::test]
async fn epics_column_and_page_show_flags() {
    let st = state();
    seed(&st);
    let b = get(&st, "/epics").await.body;
    assert!(b.contains(r#"<span class="flag over">overdue</span> <span class="small muted">6 days past target</span>"#));
    assert!(b.contains(r#"<span class="flag stale">stale</span>"#));
    let b = get(&st, "/epics/OPS-101").await.body;
    assert!(b.contains(r#"<span class="flag risk">at risk</span>"#));
    // One line this week: the last of the 8 bars is the full one.
    assert!(b.contains(r#"<span style="height: 0%"></span><span style="height: 100%"></span></div><span class="small muted">logs/week, 8 wks</span>"#));
}

#[tokio::test]
async fn nothing_flagged_hides_needs_you() {
    let st = state();
    assert!(!get(&st, "/").await.body.contains("Needs you"));
}
