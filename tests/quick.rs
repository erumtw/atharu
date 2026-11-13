mod common;
use common::*;

#[tokio::test]
async fn renders_the_three_sections_without_the_app_chrome() {
    let st = state();
    exec(
        &st,
        "INSERT INTO meetings(day, name, ended_at) VALUES
           ('2026-11-02', 'weekly-ops', 'x'), ('2026-11-05', 'oms-sync', 'x'),
           ('2026-11-06', 'hiring', 'x'), ('2026-11-09', 'weekly-ops', 'x'), ('2026-11-10', '1-on-1', 'x');",
    );
    let b = get(&st, "/quick").await.body;
    for want in [
        "class=\"menu quick\"",
        "Wed 11 Nov",
        "name=\"kind\" value=\"+\"",
        "<b>+</b>impact",
        "Start a meeting",
        "Nothing due today",
        "name=\"back\" value=\"/quick\"",
    ] {
        assert!(b.contains(want), "{want}");
    }
    // Recent series by last use, in the start box's menu (T93); four exist here, all listed.
    let menu: Vec<&str> = b
        .match_indices("role=\"option\" id=\"qser-")
        .map(|(i, _)| {
            b[i..]
                .split("data-name=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
        })
        .collect();
    assert_eq!(&menu[..3], ["1-on-1", "weekly-ops", "hiring"]);
    assert!(!b.contains("class=\"side"));
    assert!(!b.contains("responsive.css"));
}

#[tokio::test]
async fn one_field_with_a_kind_menu_and_the_legend() {
    let st = state();
    let b = get(&st, "/quick").await.body;
    // 0.2.6: ONE field: the kind button inside, the line, Log. No tile row.
    assert!(!b.contains("class=\"kinds\"") && !b.contains("class=\"kc"));
    assert!(b.contains("class=\"q2k\"") && b.contains("aria-label=\"Line kind: done\""));
    assert!(b.contains("<button class=\"btn\" type=\"submit\">Log ↵</button>"));
    // The menu: six kinds (a hidden radio group, so it also posts without JS) + the note row, with their typed keys.
    assert_eq!(b.matches("role=\"menuitemradio\"").count(), 7);
    assert_eq!(
        b.matches("<input class=\"vh\" type=\"radio\" name=\"kind\"")
            .count(),
        6
    );
    assert!(b.contains("<b>n</b>note<span class=\"kb\">n␣</span>"));
    assert!(b.contains("<b>x</b>blocker<span class=\"kb\">x␣</span>"));
    assert!(b.contains("aria-checked=\"true\"><input class=\"vh\" type=\"radio\" name=\"kind\" value=\"\" form=\"qline\" checked"));
    // The legend under the field; the header carries the note button (not pressed) and Open Atharu.
    assert!(b.contains("<p class=\"q2leg\">or type <b>!</b> decision · <b>x</b> blocker"));
    assert!(b.contains("aria-pressed=\"false\" aria-label=\"New note\""));
    assert!(b.contains("aria-label=\"Open Atharu\""));
    // The button shows the kind it was opened on.
    let b = get(&st, "/quick?kind=%21").await.body;
    assert!(
        b.contains("aria-label=\"Line kind: decision\"><b>!</b><span class=\"w\">decision</span>")
    );
}

#[tokio::test]
async fn kind_tile_sets_the_prefix_and_a_typed_one_wins() {
    let st = state();
    // htmx or not, `back` answers with a 303 to the popover, carrying the new line.
    let r = post(
        &st,
        "/entries",
        &form(&[
            ("line", "OPS-1 p95 down"),
            ("kind", "+"),
            ("back", "/quick"),
        ]),
        true,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/quick?saved=1");
    let r = post(
        &st,
        "/entries",
        &form(&[("line", "! keep sync"), ("kind", "+"), ("back", "/quick")]),
        false,
    )
    .await;
    assert_eq!(r.headers["location"], "/quick?saved=2");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE (id, kind, text) IN (VALUES (1, 'impact', 'p95 down'), (2, 'decision', 'keep sync'))"
        ),
        2
    );
    let b = get(&st, "/quick?saved=1").await.body;
    assert!(b.contains("Saved as <b>impact</b> on <span class=\"chip key\">OPS-1</span>"));
    assert!(b.contains("action=\"/entries/1/delete?back=/quick\""));

    // The live hint follows the tile too.
    let r = post(
        &st,
        "/parse-preview",
        &form(&[("line", "ask DBA"), ("kind", ">")]),
        true,
    )
    .await;
    assert!(r.body.contains("will save as <b>plan</b>"));

    // Undo from the popover goes back to it.
    let r = post(&st, "/entries/1/delete?back=/quick", "", false).await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/quick");
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 1);
}

#[tokio::test]
async fn todos_are_today_and_overdue_overdue_first_capped_at_eight() {
    let st = state();
    exec(
        &st,
        "INSERT INTO entries(day, kind, text, plan_state, due) VALUES
           ('2026-11-01', 'plan', 'today a', 'open', '2026-11-11'),
           ('2026-11-01', 'plan', 'late tue', 'open', '2026-11-10'),
           ('2026-11-01', 'plan', 'today b', 'open', '2026-11-11'),
           ('2026-11-01', 'plan', 'late old', 'open', '2026-11-02'),
           ('2026-11-01', 'plan', 'today c', 'open', '2026-11-11'),
           ('2026-11-01', 'plan', 'today d', 'open', '2026-11-11'),
           ('2026-11-01', 'plan', 'today e', 'open', '2026-11-11'),
           ('2026-11-01', 'plan', 'today f', 'open', '2026-11-11'),
           ('2026-11-01', 'plan', 'today g', 'open', '2026-11-11'),
           ('2026-11-01', 'plan', 'later', 'open', '2026-11-20'),
           ('2026-11-01', 'plan', 'no date', 'open', NULL),
           ('2026-11-01', 'plan', 'done one', 'done', '2026-11-11');",
    );
    let b = get(&st, "/quick").await.body;
    let rows: Vec<&str> = b
        .match_indices("<span class=\"qtx\">")
        .map(|(i, m)| b[i + m.len()..].split('<').next().unwrap())
        .collect();
    assert_eq!(
        rows,
        [
            "late old", "late tue", "today a", "today b", "today c", "today d", "today e",
            "today f"
        ]
    );
    assert!(b.contains("· 2 overdue, 7 today"));
    assert!(b.contains("<span class=\"due late\">due 2 Nov</span>"));
    assert!(b.contains("<span class=\"due late\">due Tue</span>"));
    assert!(b.contains("<span class=\"due\">due today</span>"));
    // 11 open, 8 shown: today g, later, no date.
    assert!(b.contains("+ 3 more open · Open Atharu"));

    // Ticking posts to the plans slice; a plain form comes back here.
    let r = post(
        &st,
        "/plans/1",
        &form(&[("state", "done"), ("back", "/quick")]),
        false,
    )
    .await;
    assert_eq!(r.headers["location"], "/quick");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM entries WHERE id = 1 AND plan_state = 'done'"
        ),
        1
    );
}

#[tokio::test]
async fn start_and_end_meeting_come_back_to_the_popover() {
    let st = state();
    let r = post(
        &st,
        "/meetings",
        &form(&[("name", "oms-sync"), ("back", "/quick")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/quick");
    let b = get(&st, "/quick").await.body;
    assert!(b.contains("<a class=\"nm\" href=\"/meetings/1\" title=\"oms-sync\">oms-sync</a>"));
    assert!(b.contains("<span class=\"rect\">0 min</span>"));
    assert!(b.contains("action=\"/meetings/1/end\""));
    assert!(!b.contains("Start a meeting"));
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM meetings WHERE started_at IS NOT NULL"
        ),
        1
    );

    let r = post(&st, "/meetings/1/end", &form(&[("back", "/quick")]), false).await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/quick?ended=1");
    let b = get(&st, "/quick?ended=1").await.body;
    assert!(b.contains("Start a meeting"));
    assert!(b.contains("Ended <b>oms-sync</b> · <a href=\"/meetings/1\">Open meeting</a>"));
}

#[tokio::test]
async fn note_tile_saves_a_note_and_comes_back_on_the_kind() {
    let st = state();
    // Without JS the Note tile is a GET carrying the line form: the half-typed line moves into the note box.
    let b = get(
        &st,
        "/quick?back=%2Fquick&kind=%21&line=vendor+call&mode=note",
    )
    .await
    .body;
    assert!(b.contains("data-mode=\"note\""));
    // The header's note button is the pressed one; the legend lives with the (hidden) line form.
    assert!(b.contains("name=\"mode\" value=\"\" aria-pressed=\"true\" aria-label=\"New note\""));
    assert!(b.contains("autofocus>vendor call</textarea>"));
    assert!(b.contains("name=\"back\" value=\"/quick?kind=%21\""));

    // htmx or not, the save answers with a 303 to the popover on the same kind, carrying the note.
    let r = post(
        &st,
        "/notes/new",
        &form(&[
            ("text", "Vendor call\nThey want 4 h windows #vendor"),
            ("back", "/quick?kind=%2B"),
        ]),
        true,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(r.headers["location"], "/quick?kind=%2B&note=1");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE id = 1 AND title = 'Vendor call'
               AND body = 'They want 4 h windows #vendor' AND author = 'me'"
        ),
        1
    );
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM taggings WHERE tag = 'vendor' AND item_type = 'note'"
        ),
        1
    );
    let b = get(&st, "/quick?kind=%2B&note=1").await.body;
    assert!(b.contains("Saved note · <a href=\"/notes/1\">Open</a>"));
    assert!(b.contains("value=\"+\" form=\"qline\" checked"));
    assert!(b.contains("data-mode=\"line\""));
    assert!(get(&st, "/notes").await.body.contains("Vendor call"));
}

#[tokio::test]
async fn blank_quick_note_is_refused_into_the_hint() {
    let st = state();
    let blank = form(&[("text", " \n  "), ("back", "/quick?kind=")]);
    let r = post(&st, "/notes/new", &blank, true).await;
    assert_eq!(r.status, 422);
    assert_eq!(r.headers["hx-retarget"], "#hint");
    assert!(r.body.contains("Type the note’s title first."));
    let r = post(&st, "/notes/new", &blank, false).await;
    assert_eq!(r.headers["location"], "/quick?kind=&mode=note&err=1");
    let b = get(&st, "/quick?kind=&mode=note&err=1").await.body;
    assert!(b.contains("<span class=\"err\">Nothing to save."));
    assert_eq!(count(&st, "SELECT COUNT(*) FROM notes"), 0);
}

#[tokio::test]
async fn a_tag_on_the_title_line_tags_the_note() {
    // D54: a one-line quick note keeps its tag in the title, so the title counts like the body.
    let st = state();
    let r = post(
        &st,
        "/notes/new",
        &form(&[("text", "vendor call recap #expense"), ("back", "/quick")]),
        true,
    )
    .await;
    assert_eq!(r.status, 303);
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM taggings WHERE tag = 'expense' AND item_type = 'note'"
        ),
        1
    );
}

#[tokio::test]
async fn n_space_in_the_line_box_saves_a_note_without_js() {
    // D55: the browser re-sends a 307 with the same body, so the test posts it twice by hand.
    let st = state();
    let body = form(&[
        ("line", "n vendor call #ops"),
        ("kind", "!"),
        ("back", "/quick"),
    ]);
    let r = post(&st, "/entries", &body, false).await;
    assert_eq!(r.status, 307);
    assert_eq!(r.headers["location"], "/notes/new");
    let r = post(&st, "/notes/new", &body, false).await;
    assert_eq!(r.headers["location"], "/quick?note=1");
    assert_eq!(
        count(
            &st,
            "SELECT COUNT(*) FROM notes WHERE title = 'vendor call #ops' AND author = 'me'"
        ),
        1
    );
    assert_eq!(count(&st, "SELECT COUNT(*) FROM entries"), 0);

    // Needs the space, like `x `; and only the popover's box: on Today `n …` stays a line.
    let r = post(
        &st,
        "/entries",
        &form(&[("line", "nginx fixed"), ("back", "/quick")]),
        false,
    )
    .await;
    assert_eq!(r.status, 303);
    post(&st, "/entries", &form(&[("line", "n plain")]), false).await;
    assert_eq!(
        count(&st, "SELECT COUNT(*) FROM entries WHERE kind = 'done'"),
        2
    );
    assert_eq!(count(&st, "SELECT COUNT(*) FROM notes"), 1);
}
