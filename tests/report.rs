mod common;
use common::*;

/// Today = Wed 11 Nov 2026 → this week = Mon 9 – Sun 15 Nov.
fn seed(st: &worklog::app::AppState) {
    exec(
        st,
        "INSERT INTO epics(key, title, okr, status, done_on, metric, before, after) VALUES
           ('OPS-101', 'Checkout latency', 'O1', 'done', '2026-11-10', 'checkout p95', '1.8s', '0.6s'),
           ('OPS-087', 'Cart autosave', 'O1', 'done', '2026-10-20', '', '', ''),
           ('OPS-092', 'Log retention', '', 'active', NULL, '', '', '');
         INSERT INTO epics(key) VALUES ('OPS-140');
         INSERT INTO evidence(epic_key, url) VALUES ('OPS-101', 'https://gh.io/412');
         INSERT INTO entries(day, epic_key, kind, text) VALUES
           ('2026-11-10', 'OPS-101', 'decision', 'keep sync path'),
           ('2026-11-09', 'OPS-101', 'impact', 'p95 1.8s → 0.6s'),
           ('2026-11-11', 'OPS-140', 'blocker', 'waiting on staging DB'),
           ('2026-11-11', 'OPS-092', 'done', 'plain done is not in the note'),
           ('2026-11-12', NULL, 'impact', 'reviewed <3> PRs'),
           ('2026-11-02', 'OPS-092', 'impact', 'last week, out of range');",
    );
}

fn markdown(body: &str) -> String {
    let start = body
        .find(r#"<textarea id="report-md" hidden readonly aria-hidden="true">"#)
        .unwrap();
    let rest = &body[start..];
    let open = rest.find('>').unwrap() + 1;
    let close = rest.find("</textarea>").unwrap();
    rest[open..close]
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#62;", ">")
        .replace("&#60;", "<")
}

#[tokio::test]
async fn week_note_markdown_exact() {
    let st = state();
    seed(&st);
    let r = get(&st, "/report").await;
    assert_eq!(r.status, 200);
    let md = markdown(&r.body);
    assert_eq!(
        md,
        "# Week of 9 Nov

## Shipped
- Did Checkout latency → checkout p95 1.8s→0.6s → https://gh.io/412

## OPS-101 · Checkout latency
- **Impact** p95 1.8s → 0.6s
- **Decision** keep sync path

## OPS-140
- **Blocker** waiting on staging DB

## Not filed under an epic
- **Impact** reviewed <3> PRs",
    );
}

#[tokio::test]
async fn range_filter_and_done_in_range() {
    let st = state();
    seed(&st);
    let week = get(&st, "/report").await.body;
    assert!(!week.contains("out of range"));
    assert!(!week.contains("Cart autosave"));
    assert!(week.contains(r#"<span class="small muted">Epics done</span><b>1</b>"#));
    assert!(week.contains(r#"<span class="small muted">Impact lines</span><b>2</b>"#));
    assert!(week.contains(r#"<span class="small muted">Decisions</span><b>1</b>"#));

    let month = get(&st, "/report?preset=month").await.body;
    assert!(month.contains("November 2026"));
    assert!(month.contains("last week, out of range"));
    assert!(!month.contains("Did Cart autosave"));

    let quarter = get(&st, "/report?preset=quarter").await.body;
    assert!(quarter.contains("Q4 2026"));
    assert!(quarter.contains("<li>Did Cart autosave</li>"));

    // Hand-picked range given backwards: read as 9 → 10 Nov.
    let custom = get(&st, "/report?from=2026-11-10&to=2026-11-09").await.body;
    assert!(custom.contains("9 Nov 2026 – 10 Nov 2026"));
    assert!(custom.contains("keep sync path"));
    assert!(!custom.contains("waiting on staging DB"));
    assert!(custom.contains("Did Checkout latency"));
}

#[tokio::test]
async fn empty_range_nudges_wider() {
    let st = state();
    let b = get(&st, "/report").await.body;
    assert!(b.contains("Nothing logged in this range. Try</span><a class=\"small\" href=\"/report?preset=month\">This month</a>"));
    assert!(markdown(&b) == "# Week of 9 Nov");
}
