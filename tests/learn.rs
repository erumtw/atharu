mod common;
use common::*;

async fn log(st: &atharu::app::AppState, line: &str, day: &str) {
    post(
        st,
        "/entries",
        &form(&[("line", line), ("day", day)]),
        false,
    )
    .await;
}

#[tokio::test]
async fn inbox_make_topic_and_stale_ordering() {
    let st = state();
    log(&st, "? pgvector: ivfflat needs analyze", "2026-11-10").await;
    log(&st, "? pgvector: hnsw is faster", TODAY).await;
    log(&st, "? no topic here", "2026-11-09").await;
    let b = get(&st, "/learn").await.body;
    assert!(b.contains("New topics from your log · 2"));
    assert!(
        b.contains("<span class=\"chip key\">pgvector</span><span class=\"muted\">2 lines</span>")
    );
    assert!(b.contains("1 learned line has no topic"));
    assert!(b.contains("href=\"/?day=2026-11-09\">its day</a>"));
    assert!(b.contains("No topics yet."));

    let r = post(&st, "/learn", &form(&[("slug", "pgvector")]), false).await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/learn/pgvector");
    let r = post(&st, "/learn", &form(&[("slug", "Bad Slug")]), false).await;
    assert_eq!(r.headers["location"], "/learn");
    assert_eq!(count(&st, "SELECT COUNT(*) FROM topics"), 1);

    // fresh (rated 3 days ago), stale (40 days), never rated: stale ones first.
    exec(
        &st,
        "INSERT INTO topics(slug) VALUES ('kafka-offsets'), ('go-generics');
         INSERT INTO depth_ratings VALUES ('pgvector', '2026-11-08', 3),
                                          ('kafka-offsets', '2026-10-02', 1);",
    );
    let b = get(&st, "/learn").await.body;
    assert!(b.contains("New topics from your log · 1"));
    let (g, k, p) = (
        b.find(">go-generics</a></td>").unwrap(),
        b.find(">kafka-offsets</a></td>").unwrap(),
        b.find(">pgvector</a></td>").unwrap(),
    );
    assert!(g < k && k < p, "never rated, then stale, then fresh");
    assert!(b.contains("2 Oct <span class=\"flag stale\">40 days ago · re-rate</span>"));
    assert!(b.contains("aria-label=\"Depth 3 of 4\""));
    assert!(b.contains("can build without AI"));
}

#[tokio::test]
async fn rating_appends_and_undo_drops_today_only() {
    let st = state();
    exec(
        &st,
        "INSERT INTO topics(slug) VALUES ('vector-search');
         INSERT INTO depth_ratings VALUES ('vector-search', '2026-09-12', 1),
                                          ('vector-search', '2026-10-10', 2);",
    );
    let r = post(
        &st,
        "/learn/vector-search/rate",
        &form(&[("depth", "4")]),
        true,
    )
    .await;
    assert_eq!(r.status, 200);
    // Same day re-rate replaces, earlier days stay as history.
    let r = post(
        &st,
        "/learn/vector-search/rate",
        &form(&[("depth", "3")]),
        true,
    )
    .await;
    assert!(r.body.contains("Rated 3 today ·"));
    assert!(r.body.contains("hx-swap-oob"));
    assert!(r.body.contains("<span class=\"chip\">10 Oct · 2 can explain</span>→<span class=\"chip\">11 Nov · 3 can build without AI</span>"));
    assert_eq!(count(&st, "SELECT COUNT(*) FROM depth_ratings"), 3);

    let r = post(&st, "/learn/vector-search/unrate", "", false).await;
    assert_eq!(r.status, 303);
    assert_eq!(count(&st, "SELECT COUNT(*) FROM depth_ratings"), 2);
    let b = get(&st, "/learn/vector-search").await.body;
    assert!(!b.contains("Rated "));
    assert!(b.contains("rated 10 Oct"));

    assert_eq!(
        post(
            &st,
            "/learn/vector-search/rate",
            &form(&[("depth", "5")]),
            true
        )
        .await
        .status,
        422
    );
    assert_eq!(
        post(&st, "/learn/nope/rate", &form(&[("depth", "2")]), true)
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn autosave_writes_only_posted_fields() {
    let st = state();
    exec(
        &st,
        "INSERT INTO topics(slug, resources) VALUES ('kafka', 'kip-500')",
    );
    log(&st, "OPS-118 ? kafka: lag is rebalance", TODAY).await;
    let r = post(
        &st,
        "/learn/kafka",
        &form(&[
            ("summary", "offsets per partition"),
            ("own_words", "consumers commit"),
        ]),
        true,
    )
    .await;
    assert!(r.body.contains("✓ saved"));
    let r = post(&st, "/learn/kafka", &form(&[("resources", "docs")]), false).await;
    assert_eq!(r.headers["location"], "/learn/kafka");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM topics WHERE summary = 'offsets per partition'
             AND own_words = 'consumers commit' AND resources = 'docs'"
        ),
        1
    );
    let b = get(&st, "/learn/kafka").await.body;
    // Learn lines draw through the shared entry_row.html: text span, then the epic chip.
    assert!(b.contains("lag is rebalance</span> <a href=\"/epics/OPS-118\">"));
    assert!(b.contains("not rated yet"));
    assert_eq!(get(&st, "/learn/nope").await.status, 404);
}

#[tokio::test]
async fn report_heatmap_and_depth_changes() {
    let st = state();
    exec(
        &st,
        "INSERT INTO topics(slug) VALUES ('vector-search'), ('kafka');
         INSERT INTO depth_ratings VALUES ('vector-search', '2026-09-12', 1),
                                          ('vector-search', '2026-10-10', 2),
                                          ('vector-search', '2026-10-30', 3),
                                          ('vector-search', '2026-11-10', 4),
                                          ('kafka', '2026-10-05', 1);",
    );
    let b = get(&st, "/report?from=2026-09-01&to=2026-11-15").await.body;
    assert!(b.contains("<th>Sep</th><th>Oct</th><th>Nov</th>"));
    // Oct = latest as of 31 Oct (3, not 2); kafka blank before its first rating.
    assert!(b.contains(
        "<td class=\"mono\"><span class=\"clip\" title=\"vector-search\">vector-search</span></td><td class=\"heat1\">1</td><td class=\"heat3\">3</td>"
    ));
    assert!(b.contains("<td class=\"mono\"><span class=\"clip\" title=\"kafka\">kafka</span></td><td class=\"muted\">–</td>"));

    let b = get(&st, "/report").await.body; // week of 9 Nov
    assert!(b.contains("<li>vector-search: depth 3 → 4 (can teach)</li>"));
    assert!(!b.contains("kafka: depth"));
    assert!(b.contains("## Learning\n- vector-search: depth 3 → 4 (can teach)"));
}
