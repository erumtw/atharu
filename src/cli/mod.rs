//! Agent CLI (spec "Agent access", D8): `atharu log | epic | note | evidence | today | show | tags`,
//! JSON on stdout. No delete or edit commands (D14): those stay with the user, on the web.
//! An entry point like app.rs: it calls the slices' own queries and `domain::parse`, so a line
//! filed by an agent goes through exactly the validation the capture bar uses.
use crate::{
    domain::{
        dates,
        entry::{NoteRef, is_epic_key, is_slug},
        links,
        parse::{ParseError, parse},
        tags::{parse_list, tags as tags_in},
    },
    epics::queries as epics,
    meetings::queries as meetings,
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
pub const COMMANDS: [&str; 11] = [
    "log", "edit", "epic", "note", "evidence", "today", "show", "meeting", "tags", "version",
    "update",
];

/// The command table for `atharu help` and every usage error (CLI and `config`), so they can't drift.
pub const HELP: &str = "usage: atharu [--db PATH] [command]
  atharu                                   serve http://127.0.0.1:7070 (ATHARU_HOST / ATHARU_PORT)
  atharu log \"<line>\" [--day YYYY-MM-DD] [--note N] [--meeting N] --as <agent>
  atharu edit <id> \"<line>\" --as <agent>   rewrite a line you wrote (same grammar; the owner's stay web-only)
  atharu epic <KEY|jira-url> [--title …] [--okr …] [--tags a,b] --as <agent>
  atharu note \"<title>\" [--file PATH] [--links a,b] [--id N] --as <agent>   (body from stdin without --file)
  atharu evidence <KEY|jira-url> <url> [--label …] --as <agent>
  atharu today [--day YYYY-MM-DD]
  atharu show <KEY|jira-url>
  atharu meeting <N|latest>                meeting + its lines + transcript (for /meeting notes)
  atharu tags
  atharu dump                              write dump.sql next to the DB
  atharu version                           {version, tag, sha, built}
  atharu update [--app] [--docker]         install the newest release tag if newer (scripts/install.sh)
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
            "log" => &["as", "day", "note", "meeting"],
            "edit" => &["as"],
            "epic" => &["as", "title", "okr", "tags"],
            "note" => &["as", "file", "links", "id"],
            "evidence" => &["as", "label"],
            "today" => &["day"],
            "show" | "tags" | "meeting" => &[],
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
                flag("note"),
                flag("meeting"),
                agent(flag("as"), env_agent)?,
            ),
            "edit" => edit(conn, today, &pos, agent(flag("as"), env_agent)?),
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
            "meeting" => meeting(conn, &pos),
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
    note: Option<&str>,
    meeting: Option<&str>,
    author: &str,
) -> Result<Value, Fail> {
    // Unquoted words are one line, like typing them into the capture bar.
    let line = pos.join(" ");
    let day = dates::iso(pick_day(day, today_)?);
    let e = parse(&line, today_).map_err(|e| Fail(1, e.to_string()))?;
    // Checked before the insert, so a bad --note files nothing.
    let note = match note {
        None => None,
        Some(n) => {
            let id = n
                .parse::<i64>()
                .map_err(|_| usage(format!("--note takes a note number, got ‘{n}’")))?;
            let row = notes::note(conn, id).map_err(db)?.ok_or_else(|| {
                Fail(
                    1,
                    format!("No note {id}. Link an existing note (see /notes)."),
                )
            })?;
            Some(json!({"id": id, "title": row.title}))
        }
    };
    let meeting = match meeting {
        None => None,
        Some(m) => Some(meeting_id(conn, m)?),
    };
    let (id, new_epic) = today::insert_entry(conn, &day, &e, author).map_err(db)?;
    if let Some(n) = &note {
        today::set_note(conn, id, n["id"].as_i64()).map_err(db)?;
    }
    if let Some(m) = meeting {
        today::set_meeting(conn, id, m).map_err(db)?;
    }
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
        "note": note,
        "meeting": meeting.map(|m| json!({"id": m})),
        "author": author,
    }))
}

/// Rewrites line `id` from a grammar line, keeping its day, meeting, note and author. Only the agent that wrote it
/// may: an agent fixing its own mistake (a misheard owner in /meeting) is fine, the owner's lines are not its to edit.
fn edit(
    conn: &mut Connection,
    today_: NaiveDate,
    pos: &[String],
    author: &str,
) -> Result<Value, Fail> {
    let (id, words) = pos
        .split_first()
        .ok_or_else(|| usage("edit takes a line number and the new line"))?;
    let id: i64 = id
        .parse()
        .map_err(|_| usage(format!("edit takes a line number first, got ‘{id}’")))?;
    let line = words.join(" ");
    if line.trim().is_empty() {
        return Err(usage("edit takes the new line after the number"));
    }
    match today::entry_author(conn, id).map_err(db)? {
        None => return Err(Fail(1, format!("No line {id}."))),
        Some(a) if a != author => {
            return Err(Fail(
                1,
                format!(
                    "Line {id} was written by {a}; only its author can edit it (the owner edits on the web)."
                ),
            ));
        }
        Some(_) => {}
    }
    let e = parse(&line, today_).map_err(|e| Fail(1, e.to_string()))?;
    let day = today::update_entry(conn, id, &e)
        .map_err(db)?
        .ok_or_else(|| Fail(1, format!("No line {id}.")))?;
    Ok(json!({
        "id": id,
        "day": day,
        "kind": e.kind.as_str(),
        "text": e.text,
        "topic": e.topic,
        "due": e.due.map(dates::iso),
        "link": e.link,
        "tags": tags_in(&e.text),
        "epic": e.epic_key.map(|k| json!({"key": k})),
        "author": author,
    }))
}

/// `N` or `latest` → an existing meeting's id (checked before anything is written).
fn meeting_id(conn: &Connection, arg: &str) -> Result<i64, Fail> {
    let id = if arg == "latest" {
        meetings::latest(conn)
            .map_err(db)?
            .ok_or_else(|| Fail(1, "No meetings yet.".into()))?
    } else {
        arg.parse::<i64>()
            .map_err(|_| usage(format!("meeting takes a number or ‘latest’, got ‘{arg}’")))?
    };
    meetings::meeting(conn, id)
        .map_err(db)?
        .ok_or_else(|| Fail(1, format!("No meeting {id}.")))?;
    Ok(id)
}

/// One meeting for an LLM to write notes from: the meeting, what was logged in it, and its transcript(s), segments
/// in time order with the mic's echo of the call already dropped. Read-only.
fn meeting(conn: &Connection, pos: &[String]) -> Result<Value, Fail> {
    let id = meeting_id(conn, &one(pos, "meeting number or ‘latest’")?)?;
    let m = meetings::meeting(conn, id).map_err(db)?.expect("checked");
    let lines: Vec<Value> = meetings::lines(conn, id)
        .map_err(db)?
        .into_iter()
        .map(|l| json!({"kind": l.kind.as_str(), "topic": l.topic, "text": l.text, "epic": l.epic_key.map(|k| json!({"key": k}))}))
        .collect();
    let plans: Vec<Value> = meetings::plans(conn, id)
        .map_err(db)?
        .into_iter()
        .map(|p| json!({"id": p.id, "text": p.text, "state": p.state, "due": p.due.map(dates::iso), "epic": p.epic_key.map(|k| json!({"key": k}))}))
        .collect();
    let transcripts: Vec<Value> = meetings::transcripts(conn, id)
        .map_err(db)?
        .into_iter()
        .map(|t| {
            json!({
                "id": t.id,
                "startedAt": t.started_at,
                "endedAt": t.ended_at,
                "status": t.status,
                "model": t.model,
                "lang": t.lang,
                "segments": t.segments.into_iter().map(|(t, speaker, text)| json!({"t": t, "speaker": speaker, "text": text})).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({
        "id": id,
        "day": m.day,
        "name": m.name,
        "notes": m.notes,
        "endedAt": m.ended_at,
        "lines": lines,
        "plans": plans,
        "transcripts": transcripts,
        // Absolute paths, so an agent can open the images (T89).
        "shots": crate::meetings::shots::root(conn)
            .map(|r| {
                crate::meetings::shots::list(&r, id)
                    .into_iter()
                    .map(|f| json!({"time": crate::meetings::shots::label(&f), "path": r.join(id.to_string()).join(f)}))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
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
            notes::insert(conn, &title, &body, author).map_err(db)?,
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
                "note": note_json(r.note),
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
                "note": note_json(p.note),
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
                "note": note_json(t.note),
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

/// A line's note as `note{id, title}` (null when it has none).
fn note_json(n: Option<NoteRef>) -> Value {
    n.map_or(Value::Null, |n| json!({"id": n.id, "title": n.title}))
}

fn db(e: rusqlite::Error) -> Fail {
    Fail(1, format!("Something broke: {e}"))
}
