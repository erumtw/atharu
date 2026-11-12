mod common;
use common::*;

/// Runs the CLI on the test state's DB: (exit code, stdout, stderr).
fn cli(
    st: &atharu::app::AppState,
    args: &[&str],
    env_agent: Option<&str>,
) -> (i32, String, String) {
    cli_in(st, args, env_agent, "")
}

/// Same, with `stdin` piped in.
fn cli_in(
    st: &atharu::app::AppState,
    args: &[&str],
    env_agent: Option<&str>,
    stdin: &str,
) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = atharu::cli::run(
        &mut st.conn(),
        TODAY.parse().unwrap(),
        &args,
        env_agent,
        &mut stdin.as_bytes(),
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
    let (code, _, err) = cli(&st, &["log", "> x by 32 Nov", "--as", "claude"], None);
    assert_eq!((code, err.as_str()), (1, "Can't read date ‘32 Nov’\n"));
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

#[tokio::test]
async fn note_and_evidence_write_only_as_the_agent() {
    let st = state();
    exec(
        &st,
        "INSERT INTO epics(key) VALUES ('OPS-101');
         INSERT INTO notes(id, title, body) VALUES (1, 'Mine', 'my words');",
    );
    let (code, out, _) = cli_in(
        &st,
        &[
            "note",
            "Vector search primer",
            "--links",
            "vector-search, OPS-101",
            "--as",
            "claude",
        ],
        None,
        "Summary of the post.\n",
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let id = v["id"].as_i64().unwrap();
    let (author, body): (String, String) = st
        .conn()
        .query_row("SELECT author, body FROM notes WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(author, "claude");
    assert_eq!(
        body,
        "Summary of the post.\n\nLinks: [[vector-search]] · [[OPS-101]]"
    );

    // --file + --id rewrites its own note; my note and another agent's stay untouched.
    let path = std::env::temp_dir().join(format!("atharu-note-{}.md", std::process::id()));
    std::fs::write(&path, "v2").unwrap();
    let file = path.to_str().unwrap();
    let ids = id.to_string();
    let (code, out, _) = cli(
        &st,
        &[
            "note",
            "Primer v2",
            "--file",
            file,
            "--id",
            &ids,
            "--as",
            "claude",
        ],
        None,
    );
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        count(
            &st,
            &format!("SELECT COUNT(*) FROM notes WHERE id = {id} AND body = 'v2'")
        ),
        1
    );
    let (code, _, err) = cli(
        &st,
        &[
            "note", "Hijack", "--file", file, "--id", "1", "--as", "claude",
        ],
        None,
    );
    assert_eq!(code, 1);
    assert!(err.contains("Note 1 isn't one claude wrote"), "{err}");
    let (code, _, _) = cli(
        &st,
        &["note", "X", "--file", file, "--id", &ids, "--as", "codex"],
        None,
    );
    assert_eq!(code, 1, "codex can't rewrite claude's note");
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE id = 1 AND body = 'my words'"
        ),
        1
    );
    assert_eq!(cli(&st, &["note", "No agent"], None).0, 2);

    // Evidence: http(s) only (web copy), known epic only, badge on the epic page.
    let (code, _, err) = cli(
        &st,
        &[
            "evidence",
            "OPS-101",
            "javascript:alert(1)",
            "--as",
            "claude",
        ],
        None,
    );
    assert_eq!(
        (code, err.trim()),
        (1, "Paste a link that starts with http:// or https://.")
    );
    let (code, _, err) = cli(
        &st,
        &["evidence", "OPS-999", "https://x.io/pr/1", "--as", "claude"],
        None,
    );
    assert_eq!(code, 1);
    assert!(err.contains("No epic OPS-999 yet"));
    let (code, out, _) = cli(
        &st,
        &[
            "evidence",
            "https://acme.atlassian.net/browse/OPS-101",
            "https://github.com/acme/pr/7",
            "--label",
            "the PR",
            "--as",
            "claude",
        ],
        None,
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (v["epic"]["key"].as_str(), v["author"].as_str()),
        (Some("OPS-101"), Some("claude"))
    );
    let page = get(&st, "/epics/OPS-101").await.body;
    assert!(
        page.contains("https://github.com/acme/pr/7") && page.contains("title=\"Filed by claude\"")
    );
}

#[tokio::test]
async fn meeting_reads_the_transcript_and_log_files_under_it() {
    let st = state();
    exec(
        &st,
        "INSERT INTO meetings(id, day, name, ended_at) VALUES (1, '2026-11-11', 'oms-sync', '2026-11-11 10:30:00');
         INSERT INTO meetings(id, day, name) VALUES (2, '2026-11-11', 'vendor-call');
         INSERT INTO meeting_transcripts(meeting_id, started_at, ended_at, status, model, lang, segments)
         VALUES (1, '2026-11-11 10:00:00', '2026-11-11 10:30:00', 'done', 'large-v3-turbo', 'th',
           '[{\"t\":20,\"speaker\":\"me\",\"text\":\"we ship on friday then\"},
             {\"t\":3,\"speaker\":\"others\",\"text\":\"we ship on friday then\"},
             {\"t\":9,\"speaker\":\"me\",\"text\":\"sounds good\"}]');",
    );
    let (code, out, _) = cli(&st, &["meeting", "1"], None);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["name"], "oms-sync");
    // Time order, and the mic's copy of the call's line is gone.
    let segs: Vec<(&str, &str)> = v["transcripts"][0]["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["speaker"].as_str().unwrap(), s["text"].as_str().unwrap()))
        .collect();
    assert_eq!(
        segs,
        [("others", "we ship on friday then"), ("me", "sounds good")]
    );
    assert_eq!(cli(&st, &["meeting", "latest"], None).0, 0);
    assert_eq!(cli(&st, &["meeting", "9"], None).0, 1);

    // Meeting 2 is running, but --meeting 1 files the decision under the ended one.
    let (code, out, _) = cli(
        &st,
        &[
            "log",
            "! ship on friday",
            "--meeting",
            "1",
            "--as",
            "claude",
        ],
        None,
    );
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE meeting_id = 1 AND kind = 'decision' AND author = 'claude'"
        ),
        1
    );
    // A missing meeting files nothing.
    assert_eq!(
        cli(
            &st,
            &["log", "! nope", "--meeting", "9", "--as", "claude"],
            None
        )
        .0,
        1
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM entries WHERE text = 'nope'"),
        0
    );
}

#[tokio::test]
async fn edit_rewrites_only_the_agents_own_lines() {
    let st = state();
    let (_, out, _) = cli(
        &st,
        &["log", "> Frank to check pre-order by fri", "--as", "claude"],
        None,
    );
    let id = serde_json::from_str::<serde_json::Value>(&out).unwrap()["id"]
        .as_i64()
        .unwrap();
    let (code, out, err) = cli(
        &st,
        &[
            "edit",
            &id.to_string(),
            "> Off to check pre-order by fri",
            "--as",
            "claude",
        ],
        None,
    );
    assert_eq!(code, 0, "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["text"], "Off to check pre-order");
    assert_eq!(
        count(
            &st,
            &format!(
                "SELECT COUNT(*) FROM entries WHERE id = {id} AND text = 'Off to check pre-order' AND kind = 'plan'"
            )
        ),
        1
    );

    // The owner's line and another agent's are refused, and stay as they were.
    exec(
        &st,
        "INSERT INTO entries(id, day, kind, text, author) VALUES (90, '2026-11-11', 'done', 'mine', 'me')",
    );
    let (code, _, err) = cli(&st, &["edit", "90", "not mine", "--as", "claude"], None);
    assert_eq!(code, 1);
    assert!(err.contains("written by me"), "{err}");
    assert_eq!(
        cli(&st, &["edit", &id.to_string(), "x", "--as", "codex"], None).0,
        1
    );
    assert_eq!(cli(&st, &["edit", "999", "x", "--as", "claude"], None).0, 1);
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE id = 90 AND text = 'mine'"
        ),
        1
    );
}
