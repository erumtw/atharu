//! Pasted note images (0.2.10): upload by magic bytes, serve path-checked.
mod common;
use atharu::app::{AppState, router};
use axum::{body::Body, http::Request};
use common::*;

fn file_state(tag: &str) -> (AppState, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("atharu-noteimg-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (
        AppState::new(atharu::db::open(&dir.join("atharu.db")).unwrap()),
        dir,
    )
}

/// The raw file as the body, with whatever Content-Type the (untrusted) client claims.
async fn upload(st: &AppState, id: i64, claimed: &str, bytes: Vec<u8>) -> Resp {
    let req = Request::post(format!("/notes/{id}/images"))
        .header("content-type", claimed)
        .body(Body::from(bytes))
        .unwrap();
    send(router(st.clone()), req).await
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake-png-bytes";

#[tokio::test]
async fn upload_stores_serves_and_answers_the_url() {
    let (st, dir) = file_state("ok");
    post(&st, "/notes", "", false).await;
    let r = upload(&st, 1, "image/png", PNG.to_vec()).await;
    assert_eq!(r.status, 200, "{}", r.body);
    let json: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    let url = json["data"]["image"]["url"].as_str().unwrap().to_string();
    assert!(
        url.starts_with("/attachments/notes/1/") && url.ends_with(".png"),
        "{url}"
    );
    let name = url.rsplit('/').next().unwrap();
    assert_eq!(
        std::fs::read(dir.join("attachments/notes/1").join(name)).unwrap(),
        PNG
    );

    let got = get(&st, &url).await;
    assert_eq!(got.status, 200);
    assert_eq!(got.headers["content-type"], "image/png");
    assert_eq!(got.headers["x-content-type-options"], "nosniff");
    assert!(
        got.headers["cache-control"]
            .to_str()
            .unwrap()
            .contains("immutable")
    );

    // The header is the client's word: the bytes decide (a PNG sent as text/html is still a PNG).
    assert_eq!(upload(&st, 1, "text/html", PNG.to_vec()).await.status, 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn svg_and_other_bytes_are_refused_whatever_the_header_says() {
    let (st, dir) = file_state("refuse");
    post(&st, "/notes", "", false).await;
    for bytes in [
        &b"<svg xmlns=\"http://www.w3.org/2000/svg\" onload=\"alert(1)\"/>"[..],
        b"<html><script>alert(1)</script></html>",
        b"",
    ] {
        let r = upload(&st, 1, "image/png", bytes.to_vec()).await;
        assert_eq!(r.status, 415, "{:?}", String::from_utf8_lossy(bytes));
        assert_eq!(r.body, "Paste a PNG, JPEG, GIF or WebP image.");
    }
    assert!(!dir.join("attachments/notes/1").exists(), "nothing written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn unknown_note_and_oversize_are_refused() {
    let (st, dir) = file_state("limits");
    post(&st, "/notes", "", false).await;
    assert_eq!(upload(&st, 9, "image/png", PNG.to_vec()).await.status, 404);
    let mut big = PNG.to_vec();
    big.resize(25 * 1024 * 1024 + 1, 0);
    assert_eq!(upload(&st, 1, "image/png", big).await.status, 413);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn only_names_we_wrote_are_served() {
    let (st, dir) = file_state("paths");
    post(&st, "/notes", "", false).await;
    upload(&st, 1, "image/png", PNG.to_vec()).await;
    // A look-alike file the app never wrote must stay unreachable even if it sits in the folder.
    std::fs::write(
        dir.join("attachments/notes/1/20261117-101500.svg"),
        "<svg/>",
    )
    .unwrap();
    for bad in [
        "..%2Fatharu.db",
        "../atharu.db",
        "x.png",
        "20261117-101500.svg",
        "20261117-101500.png.html",
    ] {
        assert_eq!(
            get(&st, &format!("/attachments/notes/1/{bad}"))
                .await
                .status,
            404,
            "{bad}"
        );
    }
    // Note 1's folder is not another note's.
    assert_eq!(
        get(&st, "/attachments/notes/2/20261117-101500.png")
            .await
            .status,
        404
    );
    let _ = std::fs::remove_dir_all(&dir);
}
