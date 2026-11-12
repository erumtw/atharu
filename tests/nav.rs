mod common;
use common::*;

/// T26 / D18: one nav on every page, grouped by the user's loops; Settings last, after the active epics.
#[tokio::test]
async fn nav_follows_the_user_journey_on_every_page() {
    let st = state();
    exec(
        &st,
        "INSERT INTO epics(key, title) VALUES ('OPS-1', 'Cart');",
    );
    let order = [
        ">Today<",
        ">To-dos<",
        ">Calendar<",
        ">Meetings<",
        "<div class=\"lbl nh\">Track</div>",
        ">Epics<",
        ">Report<",
        "<div class=\"lbl nh\">Grow</div>",
        ">Learn<",
        ">Notes<",
        ">Active epics<",
        "href=\"/epics/OPS-1\"",
        "<span class=\"gap\"></span>",
        ">Settings<",
    ];
    for p in [
        "/",
        "/todos",
        "/calendar",
        "/meetings",
        "/epics",
        "/report",
        "/learn",
        "/notes",
        "/settings",
    ] {
        let b = get(&st, p).await.body;
        let nav = &b[b.find("<nav class=\"side").unwrap()..b.find("</nav>").unwrap()];
        let mut at = 0;
        for o in order {
            let i = nav[at..]
                .find(o)
                .unwrap_or_else(|| panic!("{p}: {o} missing or out of order"));
            at += i + o.len();
        }
        assert_eq!(nav.matches("class=\"lbl nh\"").count(), 2, "{p}");
    }
}

fn req(method: &str, host: &str, site: Option<&str>) -> axum::http::Request<axum::body::Body> {
    let mut r = axum::http::Request::builder()
        .method(method)
        .uri("/entries")
        .header("host", host)
        .header("content-type", "application/x-www-form-urlencoded");
    if let Some(s) = site {
        r = r.header("sec-fetch-site", s);
    }
    r.body(axum::body::Body::from("line=hi")).unwrap()
}

#[tokio::test]
async fn foreign_host_is_refused() {
    let st = state();
    let app = || atharu::app::router(st.clone());
    assert_eq!(send(app(), req("POST", "evil.com", None)).await.status, 403);
    assert_eq!(
        send(app(), req("POST", "localhost.evil.com:7070", None))
            .await
            .status,
        403
    );
    for ok in ["127.0.0.1:7070", "localhost", "[::1]:51234"] {
        assert_eq!(send(app(), req("POST", ok, None)).await.status, 303, "{ok}");
    }
}

#[tokio::test]
async fn cross_site_write_is_refused() {
    let st = state();
    let app = || atharu::app::router(st.clone());
    let r = send(app(), req("POST", "127.0.0.1:7070", Some("cross-site"))).await;
    assert_eq!(r.status, 403);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 0);
    let r = send(app(), req("POST", "127.0.0.1:7070", Some("same-origin"))).await;
    assert_eq!(r.status, 303);
}
