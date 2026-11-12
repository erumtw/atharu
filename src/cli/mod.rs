//! Agent CLI (spec "Agent access", D8): `atharu log | epic | note | evidence | today | show | tags`,
//! JSON on stdout. No delete or edit commands (D14): those stay with the user, on the web.
//! An entry point like app.rs: it calls the slices' own queries and `domain::parse`, so a line
//! filed by an agent goes through exactly the validation the capture bar uses.
use crate::{
    domain::{
        dates,
        entry::{is_epic_key, is_slug},
        links,
        parse::{ParseError, parse},
        tags::{parse_list, tags as tags_in},
    },
    epics::queries as epics,
    notes::queries as notes,
    tags::queries as tags,
    today::queries as today,
};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{Read, Write},
};

pub mod update;

/// `version` and `update` need no DB: main hands them to `update::run` before opening one.
pub const COMMANDS: [&str; 9] = [
    "log", "epic", "note", "evidence", "today", "show", "tags", "version", "update",
];

/// The command table for `atharu help` and every usage error (CLI and `config`), so they can't drift.
pub const HELP: &str = "usage: atharu [--db PATH] [command]
  atharu                                   serve http://127.0.0.1:7070 (ATHARU_HOST / ATHARU_PORT)
  atharu log \"<line>\" [--day YYYY-MM-DD] --as <agent>
  atharu epic <KEY|jira-url> [--title …] [--okr …] [--tags a,b] --as <agent>
  atharu note \"<title>\" [--file PATH] [--links a,b] [--id N] --as <agent>   (body from stdin without --file)
  atharu evidence <KEY|jira-url> <url> [--label …] --as <agent>
  atharu today [--day YYYY-MM-DD]
  atharu show <KEY|jira-url>
  atharu tags
  atharu dump                              write dump.sql next to the DB
  atharu version · atharu update [--app] [--docker]
  atharu help                              this table (also -h, --help)
--as can come from ATHARU_AGENT instead; --db from ATHARU_DB.";

/// Exit code + message for stderr. 1 = input rejected (same copy as the web), 2 = usage.
struct Fail(i32, String);

fn usage(msg: impl Into<String>) -> Fail {
    Fail(2, format!("{}\n{HELP}", msg.into()))
}

/// Runs one command. Exit code 0 prints one JSON document on `out`; anything else prints to `err`.
/// `input` is read only by `note` without `--file`.
pub fn run(
    conn: &mut Connection,
    today: NaiveDate,
    args: &[String],
    env_agent: Option<&str>,
    input: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let res = (|| {
        let (cmd, rest) = args.split_first().ok_or_else(|| usage("no command"))?;
        let (pos, flags) = split_args(rest)?;
        let allowed: &[&str] = match cmd.as_str() {
            "log" => &["as", "day"],
            "epic" => &["as", "title", "okr", "tags"],
            "note" => &["as", "file", "links", "id"],
            "evidence" => &["as", "label"],
            "today" => &["day"],
            "show" | "tags" => &[],
            other => return Err(usage(format!("unknown command {other:?}"))),
        };
        if let Some(f) = flags.keys().find(|f| !allowed.contains(&f.as_str())) {
            return Err(usage(format!("{cmd} takes no --{f}")));
        }
        let flag = |k: &str| flags.get(k).map(String::as_str);
        match cmd.as_str() {
            "log" => log(
                conn,
                today,
                &pos,
                flag("day"),
                agent(flag("as"), env_agent)?,
            ),
            "epic" => epic(
                conn,
                &pos,
                flag("title").unwrap_or(""),
                flag("okr").unwrap_or(""),
                flag("tags").unwrap_or(""),
                agent(flag("as"), env_agent)?,
            ),
            "note" => note(
                conn,
                &pos,
                flag("file"),
                flag("links"),
                flag("id"),
                input,
                agent(flag("as"), env_agent)?,
            ),
            "evidence" => evidence(
                conn,
                &pos,
                flag("label").unwrap_or(""),
                agent(flag("as"), env_agent)?,
            ),
            "today" => day(conn, today, &pos, flag("day")),
            "tags" => tag_list(conn, &pos),
            _ => show(conn, &pos),
        }
    })();
    match res {
        Ok(v) => {
            let _ = writeln!(out, "{v}");
            0
        }
        Err(Fail(code, msg)) => {
            let _ = writeln!(err, "{msg}");
            code
        }
    }
}

/// Positionals + `--name value` / `--name=value` flags.
fn split_args(args: &[String]) -> Result<(Vec<String>, HashMap<String, String>), Fail> {
    let (mut pos, mut flags) = (Vec::new(), HashMap::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let Some(name) = a.strip_prefix("--") else {
            pos.push(a.clone());
            continue;
        };
        let (name, val) = match name.split_once('=') {
            Some((n, v)) => (n.to_string(), v.to_string()),
            None => (
                name.to_string(),
                it.next()
                    .ok_or_else(|| usage(format!("--{name} needs a value")))?
                    .clone(),
            ),
        };
        flags.insert(name, val);
    }
    Ok((pos, flags))
}

/// Who is writing. Never `me`: that name is the user's, and an agent must not pose as them.
fn agent<'a>(flag: Option<&'a str>, env: Option<&'a str>) -> Result<&'a str, Fail> {
    let a = flag.or(env).map(str::trim).filter(|a| !a.is_empty());
    match a {
        None => Err(Fail(2, "Say who is writing: --as claude".into())),
        Some("me") => Err(Fail(
            2,
            "--as me is refused: \"me\" is the user. Write under your own name, e.g. --as claude"
                .into(),
        )),
        Some(a) if !is_slug(a) => Err(Fail(
            2,
            format!("Agent name is a lowercase slug, e.g. --as claude (got ‘{a}’)"),
        )),
        Some(a) => Ok(a),
    }
}

fn pick_day(day: Option<&str>, today: NaiveDate) -> Result<NaiveDate, Fail> {
    match day {
        None => Ok(today),
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| Fail(1, ParseError::BadDate(d.to_string()).to_string())),
    }
}

/// `OPS-101`, or the first key-shaped part of a Jira URL (`…/browse/OPS-101`, `?selectedIssue=OPS-101`).
fn epic_key(arg: &str) -> Result<String, Fail> {
    arg.split(['/', '?', '&', '=', '#'])
        .find(|p| is_epic_key(p))
        .map(str::to_string)
        .ok_or_else(|| {
            Fail(
                1,
                format!("Can't find an epic key like ABC-123 in ‘{arg}’."),
            )
        })
}

fn one(pos: &[String], what: &str) -> Result<String, Fail> {
    match pos {
        [p] => Ok(p.clone()),
        _ => Err(usage(format!("give one {what}"))),
    }
}

fn log(
    conn: &mut Connection,
    today_: NaiveDate,
    pos: &[String],
    day: Option<&str>,
    author: &str,
) -> Result<Value, Fail> {
    // Unquoted words are one line, like typing them into the capture bar.
    let line = pos.join(" ");
    let day = dates::iso(pick_day(day, today_)?);
    let e = parse(&line, today_).map_err(|e| Fail(1, e.to_string()))?;
    let (id, new_epic) = today::insert_entry(conn, &day, &e, author).map_err(db)?;
    Ok(json!({
        "id": id,
        "day": day,
        "kind": e.kind.as_str(),
        "text": e.text,
        "topic": e.topic,
        "due": e.due.map(dates::iso),
        "link": e.link,
        "tags": tags_in(&e.text),
        "epic": e.epic_key.map(|k| json!({"key": k, "isNew": new_epic})),
        "author": author,
    }))
}

fn epic_json(e: &epics::EpicRow) -> Value {
    json!({
        "key": e.key,
        "title": e.title,
        "role": e.role,
        "okr": e.okr,
        "status": e.status,
        "start": e.start,
        "target": e.target,
        "doneOn": e.done_on,
        "metric": e.metric,
        "before": e.before,
        "after": e.after,
        "lastLog": e.last_log,
        "openPlans": e.open_plans,
        "author": e.author,
        "tags": parse_list(&e.tags),
    })
}

fn load(conn: &Connection, key: &str) -> Result<epics::EpicRow, Fail> {
    epics::epic(conn, key).map_err(db)?.ok_or_else(|| {
        Fail(
            1,
            format!("No epic {key} yet. File it with: atharu epic {key} --as <agent>"),
        )
    })
}

fn epic(
    conn: &Connection,
    pos: &[String],
    title: &str,
    okr: &str,
    tag_list: &str,
    author: &str,
) -> Result<Value, Fail> {
    let key = epic_key(&one(pos, "epic key or Jira link")?)?;
    // Same folding as the web field; like title/OKR it only fills an empty field (never overwrites mine).
    let tag_list = parse_list(tag_list).join(",");
    let created =
        epics::agent_upsert(conn, &key, title.trim(), okr.trim(), &tag_list, author).map_err(db)?;
    let mut v = epic_json(&load(conn, &key)?);
    v["created"] = created.into();
    Ok(v)
}

fn note(
    conn: &Connection,
    pos: &[String],
    file: Option<&str>,
    links: Option<&str>,
    id: Option<&str>,
    input: &mut dyn Read,
    author: &str,
) -> Result<Value, Fail> {
    let title = one(pos, "note title (quote it)")?.trim().to_string();
    if title.is_empty() {
        return Err(Fail(1, "Give the note a title.".into()));
    }
    let id = id
        .map(|i| {
            i.parse::<i64>()
                .map_err(|_| usage(format!("--id takes a note number, got ‘{i}’")))
        })
        .transpose()?;
    let mut body = String::new();
    match file {
        Some(path) => {
            body = std::fs::read_to_string(path)
                .map_err(|e| Fail(1, format!("Can't read ‘{path}’: {e}")))?
        }
        None => {
            input
                .read_to_string(&mut body)
                .map_err(|e| Fail(1, format!("Can't read the note body from stdin: {e}")))?;
        }
    }
    let mut body = body.trim_end().to_string();
    // `--links vector-search,OPS-101` → a closing `[[…]]` line, the same links the editor makes.
    let extra: Vec<String> = links
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| format!("[[{l}]]"))
        .collect();
    if !extra.is_empty() {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str(&format!("Links: {}", extra.join(" · ")));
    }
    let (id, updated) = match id {
        None => (
            notes::agent_create(conn, &title, &body, author).map_err(db)?,
            false,
        ),
        Some(id) => {
            if !notes::agent_update(conn, id, &title, &body, author).map_err(db)? {
                return Err(Fail(
                    1,
                    format!(
                        "Note {id} isn't one {author} wrote, so it stays as it is. Leave out --id to write a new note."
                    ),
                ));
            }
            (id, true)
        }
    };
    Ok(
        json!({"id": id, "title": title, "author": author, "updated": updated, "tags": tags_in(&body), "path": format!("/notes/{id}")}),
    )
}

fn evidence(conn: &Connection, pos: &[String], label: &str, author: &str) -> Result<Value, Fail> {
    let [key, url] = pos else {
        return Err(usage("give an epic key and one link"));
    };
    let key = epic_key(key)?;
    if !links::is_http(url) {
        return Err(Fail(1, links::NOT_HTTP.into()));
    }
    load(conn, &key)?;
    let id = epics::add_evidence(conn, &key, url, label.trim(), author).map_err(db)?;
    Ok(json!({"id": id, "epic": {"key": key}, "url": url, "note": label.trim(), "author": author}))
}

fn day(
    conn: &Connection,
    today_: NaiveDate,
    pos: &[String],
    day: Option<&str>,
) -> Result<Value, Fail> {
    if !pos.is_empty() {
        return Err(usage("today takes no words; use --day YYYY-MM-DD"));
    }
    let d = pick_day(day, today_)?;
    let entries: Vec<Value> = today::day_entries(conn, &dates::iso(d), &[])
        .map_err(db)?
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "kind": r.kind.as_str(),
                "topic": r.topic,
                "text": r.text,
                "epic": r.epic_key.map(|k| json!({"key": k, "title": r.epic_title})),
                "meeting": r.meeting,
                "tags": r.tags.iter().map(|(t, _)| t).collect::<Vec<_>>(),
                "author": r.author,
            })
        })
        .collect();
    let plans: Vec<Value> = today::plans(conn, &dates::iso(dates::week_start(today_)))
        .map_err(db)?
        .into_iter()
        .filter(|p| p.is_open())
        .map(|p| {
            json!({
                "id": p.id,
                "day": dates::iso(p.day),
                "text": p.text,
                "due": p.due.map(dates::iso),
                "link": p.link,
                "pinned": p.pinned,
                "epic": p.epic_key.map(|k| json!({"key": k})),
            })
        })
        .collect();
    Ok(json!({"day": dates::iso(d), "entries": entries, "openPlans": plans}))
}

fn show(conn: &Connection, pos: &[String]) -> Result<Value, Fail> {
    let key = epic_key(&one(pos, "epic key or Jira link")?)?;
    let mut v = epic_json(&load(conn, &key)?);
    v["evidence"] = epics::evidence(conn, &key)
        .map_err(db)?
        .into_iter()
        .map(|e| json!({"id": e.id, "url": e.url, "note": e.note, "author": e.author}))
        .collect();
    v["timeline"] = epics::timeline(conn, &key)
        .map_err(db)?
        .into_iter()
        .map(|t| {
            json!({
                "id": t.id,
                "day": t.day,
                "kind": t.kind.as_str(),
                "topic": t.topic,
                "text": t.text,
                "author": t.author,
            })
        })
        .collect();
    Ok(v)
}

/// Every tag in use, most used first: `{tags: [{tag, count, lastUsed}]}`; count includes inherited lines.
fn tag_list(conn: &Connection, pos: &[String]) -> Result<Value, Fail> {
    if !pos.is_empty() {
        return Err(usage("tags takes no words"));
    }
    let rows: Vec<Value> = tags::all(conn)
        .map_err(db)?
        .into_iter()
        .map(|r| json!({"tag": r.tag, "count": r.lines + r.notes, "lastUsed": r.last_used}))
        .collect();
    Ok(json!({ "tags": rows }))
}

fn db(e: rusqlite::Error) -> Fail {
    Fail(1, format!("Something broke: {e}"))
}
