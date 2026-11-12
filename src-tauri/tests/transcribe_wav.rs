//! End to end without the GUI or a microphone: a WAV goes through the same worker loop and HTTP post
//! as a live recording, into a temp DB behind the real router. Needs the 574 MB model, so opt-in:
//!   ATHARU_T53_MODELS=<dir with ggml-large-v3-turbo-q5_0.bin> ATHARU_T53_WAV=<16 kHz mono s16 wav> \
//!   cargo test -p atharu-app --release --test transcribe_wav -- --ignored --nocapture
//! The WAV goes in as the Others stream, so the speaker split (T56) runs after Stop: it fetches its models into
//! `<models>/speakers` if missing. ATHARU_T56_SPEAKERS=<n> also checks it told n voices apart.
#![cfg(feature = "recording")]
use atharu::{app, db, recording::Mac};
use atharu_app::recording::{pipeline::Pipeline, run, speakers::Tap};
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, atomic::AtomicBool, mpsc},
};

fn read_wav(path: &str) -> Vec<f32> {
    let b = std::fs::read(path).unwrap();
    // Walk RIFF chunks to "data": ffmpeg / afconvert may put LIST before it.
    let mut i = 12;
    loop {
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        if &b[i..i + 4] == b"data" {
            return b[i + 8..i + 8 + len]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes(*c) as f32 / 32768.0)
                .collect();
        }
        i += 8 + len + (len & 1);
    }
}

fn http(addr: std::net::SocketAddr, req: &str) -> String {
    let mut c = TcpStream::connect(addr).unwrap();
    c.write_all(req.as_bytes()).unwrap();
    let mut out = String::new();
    c.read_to_string(&mut out).unwrap();
    out
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the speech model and a WAV (env ATHARU_T53_MODELS, ATHARU_T53_WAV)"]
async fn wav_lands_as_segments_on_the_meeting() {
    let models = std::env::var("ATHARU_T53_MODELS").expect("ATHARU_T53_MODELS");
    let wav = std::env::var("ATHARU_T53_WAV").expect("ATHARU_T53_WAV");
    let dbp = std::env::temp_dir().join(format!("atharu-t53-e2e-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&dbp);
    let mut st = app::AppState::new(db::open(&dbp).unwrap());
    let mac = Arc::new(Mac::new(models.into()));
    assert!(mac.ready(), "model missing");
    st.mac = Some(mac.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = app::router(st.clone());
    tokio::spawn(async move { axum::serve(listener, router).await });

    // The owner's active epics are the glossary in real use; these stand in for them.
    st.conn()
        .execute_batch(
            "INSERT INTO epics(key, title) VALUES ('OPS-101', 'Deploy service latency'),
               ('OPS-102', 'Database lock contention'), ('OPS-103', 'Grafana alert and rollback plan')",
        )
        .unwrap();
    let body = "name=oms-sync&record=1";
    let r = tokio::task::spawn_blocking(move || {
        http(addr, &format!("POST /meetings HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()))
    })
    .await
    .unwrap();
    assert!(r.starts_with("HTTP/1.1 303"), "{r}");
    let want = atharu::recording::wanted(&st.conn())
        .unwrap()
        .expect("live recording");
    assert!(want.prompt.starts_with("Glossary: oms-sync, OPS-101"));

    // The live path: audio arrives in small chunks on a channel, the worker posts each window.
    let audio = read_wav(&wav);
    let secs = audio.len() as f64 / 16_000.0;
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let (model, flag, tid) = (mac.model_path(), stop.clone(), want.tid);
    let models = mac.dir.clone();
    let tmp = std::env::temp_dir().join(format!("atharu-others-{tid}.pcm"));
    let t0 = std::time::Instant::now();
    let worker = std::thread::spawn(move || {
        let mut p = Pipeline::new(&model, &want.lang, &want.prompt).unwrap();
        run(&mut p, &rx, &flag, addr, tid, Tap::new(tid), &models);
    });
    for chunk in audio.chunks(1_600) {
        tx.send((1usize, chunk.to_vec())).unwrap();
    }
    // Stop: the page flips the row to finishing, the recorder drains and transcribes the tail.
    let stop_req = "POST /meetings/1/record/stop HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    tokio::task::spawn_blocking(move || http(addr, stop_req))
        .await
        .unwrap();
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    tokio::task::spawn_blocking(move || worker.join().unwrap())
        .await
        .unwrap();
    let took = t0.elapsed().as_secs_f64();

    let (status, lines, text): (String, i64, String) = st
        .conn()
        .query_row(
            "SELECT status, json_array_length(segments),
                    (SELECT group_concat(value ->> 't' || ' ' || (value ->> 'speaker') || ': ' || (value ->> 'text'), char(10))
                     FROM json_each(segments))
             FROM meeting_transcripts WHERE id = ?1",
            [tid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    println!(
        "{text}\n-- {lines} lines, {secs:.1} s audio in {took:.1} s (RTF {:.3}, incl. the speaker split)",
        took / secs
    );
    assert_eq!(status, "done");
    assert!(!tmp.exists(), "Others audio deleted once the split ends");
    if let Ok(n) = std::env::var("ATHARU_T56_SPEAKERS") {
        let got: i64 = st
            .conn()
            .query_row(
                "SELECT COUNT(DISTINCT value ->> 'speaker') FROM meeting_transcripts t, json_each(t.segments)
                 WHERE t.id = ?1 AND value ->> 'speaker' LIKE 'Speaker %'",
                [tid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(got, n.parse::<i64>().unwrap());
    }
    assert!(lines >= 8, "only {lines} lines");
    assert!(
        text.to_lowercase().contains("deploy"),
        "English term kept in Latin script"
    );
    let _ = std::fs::remove_file(&dbp);
}
