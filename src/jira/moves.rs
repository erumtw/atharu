//! Jira status moves (0.2.5 D1, D8): the only write Atharu makes to Jira, and only on an explicit press.
//! The ▾ menu on a Jira chip reads the issue's allowed transitions live; a pick posts one transition, then a
//! 5 s toast offers Undo (the transition back to the previous status, if the workflow has one). Never guesses:
//! a transition that needs fields hands over to Jira. The token never reaches a log line or the page.
use super::queries;
use crate::{
    app::AppState,
    config::{JIRA_TOKEN_URL as TOKEN_URL, Jira},
    domain::entry::is_epic_key,
    error::{AppError, AppResult},
    web,
};
use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Response,
};
use serde::Deserialize;
use std::time::Duration;

/// One allowed move: Jira's transition id, its name, and the status it lands on.
#[derive(Clone, Debug, PartialEq)]
pub struct Move {
    pub id: String,
    pub name: String,
    pub to: String,
    /// The target's `statusCategory.key`: new · indeterminate · done.
    pub to_cat: String,
    /// Fields the move requires (their display names): such a move hands over to Jira.
    pub needs: Vec<String>,
}

#[derive(Deserialize)]
struct RawMoves {
    #[serde(default)]
    transitions: Vec<RawMove>,
    /// `GET /issue/KEY?fields=status&expand=transitions.fields` also carries the live status.
    fields: Option<RawIssueFields>,
}

#[derive(Deserialize)]
struct RawIssueFields {
    status: Option<RawTo>,
}

/// The issue as Jira has it right now: its status (+ category) and the moves allowed from there.
pub struct Live {
    pub status: String,
    pub cat: String,
    pub moves: Vec<Move>,
}

#[derive(Deserialize)]
struct RawMove {
    id: String,
    #[serde(default)]
    name: String,
    to: RawTo,
    #[serde(default)]
    fields: std::collections::BTreeMap<String, RawField>,
}

#[derive(Deserialize)]
struct RawTo {
    #[serde(default)]
    name: String,
    #[serde(rename = "statusCategory")]
    category: Option<RawCat>,
}

#[derive(Deserialize)]
struct RawCat {
    key: String,
}

#[derive(Deserialize)]
struct RawField {
    #[serde(default)]
    required: bool,
    #[serde(default)]
    name: String,
}

/// `GET …/transitions?expand=transitions.fields` body → moves.
pub fn parse_moves(json: &str) -> Result<Vec<Move>, String> {
    parse_live(json).map(|l| l.moves)
}

/// `GET /issue/KEY?fields=status&expand=transitions.fields` body → live status + moves.
pub fn parse_live(json: &str) -> Result<Live, String> {
    let raw: RawMoves = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let st = raw.fields.and_then(|f| f.status);
    let moves = raw
        .transitions
        .into_iter()
        .map(|t| Move {
            id: t.id,
            name: t.name,
            to: t.to.name,
            to_cat: t.to.category.map(|c| c.key).unwrap_or_default(),
            needs: t
                .fields
                .into_values()
                .filter(|f| f.required)
                .map(|f| f.name)
                .collect(),
        })
        .collect();
    Ok(Live {
        cat: st
            .as_ref()
            .and_then(|s| s.category.as_ref())
            .map(|c| c.key.clone())
            .unwrap_or_default(),
        status: st.map(|s| s.name).unwrap_or_default(),
        moves,
    })
}

/// What Jira answered, in the words the page shows.
pub(super) enum Fail {
    /// 401: the token is dead (D9).
    Token,
    /// 403: the token works, this account may not do that.
    Denied,
    /// Network or any other status.
    Unreachable,
}

fn client() -> Result<reqwest::Client, Fail> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| Fail::Unreachable)
}

fn issue_url(j: &Jira<'_>, key: &str, tail: &str) -> String {
    // `key` passed is_epic_key: letters, digits and one dash, nothing that bends the path.
    format!("{}/rest/api/3/issue/{key}{tail}", j.base_url)
}

/// The live status and its moves in one call (the cache can be 15 min old: Undo and the menu must not trust it).
/// The cache takes the live status on the way.
async fn fetch_live(st: &AppState, j: &Jira<'_>, key: &str) -> Result<Live, Fail> {
    let res = client()?
        .get(issue_url(
            j,
            key,
            "?fields=status&expand=transitions.fields",
        ))
        .basic_auth(j.email, Some(j.token))
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|_| Fail::Unreachable)?;
    let code = res.status().as_u16();
    auth(st, code)?;
    let body = res.text().await.map_err(|_| Fail::Unreachable)?;
    let live = parse_live(&body).map_err(|_| Fail::Unreachable)?;
    if !live.status.is_empty() {
        let _ = queries::moved(&st.conn(), key, &live.status, &live.cat);
    }
    Ok(live)
}

/// Only a 401 stores the dead token (D9); a 2xx or 403 (the token works, the account may not) clears it.
/// The refresh uses it too.
pub(super) fn auth(st: &AppState, code: u16) -> Result<(), Fail> {
    if code == 401 || code == 403 || (200..300).contains(&code) {
        let _ = queries::set_auth_failed(&st.conn(), code == 401);
    }
    match code {
        200..=299 => Ok(()),
        401 => Err(Fail::Token),
        403 => Err(Fail::Denied),
        _ => Err(Fail::Unreachable),
    }
}

/// POST one transition: `Ok(None)` moved, `Ok(Some(text))` Jira refused it (a field we never fill, or its own
/// message), in the toast's words.
async fn post_move(
    st: &AppState,
    j: &Jira<'_>,
    key: &str,
    m: &Move,
) -> Result<Option<String>, Fail> {
    let res = client()?
        .post(issue_url(j, key, "/transitions"))
        .basic_auth(j.email, Some(j.token))
        .header("content-type", "application/json")
        .body(serde_json::json!({ "transition": { "id": m.id } }).to_string())
        .send()
        .await
        .map_err(|_| Fail::Unreachable)?;
    let code = res.status().as_u16();
    if code == 400 {
        let body: serde_json::Value =
            serde_json::from_str(&res.text().await.unwrap_or_default()).unwrap_or_default();
        return Ok(Some(refusal(&body, m)));
    }
    auth(st, code)?;
    queries::moved(&st.conn(), key, &m.to, &m.to_cat).map_err(|_| Fail::Unreachable)?;
    Ok(None)
}

/// A 400's words: `{"errors": {"resolution": …}}` names a field (by the move's own label when known);
/// `{"errorMessages": ["…"]}` alone is Jira's own sentence, shown as is (the page only, never a log).
fn refusal(body: &serde_json::Value, m: &Move) -> String {
    let field = body["errors"]
        .as_object()
        .and_then(|e| e.keys().next().cloned());
    let said = body["errorMessages"]
        .as_array()
        .and_then(|a| a.iter().find_map(|v| v.as_str()))
        .filter(|s| !s.is_empty());
    match (field, said) {
        (Some(f), _) => {
            let f = m.needs.first().cloned().unwrap_or(f);
            format!("Jira needs {f} for “{}”. Open in Jira", m.to)
        }
        (None, Some(s)) => format!("Jira says: {s}"),
        (None, None) => format!(
            "Jira needs {} for “{}”. Open in Jira",
            m.needs.first().map_or("more", String::as_str),
            m.to
        ),
    }
}

fn checked(key: &str) -> AppResult<String> {
    if is_epic_key(key) {
        Ok(key.to_string())
    } else {
        Err(AppError::NotFound)
    }
}

fn browse(j: Option<&Jira<'_>>, key: &str) -> Option<String> {
    j.map(|j| format!("{}/browse/{key}", j.base_url))
}

/// The ▾ menu's body: the allowed moves (not the current status) + Open in Jira.
#[derive(Template)]
#[template(path = "jira/moves.html")]
pub struct Moves {
    pub key: String,
    pub moves: Vec<Move>,
    pub url: Option<String>,
    pub back: String,
    pub note: Option<String>,
    /// Jira's live status, named in the footer ("from “In review”").
    pub current: String,
    pub token_url: &'static str,
    pub dead_token: bool,
}

#[derive(Deserialize)]
pub struct BackQ {
    back: Option<String>,
}

pub async fn moves(
    State(st): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<BackQ>,
) -> AppResult<Response> {
    let key = checked(&key)?;
    let j = st.settings.jira();
    let mut page = Moves {
        url: browse(j.as_ref(), &key),
        key: key.clone(),
        moves: Vec::new(),
        back: web::safe_back(q.back.as_deref(), "/epics").to_string(),
        note: None,
        current: String::new(),
        token_url: TOKEN_URL,
        dead_token: false,
    };
    if let Some(j) = &j {
        match fetch_live(&st, j, &key).await {
            Ok(l) => {
                page.moves = l.moves.into_iter().filter(|m| m.to != l.status).collect();
                page.current = l.status;
            }
            Err(Fail::Token) => page.dead_token = true,
            Err(Fail::Denied) => page.note = Some(DENIED.into()),
            Err(Fail::Unreachable) => page.note = Some("Couldn't reach Jira.".into()),
        }
    }
    web::html(&page)
}

const DENIED: &str = "Jira says this account may not move it.";

impl Moves {
    /// A move button's `hx-vals`: serialised, never concatenated (`back` is a path the page passed in, quotes and all).
    pub fn vals(&self, m: &Move) -> String {
        serde_json::json!({ "to": m.id, "back": self.back }).to_string()
    }
}

/// A cancel-type move (D10): to a done-category status named like Won't do / Cancelled / Rejected.
/// ponytail: English words only; a workflow that names its cancel status otherwise gets no "dropped" prompt (D10
/// says no prompt is the safe miss), add words when a real workflow shows one.
pub fn is_cancel(m: &Move) -> bool {
    const WORDS: [&str; 8] = [
        "won't",
        "wont",
        "cancel",
        "reject",
        "abandon",
        "invalid",
        "duplicate",
        "not do",
    ];
    let text = format!("{} {}", m.name, m.to).to_lowercase();
    m.to_cat == "done" && WORDS.iter().any(|w| text.contains(w))
}

/// "Also move KEY in Jira?" (D2, D10): after Atharu marks it done, the done-category moves that aren't a cancel;
/// after dropped, the cancel-type ones. None match → nothing (no prompt).
#[derive(Template)]
#[template(path = "jira/suggest.html")]
pub struct Suggest {
    pub key: String,
    pub moves: Vec<Move>,
    pub back: String,
}

#[derive(Deserialize)]
pub struct SuggestQ {
    #[serde(rename = "for")]
    after: String,
    back: Option<String>,
}

pub async fn suggest(
    State(st): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<SuggestQ>,
) -> AppResult<Response> {
    let key = checked(&key)?;
    let mut s = Suggest {
        key: key.clone(),
        moves: Vec::new(),
        back: web::safe_back(q.back.as_deref(), "/epics").to_string(),
    };
    if let Some(j) = st.settings.jira()
        && let Ok(live) = fetch_live(&st, &j, &key).await
    {
        let dropped = q.after == "dropped";
        s.moves = live
            .moves
            .into_iter()
            .filter(|m| m.to_cat == "done" && is_cancel(m) == dropped)
            .collect();
    }
    web::html(&s)
}

/// After a move or an undo: what happened, with Undo while it can still be undone.
#[derive(Template)]
#[template(path = "jira/toast.html")]
pub struct Toast {
    pub key: String,
    pub text: String,
    /// The status to go back to (Undo), when this was a move.
    pub undo: Option<String>,
    /// The new status and its category, for the chips on the page (out-of-band).
    pub status: Option<String>,
    pub cat: String,
    pub url: Option<String>,
    pub back: String,
    pub token_url: &'static str,
    pub dead_token: bool,
}

#[derive(Deserialize)]
pub struct MoveForm {
    /// Transition id (move) or the status to return to (undo).
    to: String,
    back: Option<String>,
}

async fn act(
    st: AppState,
    headers: HeaderMap,
    key: String,
    f: MoveForm,
    undo: bool,
) -> AppResult<Response> {
    let key = checked(&key)?;
    let back = web::safe_back(f.back.as_deref(), "/epics").to_string();
    let Some(j) = st.settings.jira() else {
        return Ok(web::back(&back));
    };
    let mut t = Toast {
        url: browse(Some(&j), &key),
        key: key.clone(),
        text: String::new(),
        undo: None,
        status: None,
        cat: String::new(),
        back: back.clone(),
        token_url: TOKEN_URL,
        dead_token: false,
    };
    // Jira's status right now, not the cache: Undo must return to what the issue really was.
    let mut before = String::new();
    let outcome = match fetch_live(&st, &j, &key).await {
        Err(e) => Err(e),
        Ok(live) => {
            before = live.status;
            // A move names its transition; an undo names the status it returns to (D8), by a transition that needs
            // no fields when the workflow has one (fewest required fields first).
            let pick = if undo {
                live.moves
                    .into_iter()
                    .filter(|m| m.to == f.to)
                    .min_by_key(|m| m.needs.len())
            } else {
                live.moves.into_iter().find(|m| m.id == f.to)
            };
            match pick {
                None if undo => {
                    t.text = format!("Can't undo in Jira: open {key}");
                    Ok(None)
                }
                None => {
                    t.text = format!("Jira no longer offers that move for {key}.");
                    Ok(None)
                }
                Some(m) => match post_move(&st, &j, &key, &m).await {
                    Ok(Some(text)) => {
                        t.text = text;
                        Ok(None)
                    }
                    Ok(None) => Ok(Some(m)),
                    Err(e) => Err(e),
                },
            }
        }
    };
    match outcome {
        Ok(Some(m)) => {
            t.text = if undo {
                format!("Moved {key} back to {} in Jira", m.to)
            } else {
                format!("Moved {key} to {} in Jira", m.to)
            };
            t.undo = (!undo && !before.is_empty() && before != m.to).then_some(before);
            t.status = Some(m.to);
            t.cat = m.to_cat;
        }
        Ok(None) => {}
        Err(Fail::Token) => t.dead_token = true,
        Err(Fail::Denied) => {
            t.text = format!("Jira says this account may not move {key}. Nothing moved.")
        }
        Err(Fail::Unreachable) => t.text = "Couldn't reach Jira. Nothing moved.".into(),
    }
    if !web::is_htmx(&headers) {
        return Ok(web::back(&back));
    }
    web::html(&t)
}

pub async fn move_to(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Form(f): Form<MoveForm>,
) -> AppResult<Response> {
    act(st, headers, key, f, false).await
}

pub async fn undo(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Form(f): Form<MoveForm>,
) -> AppResult<Response> {
    act(st, headers, key, f, true).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_moves_with_required_fields() {
        let m = parse_moves(
            r#"{"transitions": [
              {"id": "31", "name": "Start", "to": {"name": "In Progress", "statusCategory": {"key": "indeterminate"}}},
              {"id": "41", "name": "Done", "to": {"name": "Done", "statusCategory": {"key": "done"}},
               "fields": {"resolution": {"required": true, "name": "Resolution"}, "comment": {"required": false, "name": "Comment"}}}
            ]}"#,
        )
        .unwrap();
        assert_eq!(m[0].to_cat, "indeterminate");
        assert_eq!(m[1].needs, ["Resolution"]);
        assert!(parse_moves("{").is_err());
    }
}
