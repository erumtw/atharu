//! Agent CLI (spec "Agent access", D8): `atharu log | epic | today | show`, JSON on stdout.
//! An entry point like app.rs: it calls the slices' own queries and `domain::parse`, so a line
//! filed by an agent goes through exactly the validation the capture bar uses.
use crate::{
    domain::{
        dates,
        entry::{is_epic_key, is_slug},
        parse::{ParseError, parse},
    },
    epics::queries as epics,
    today::queries as today,
};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{collections::HashMap, io::Write};

pub const COMMANDS: [&str; 4] = ["log", "epic", "today", "show"];

const USAGE: &str = "usage:
  atharu log \"<line>\" [--day YYYY-MM-DD] --as <agent>
  atharu epic <KEY|jira-url> [--title …] [--okr …] --as <agent>
  atharu today [--day YYYY-MM-DD]
  atharu show <KEY|jira-url>
--as can come from ATHARU_AGENT instead.";

/// Exit code + message for stderr. 1 = input rejected (same copy as the web), 2 = usage.
struct Fail(i32, String);

fn usage(msg: impl Into<String>) -> Fail {
    Fail(2, format!("{}\n{USAGE}", msg.into()))
}

/// Runs one command. Exit code 0 prints one JSON document on `out`; anything else prints to `err`.
pub fn run(
    conn: &mut Connection,
    today: NaiveDate,
    args: &[String],
    env_agent: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let res = (|| {
        let (cmd, rest) = args.split_first().ok_or_else(|| usage("no command"))?;
        let (pos, flags) = split_args(rest)?;
        let allowed: &[&str] = match cmd.as_str() {
            "log" => &["as", "day"],
            "epic" => &["as", "title", "okr"],
            "today" => &["day"],
            "show" => &[],
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
                agent(flag("as"), env_agent)?,
            ),
            "today" => day(conn, today, &pos, flag("day")),
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
    author: &str,
) -> Result<Value, Fail> {
    let key = epic_key(&one(pos, "epic key or Jira link")?)?;
    let created = epics::agent_upsert(conn, &key, title.trim(), okr.trim(), author).map_err(db)?;
    let mut v = epic_json(&load(conn, &key)?);
    v["created"] = created.into();
    Ok(v)
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
    let entries: Vec<Value> = today::day_entries(conn, &dates::iso(d))
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

fn db(e: rusqlite::Error) -> Fail {
    Fail(1, format!("Something broke: {e}"))
}
