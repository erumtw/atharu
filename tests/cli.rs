mod common;
use common::*;

/// Runs the CLI on the test state's DB: (exit code, stdout, stderr).
fn cli(
    st: &atharu::app::AppState,
    args: &[&str],
    env_agent: Option<&str>,
) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = atharu::cli::run(
        &mut st.conn(),
        TODAY.parse().unwrap(),
        &args,
        env_agent,
        &mut out,
        &mut err,
    );
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[tokio::test]
async fn log_tags_the_agent_and_refuses_bad_callers() {
    let st = state();
    let (code, out, _) = cli(
        &st,
        &["log", "OPS-101 + p95 1.8s → 0.6s", "--as", "claude"],
        None,
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["kind"], "impact");
    assert_eq!(v["epic"]["key"], "OPS-101");
    assert_eq!(v["epic"]["isNew"], true);
    assert_eq!(v["author"], "claude");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE author = 'claude' AND day = '2026-11-11'"
        ),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM epics WHERE key = 'OPS-101' AND author = 'claude'"
        ),
        1
    );

    // ATHARU_AGENT stands in for --as; --day backfills; unquoted words are one line.
    let (code, out, _) = cli(
        &st,
        &[
            "log",
            ">",
            "write",
            "runbook",
            "by",
            "fri",
            "--day=2026-11-10",
        ],
        Some("codex"),
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"author\":\"codex\"") && out.contains("\"due\":\"2026-11-13\""));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE author = 'codex' AND day = '2026-11-10' AND plan_state = 'open'"
        ),
        1
    );

    let (code, _, err) = cli(&st, &["log", "shipped it"], None);
    assert_eq!(
        (code, err.as_str()),
        (2, "Say who is writing: --as claude\n")
    );
    let (code, _, err) = cli(&st, &["log", "shipped it", "--as", "me"], None);
    assert_eq!(code, 2);
    assert!(err.contains("--as me is refused"));
    let (code, _, _) = cli(&st, &["log", "shipped it"], Some("me"));
    assert_eq!(code, 2, "env can't pose as me either");
    let (code, _, err) = cli(&st, &["log", "OPS-101 +", "--as", "claude"], None);
    assert_eq!(
        (code, err.as_str()),
        (1, "Nothing after “+”. Type what happened.\n")
    );
    let (code, _, err) = cli(&st, &["log", "> x by someday", "--as", "claude"], None);
    assert_eq!((code, err.as_str()), (1, "Can't read date ‘someday’\n"));
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM entries"),
        2,
        "rejected lines save nothing"
    );

    // Badge on Today, the epic timeline, and `(via claude)` in the report note + markdown.
    let b = get(&st, "/").await.body;
    assert!(b.contains("<span class=\"chip llm\" title=\"Filed by claude\">claude</span>"));
    post(
        &st,
        "/entries",
        &form(&[("line", "OPS-101 ! keep sync path")]),
        false,
    )
    .await;
    let b = get(&st, "/epics/OPS-101").await.body;
    assert_eq!(b.matches("chip llm").count(), 1, "my own line has no badge");
    let b = get(&st, "/report").await.body;
    assert!(b.contains("p95 1.8s → 0.6s <span class=\"muted\">(via claude)</span>"));
    assert!(b.contains("**Impact** p95 1.8s → 0.6s (via claude)"));
    assert!(b.contains("**Decision** keep sync path") && !b.contains("keep sync path (via"));
}

#[tokio::test]
async fn epic_upsert_never_overwrites_and_reads_back() {
    let st = state();
    let url = "https://acme.atlassian.net/browse/OPS-140?focusedCommentId=1";
    let (code, out, _) = cli(
        &st,
        &[
            "epic",
            url,
            "--title",
            "Retention runbook",
            "--okr",
            "O2 · Reliability",
            "--as",
            "claude",
        ],
        None,
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (v["key"].as_str(), v["created"].as_bool()),
        (Some("OPS-140"), Some(true))
    );
    assert_eq!(v["title"], "Retention runbook");

    // My fields stay; only an empty one gets filled.
    exec(
        &st,
        "INSERT INTO epics(key, title) VALUES ('OPS-101', 'Checkout latency')",
    );
    let (code, out, _) = cli(
        &st,
        &[
            "epic",
            "OPS-101",
            "--title",
            "Agent title",
            "--okr",
            "O1",
            "--as",
            "claude",
        ],
        None,
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (v["title"].as_str(), v["okr"].as_str()),
        (Some("Checkout latency"), Some("O1"))
    );
    assert_eq!(
        (v["author"].as_str(), v["created"].as_bool()),
        (Some("me"), Some(false))
    );

    let (code, _, err) = cli(
        &st,
        &["epic", "https://example.com/nothing", "--as", "claude"],
        None,
    );
    assert_eq!(code, 1);
    assert!(err.starts_with("Can't find an epic key like ABC-123"));
    assert_eq!(cli(&st, &["epic", "OPS-1"], None).0, 2, "writes need --as");

    cli(
        &st,
        &["log", "OPS-101 shipped index", "--as", "claude"],
        None,
    );
    exec(
        &st,
        "INSERT INTO evidence(epic_key, url) VALUES ('OPS-101', 'https://x.io/pr/1')",
    );
    let (code, out, _) = cli(&st, &["show", "OPS-101"], None);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["timeline"][0]["text"], "shipped index");
    assert_eq!(v["timeline"][0]["author"], "claude");
    assert_eq!(v["evidence"][0]["url"], "https://x.io/pr/1");
    assert_eq!(cli(&st, &["show", "NOPE-1"], None).0, 1);

    let (code, out, _) = cli(&st, &["today"], None);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["day"], TODAY);
    assert_eq!(v["entries"][0]["epic"]["title"], "Checkout latency");
    assert_eq!(
        cli(&st, &["today", "--as", "claude"], None).0,
        2,
        "unknown flag for a read"
    );
    assert_eq!(cli(&st, &["nope"], None).0, 2);
}
