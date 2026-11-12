//! Meeting recording (T53) as the server sees it: the app's segments land, pages show them, the
//! browser only watches. The Mac side is faked by a `Mac` over a temp models dir.
mod common;
use atharu::{app::AppState, recording::Mac};
use axum::{body::Body, http::Request};
use common::*;
use std::{path::PathBuf, sync::Arc, time::Duration};

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("atharu-t53-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The app's state: a Mac with (or without) the speech model on disk.
fn mac_state(name: &str, model: bool) -> (AppState, Arc<Mac>) {
    let mut st = state();
    let mac = Arc::new(Mac::new(tmp_dir(name)));
    if model {
        std::fs::write(mac.model_path(), b"fake model").unwrap();
    }
    st.mac = Some(mac.clone());
    (st, mac)
}

async fn post_json(st: &AppState, uri: &str, json: &str) -> Resp {
    let req = Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(json.to_string()))
        .unwrap();
    send(atharu::app::router(st.clone()), req).await
}

fn segs(st: &AppState, tid: i64) -> String {
    st.conn()
        .query_row(
            "SELECT segments FROM meeting_transcripts WHERE id = ?1",
            [tid],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn migration_009_adds_meeting_transcripts() {
    let st = state();
    assert_eq!(count(&st, "PRAGMA user_version"), 9);
    exec(
        &st,
        "INSERT INTO meetings(day, name) VALUES ('2026-11-11', 'oms-sync');
         INSERT INTO meeting_transcripts(meeting_id, started_at, model, lang)
         VALUES (1, '2026-11-11 10:00:00', 'large-v3-turbo', 'th');",
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meeting_transcripts WHERE status = 'live' AND segments = '[]'"
        ),
        1
    );
    // One recorder: a second running recording is refused by the index.
    assert!(
        st.conn()
            .execute(
                "INSERT INTO meeting_transcripts(meeting_id, started_at, model, lang, status)
                 VALUES (1, '2026-11-11 10:05:00', 'x', 'th', 'finishing')",
                []
            )
            .is_err()
    );
    exec(&st, "DELETE FROM meetings");
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meeting_transcripts"), 0);
}

#[tokio::test]
async fn record_switch_only_in_the_app_and_only_with_a_model() {
    // Browser: no switch, and a hand-made record=1 records nothing.
    let st = state();
    assert!(!get(&st, "/").await.body.contains("Record audio"));
    let r = post(
        &st,
        "/meetings",
        &form(&[("name", "oms"), ("record", "1")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meeting_transcripts"), 0);

    // App without the model: the switch is there but disabled, with the way to the download.
    let (st, _) = mac_state("nomodel", false);
    let b = get(&st, "/").await.body;
    assert!(b.contains("name=\"record\" value=\"1\" form=\"start-meeting\" disabled"));
    assert!(b.contains("Needs the speech model (574 MB)."));
    post(
        &st,
        "/meetings",
        &form(&[("name", "oms"), ("record", "1")]),
        false,
    )
    .await;
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meeting_transcripts"), 0);

    // App with the model: off by default, consent copy ready; ticked → a live recording.
    let (st, mac) = mac_state("model", true);
    let b = get(&st, "/").await.body;
    assert!(b.contains(
        "name=\"record\" value=\"1\" form=\"start-meeting\" hx-post=\"/meetings/permissions\""
    ));
    assert!(b.contains("Tell people you’re recording before you start."));
    post(
        &st,
        "/meetings",
        &form(&[("name", "oms"), ("record", "1")]),
        false,
    )
    .await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meeting_transcripts WHERE status = 'live' AND lang = 'th'"
        ),
        1
    );
    *mac.levels.lock().unwrap() = [4, 0];
    let b = get(&st, "/").await.body;
    assert!(b.contains("<b>Recording</b>"));
    assert!(b.contains(
        "aria-label=\"Me level\" aria-valuemin=\"0\" aria-valuemax=\"6\" aria-valuenow=\"4\""
    ));
    assert!(b.contains("Stop recording"));
    assert!(b.contains("hx-trigger=\"every 1s\""));
    // The meeting page carries the banner too while it records.
    assert!(
        get(&st, "/meetings/1")
            .await
            .body
            .contains("Stop recording")
    );

    // Stop: the meeting keeps running, the tail finishes; the banner says so and offers no Stop.
    let r = post(
        &st,
        "/meetings/1/record/stop",
        &form(&[("back", "/")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("transcribing the last part…"));
    assert!(!r.body.contains("Stop recording"));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meeting_transcripts WHERE status = 'finishing' AND ended_at IS NOT NULL"
        ),
        1
    );
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM meetings WHERE ended_at IS NULL"),
        1
    );
    let _ = std::fs::remove_dir_all(&mac.dir);
}

#[tokio::test]
async fn segments_append_dedupe_and_close() {
    let (st, mac) = mac_state("segs", true);
    post(
        &st,
        "/meetings",
        &form(&[("name", "oms"), ("record", "1")]),
        false,
    )
    .await;
    let one = r#"{"segments":[{"t":19.04,"speaker":"others","text":"ได้ครับ deploy service ใหม่แล้ว"},
                              {"t":4.0,"speaker":"me","text":" โอเค เริ่มเลย "}]}"#;
    assert_eq!(
        post_json(&st, "/meetings/transcripts/1/segments", one)
            .await
            .status,
        204
    );
    // A retried post (or the overlap repeating a line) doesn't double it.
    assert_eq!(
        post_json(&st, "/meetings/transcripts/1/segments", one)
            .await
            .status,
        204
    );
    let two = r#"{"segments":[{"t":19.0,"speaker":"others","text":"dup by start"},
                              {"t":62.0,"speaker":"me","text":"Do we have a rollback plan?"},
                              {"t":70.0,"speaker":"nobody","text":"dropped"}]}"#;
    post_json(&st, "/meetings/transcripts/1/segments", two).await;
    assert_eq!(
        count(
            &st,
            "SELECT json_array_length(segments) FROM meeting_transcripts"
        ),
        3
    );
    assert!(segs(&st, 1).contains(r#""t":19.0,"speaker":"others""#));

    // Rows in time order; repeated speaker hidden; live tail note.
    let b = get(&st, "/meetings/1").await.body;
    let me = b.find("โอเค เริ่มเลย</span>").unwrap();
    let others = b.find("ได้ครับ deploy service ใหม่แล้ว</span>").unwrap();
    assert!(me < others);
    assert!(b.contains("<time>00:00:04</time><span class=\"who\" title=\"Me\">Me</span>"));
    assert!(b.contains("Live · about 30 s behind · th · 3 lines"));
    assert!(b.contains("Listening · the next lines arrive about 30 s after they’re said"));
    assert!(b.contains("hx-get=\"/meetings/1/transcript?q=\""));

    // Stop → finishing shows progress; done closes it; a late post is refused.
    post(&st, "/meetings/1/record/stop", "", false).await;
    let b = get(&st, "/meetings/1/transcript").await.body;
    assert!(b.contains("Transcribing the last part…"));
    assert!(b.contains("<progress class=\"prog\""));
    assert_eq!(
        post_json(&st, "/meetings/transcripts/1/segments", r#"{"done":true}"#)
            .await
            .status,
        204
    );
    let b = get(&st, "/meetings/1").await.body;
    assert!(b.contains("<code id=\"cmd-1\">/meeting 1</code>"));
    assert!(b.contains("3 lines · large-v3-turbo · th"));
    assert!(!b.contains("hx-get=\"/meetings/1/transcript"));
    assert_eq!(
        post_json(&st, "/meetings/transcripts/1/segments", one)
            .await
            .status,
        409
    );
    assert_eq!(
        post_json(&st, "/meetings/transcripts/9/segments", one)
            .await
            .status,
        404
    );

    // A recording that ends with no line leaves nothing behind.
    post(&st, "/meetings/1/record", "", false).await;
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meeting_transcripts WHERE id = 2 AND status = 'live'"
        ),
        1
    );
    post(&st, "/meetings/1/end", "", false).await;
    post_json(&st, "/meetings/transcripts/2/segments", r#"{"done":true}"#).await;
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meeting_transcripts"), 1);
    let _ = std::fs::remove_dir_all(&mac.dir);
}

#[tokio::test]
async fn speaker_split_replaces_others_and_names_follow_every_line() {
    let (st, mac) = mac_state("split", true);
    post(
        &st,
        "/meetings",
        &form(&[("name", "oms"), ("record", "1")]),
        false,
    )
    .await;
    let live = r#"{"segments":[{"t":1.0,"speaker":"others","text":"Hello all, two voices in one line"},
                               {"t":9.0,"speaker":"others","text":"Rollback is ready"},
                               {"t":12.0,"speaker":"me","text":"Rollback is ready"},
                               {"t":20.0,"speaker":"me","text":"Thanks both"}]}"#;
    post_json(&st, "/meetings/transcripts/1/segments", live).await;
    post(&st, "/meetings/1/record/stop", "", false).await;
    // The split: Others lines come back with speakers (the shared line cut in two); a bad label is dropped.
    let split = r#"{"done":true,"split":[{"t":1.0,"speaker":"Speaker 1","text":"Hello all"},
                   {"t":4.5,"speaker":"Speaker 2","text":"two voices in one line"},
                   {"t":9.0,"speaker":"Speaker 1","text":"Rollback is ready"},
                   {"t":15.0,"speaker":"me","text":"not the split's to write"}]}"#;
    assert_eq!(
        post_json(&st, "/meetings/transcripts/1/segments", split)
            .await
            .status,
        204
    );
    assert_eq!(
        count(
            &st,
            "SELECT json_array_length(segments) FROM meeting_transcripts"
        ),
        5
    );
    let b = get(&st, "/meetings/1").await.body;
    assert!(!b.contains(">Others</span>"));
    assert!(b.contains("aria-label=\"Name for Speaker 2\""));
    // The mic's echo of Speaker 1 is still hidden.
    assert!(b.contains("00:00:09 Speaker 1: Rollback is ready"));
    assert!(!b.contains("00:00:12 Me: Rollback is ready"));

    // Naming: every line of the voice follows; Me / blank are refused quietly.
    let r = post(
        &st,
        "/meetings/1/transcripts/1/speakers",
        &form(&[("from", "Speaker 1"), ("to", " Anna ")]),
        true,
    )
    .await;
    assert!(r.body.contains("00:00:01 Anna: Hello all"));
    assert!(r.body.contains("00:00:09 Anna: Rollback is ready"));
    for (from, to) in [("Speaker 2", "me"), ("Speaker 2", "  "), ("me", "Bob")] {
        post(
            &st,
            "/meetings/1/transcripts/1/speakers",
            &form(&[("from", from), ("to", to)]),
            false,
        )
        .await;
    }
    let s = segs(&st, 1);
    assert!(s.contains(r#""speaker":"Speaker 2""#) && !s.contains("Bob"));
    // Renaming one voice to another's name merges them (the split cut one person in two).
    post(
        &st,
        "/meetings/1/transcripts/1/speakers",
        &form(&[("from", "Speaker 2"), ("to", "Anna")]),
        false,
    )
    .await;
    let b = get(&st, "/meetings/1").await.body;
    assert_eq!(b.matches("aria-label=\"Name for ").count(), 1);
    let _ = std::fs::remove_dir_all(&mac.dir);
}

#[tokio::test]
async fn search_marks_hits_server_side() {
    let st = state();
    exec(
        &st,
        r#"INSERT INTO meetings(day, name, ended_at) VALUES ('2026-11-11', 'oms', '2026-11-11 11:00:00');
           INSERT INTO meeting_transcripts(meeting_id, started_at, ended_at, status, model, lang, segments)
           VALUES (1, '2026-11-11 10:00:00', '2026-11-11 11:02:00', 'done', 'large-v3-turbo', 'th',
             '[{"t":4,"speaker":"me","text":"Rollback is one script"},{"t":9,"speaker":"others","text":"no match here"},
               {"t":12,"speaker":"others","text":"rollback <b> plan"}]');"#,
    );
    let b = get(&st, "/meetings/1?q=ROLLBACK").await.body;
    assert!(b.contains("<mark>Rollback</mark> is one script"));
    assert!(b.contains("<mark>rollback</mark> &#60;b&#62; plan"));
    assert!(!b.contains("no match here</span>"));
    // Copy keeps every line, whatever the search shows.
    assert!(b.contains("00:00:09 Others: no match here"));
    assert!(b.contains("Recorded 1 h 02 min · 3 lines · large-v3-turbo · th"));
    let b = get(&st, "/meetings/1?q=zzz").await.body;
    assert!(b.contains("No line has “zzz”."));
}

#[tokio::test]
async fn a_recording_left_running_is_interrupted_at_the_next_launch() {
    let st = state();
    exec(
        &st,
        r#"INSERT INTO meetings(day, name) VALUES ('2026-11-11', 'oms');
           INSERT INTO meeting_transcripts(meeting_id, started_at, status, model, lang, segments)
           VALUES (1, '2026-11-11 10:00:00', 'live', 'large-v3-turbo', 'th',
             '[{"t":2490.2,"speaker":"me","text":"saved line"}]');"#,
    );
    atharu::recording::interrupt_orphans(&st.conn()).unwrap();
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meeting_transcripts WHERE status = 'interrupted' AND ended_at IS NOT NULL"
        ),
        1
    );
    let b = get(&st, "/meetings/1").await.body;
    assert!(b.contains("The transcript is saved up to <span class=\"mono\">00:41:30</span>; the rest of the audio is gone."));
    assert!(b.contains("saved line"));
    // An empty one (capture never got going) just goes.
    exec(
        &st,
        "INSERT INTO meeting_transcripts(meeting_id, started_at, status, model, lang)
         VALUES (1, '2026-11-11 10:50:00', 'live', 'large-v3-turbo', 'th')",
    );
    atharu::recording::interrupt_orphans(&st.conn()).unwrap();
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meeting_transcripts"), 1);
}

#[tokio::test]
async fn browser_only_watches_a_recording() {
    let st = state();
    exec(
        &st,
        r#"INSERT INTO meetings(day, name, started_at) VALUES ('2026-11-11', 'oms', '2026-11-11 10:00:00');
           INSERT INTO meeting_transcripts(meeting_id, started_at, model, lang)
           VALUES (1, datetime('now', 'localtime', '-1 minutes'), 'large-v3-turbo', 'th');"#,
    );
    let b = get(&st, "/").await.body;
    assert!(b.contains("Recording on the Mac <span class=\"rect\">01:0"));
    assert!(!b.contains("Stop recording"));
    let b = get(&st, "/meetings/1").await.body;
    assert!(b.contains("recorded in the Mac app; view only here"));
    assert!(b.contains("Live · th"));
    assert!(b.contains("About 30 s behind"));
    // Its record / stop buttons don't exist here, and a hand-made record does nothing.
    post(&st, "/meetings/1/record/stop", "", false).await;
    exec(&st, "UPDATE meeting_transcripts SET status = 'done'");
    post(&st, "/meetings/1/record", "", false).await;
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meeting_transcripts"), 1);
}

#[tokio::test]
async fn delete_and_undo_keep_the_transcript() {
    let st = state();
    exec(
        &st,
        r#"INSERT INTO meetings(day, name, ended_at) VALUES ('2026-11-11', 'oms', '2026-11-11 11:00:00');
           INSERT INTO meeting_transcripts(meeting_id, started_at, ended_at, status, model, lang, segments)
           VALUES (1, '2026-11-11 10:00:00', '2026-11-11 10:30:00', 'done', 'large-v3-turbo', 'th',
             '[{"t":4,"speaker":"me","text":"keep me"}]');"#,
    );
    let r = post(&st, "/meetings/1/delete", "", true).await;
    assert_eq!(count(&st, "SELECT COUNT(*) FROM meeting_transcripts"), 0);
    let at = r.body.find("name=\"snap\" value=\"").unwrap() + 19;
    let snap = r.body[at..at + r.body[at..].find('"').unwrap()]
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&#38;", "&");
    post(&st, "/meetings/restore", &form(&[("snap", &snap)]), false).await;
    assert!(get(&st, "/meetings/1").await.body.contains("keep me"));
}

/// Settings › Transcription over a fake models dir; the download runs curl on a `file://` URL.
#[tokio::test]
async fn settings_model_states() {
    let st = state();
    let b = get(&st, "/settings").await.body;
    assert!(b.contains("Recording and transcribing run in the Mac app."));
    assert_eq!(
        post(&st, "/settings/transcription/model", "act=download", false)
            .await
            .status,
        404
    );

    let (st, mac) = mac_state("settings", false);
    let b = get(&st, "/settings").await.body;
    assert!(b.contains("Download 574 MB"));
    assert!(b.contains("value=\"th\" aria-pressed=\"true\""));
    let r = post(&st, "/settings/transcription/lang", "lang=en", true).await;
    assert!(r.body.contains("value=\"en\" aria-pressed=\"true\""));
    assert_eq!(
        post(&st, "/settings/transcription/lang", "lang=xx", false)
            .await
            .status,
        422
    );

    // Wrong checksum: nothing kept, "Try again".
    let src = mac.dir.join("source.bin");
    std::fs::write(&src, vec![7u8; 4096]).unwrap();
    let mut m = Mac::new(mac.dir.join("m"));
    m.url = format!("file://{}", src.display());
    let fake = Arc::new(m);
    let mut st2 = st.clone();
    st2.mac = Some(fake.clone());
    post(&st2, "/settings/transcription/model", "act=download", false).await;
    let failed = wait(&st2, "didn’t match its checksum").await;
    assert!(failed.contains("Try again"));
    assert!(
        std::fs::read_dir(&fake.dir).unwrap().next().is_none(),
        "partial deleted"
    );

    // Right checksum: Ready, and the start switch turns on.
    let out = std::process::Command::new("shasum")
        .args(["-a", "256"])
        .arg(&src)
        .output()
        .unwrap();
    let sha = String::from_utf8(out.stdout).unwrap()[..64].to_string();
    let mut m = Mac::new(mac.dir.join("g"));
    m.url = format!("file://{}", src.display());
    m.sha256 = sha;
    let good = Arc::new(m);
    st2.mac = Some(good.clone());
    post(&st2, "/settings/transcription/model", "act=download", true).await;
    wait(&st2, ">Ready</span>").await;
    assert!(
        get(&st2, "/")
            .await
            .body
            .contains("Tell people you’re recording")
    );
    post(&st2, "/settings/transcription/model", "act=delete", false).await;
    assert!(
        get(&st2, "/settings")
            .await
            .body
            .contains("Download 574 MB")
    );
    assert!(!good.model_path().exists());
    let _ = std::fs::remove_dir_all(&mac.dir);
}

async fn wait(st: &AppState, needle: &str) -> String {
    for _ in 0..50 {
        let b = get(st, "/settings/transcription").await.body;
        if b.contains(needle) {
            return b;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("never showed {needle}");
}

#[tokio::test]
async fn record_switch_asks_for_screen_recording_up_front() {
    // Browser: no recorder, nothing to ask.
    let st = state();
    let r = post(&st, "/meetings/permissions", "", true).await;
    assert_eq!(r.status, 200);
    assert!(!r.body.contains("One step before recording"));
}

#[tokio::test]
async fn end_answers_with_a_link_to_the_meeting() {
    let st = state();
    post(&st, "/meetings", &form(&[("name", "oms-sync")]), true).await;
    let r = post(&st, "/meetings/1/end", &form(&[("back", "/")]), true).await;
    assert!(r.body.contains("Ended <b>oms-sync</b>"));
    assert!(r.body.contains("href=\"/meetings/1\">Open meeting</a>"));
    // A later page load doesn't repeat it.
    assert!(!get(&st, "/").await.body.contains("Ended <b>"));
}
