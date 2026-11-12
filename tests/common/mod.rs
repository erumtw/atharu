//! Shared HTTP test harness: router over in-memory SQLite with "today" pinned.
#![allow(dead_code)] // each test binary uses a different subset

use athar::app::{AppState, router};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

pub const TODAY: &str = "2026-11-11"; // Wednesday

pub fn state() -> AppState {
    let mut st = AppState::new(athar::db::open_in_memory().unwrap());
    st.fixed_today = Some(TODAY.parse().unwrap());
    st
}

pub fn exec(st: &AppState, sql: &str) {
    st.conn().execute_batch(sql).unwrap();
}

pub fn count(st: &AppState, sql: &str) -> i64 {
    st.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

/// Form-urlencode by hand: every byte outside [A-Za-z0-9] as %XX is always valid.
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

pub fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub struct Resp {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: String,
}

pub async fn send(app: Router, req: Request<Body>) -> Resp {
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    Resp {
        status,
        headers,
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

pub async fn get(st: &AppState, uri: &str) -> Resp {
    let req = Request::get(uri).body(Body::empty()).unwrap();
    send(router(st.clone()), req).await
}

/// `htmx = true` adds `HX-Request`, i.e. the partial path instead of the redirect path.
pub async fn request(st: &AppState, method: &str, uri: &str, body: &str, htmx: bool) -> Resp {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded");
    if htmx {
        req = req.header("hx-request", "true");
    }
    send(
        router(st.clone()),
        req.body(Body::from(body.to_string())).unwrap(),
    )
    .await
}

pub async fn post(st: &AppState, uri: &str, body: &str, htmx: bool) -> Resp {
    request(st, "POST", uri, body, htmx).await
}
