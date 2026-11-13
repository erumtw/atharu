//! Jira status moves (0.2.5 D1, D8): the only write Atharu makes to Jira, and only on an explicit press.
//! The ▾ menu on a Jira chip reads the issue's allowed transitions live; a pick posts one transition, then a
//! 5 s toast offers Undo (the transition back to the previous status, if the workflow has one). Never guesses:
//! a transition that needs fields hands over to Jira. The token never reaches a log line or the page.
use super::queries;
use crate::{
    app::AppState,
    config::Jira,
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

/// D9: where the owner makes a new API token.
pub const TOKEN_URL: &str = "https://id.atlassian.com/manage-profile/security/api-tokens";

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
    let raw: RawMoves = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(raw
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
        .collect())
}

/// What Jira answered, in the words the page shows.
pub(super) enum Fail {
    /// 401 / 403 (D9).
    Token,
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
    format!("{}/rest/api/3/issue/{key}/transitions{tail}", j.base_url)
}

async fn fetch_moves(st: &AppState, j: &Jira<'_>, key: &str) -> Result<Vec<Move>, Fail> {
    let res = client()?
        .get(issue_url(j, key, "?expand=transitions.fields"))
        .basic_auth(j.email, Some(j.token))
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|_| Fail::Unreachable)?;
    let code = res.status().as_u16();
    auth(st, code)?;
    let body = res.text().await.map_err(|_| Fail::Unreachable)?;
    parse_moves(&body).map_err(|_| Fail::Unreachable)
}

/// 401 / 403 stores the dead token (D9); a 2xx clears it. The refresh uses it too.
pub(super) fn auth(st: &AppState, code: u16) -> Result<(), Fail> {
    let dead = code == 401 || code == 403;
    if dead || (200..300).contains(&code) {
        let _ = queries::set_auth_failed(&st.conn(), dead);
    }
    match code {
        200..=299 => Ok(()),
        401 | 403 => Err(Fail::Token),
        _ => Err(Fail::Unreachable),
    }
}

/// POST one transition: `Ok(None)` moved, `Ok(Some(field))` Jira wants a field we never fill.
async fn post_move(
    st: &AppState,
    j: &Jira<'_>,
    key: &str,
    m: &Move,
) -> Result<Option<String>, Fail> {
    let res = client()?
        .post(issue_url(j, key, ""))
        .basic_auth(j.email, Some(j.token))
        .header("content-type", "application/json")
        .body(serde_json::json!({ "transition": { "id": m.id } }).to_string())
        .send()
        .await
        .map_err(|_| Fail::Unreachable)?;
    let code = res.status().as_u16();
    if code == 400 {
        // `{"errors": {"resolution": "Resolution is required."}}`: name the field, by the move's own label when known.
        let body: serde_json::Value =
            serde_json::from_str(&res.text().await.unwrap_or_default()).unwrap_or_default();
        let field = body["errors"]
            .as_object()
            .and_then(|e| e.keys().next().cloned())
            .unwrap_or_else(|| "a field".into());
        return Ok(Some(m.needs.first().cloned().unwrap_or(field)));
    }
    auth(st, code)?;
    queries::moved(&st.conn(), key, &m.to, &m.to_cat).map_err(|_| Fail::Unreachable)?;
    Ok(None)
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
        token_url: TOKEN_URL,
        dead_token: false,
    };
    if let Some(j) = &j {
        let current = queries::status(&st.conn(), &key)
            .ok()
            .flatten()
            .unwrap_or_default();
        match fetch_moves(&st, j, &key).await {
            Ok(m) => page.moves = m.into_iter().filter(|m| m.to != current).collect(),
            Err(Fail::Token) => page.dead_token = true,
            Err(Fail::Unreachable) => page.note = Some("Couldn't reach Jira.".into()),
        }
    }
    web::html(&page)
}

/// A cancel-type move (D10): to a done-category status named like Won't do / Cancelled / Rejected.
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
        && let Ok(all) = fetch_moves(&st, &j, &key).await
    {
        let dropped = q.after == "dropped";
        s.moves = all
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
    /// The new status, for the chips on the page (out-of-band).
    pub status: Option<String>,
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
        back: back.clone(),
        token_url: TOKEN_URL,
        dead_token: false,
    };
    let before = queries::status(&st.conn(), &key)
        .ok()
        .flatten()
        .unwrap_or_default();
    let outcome = match fetch_moves(&st, &j, &key).await {
        Err(e) => Err(e),
        Ok(all) => {
            // A move names its transition; an undo names the status it returns to (D8).
            let pick = all
                .into_iter()
                .find(|m| if undo { m.to == f.to } else { m.id == f.to });
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
                    Ok(Some(field)) => {
                        t.text = format!("Jira needs {field} for “{}”. Open in Jira", m.to);
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
            t.status = Some(m.to);
            t.undo = (!undo && !before.is_empty()).then_some(before);
        }
        Ok(None) => {}
        Err(Fail::Token) => t.dead_token = true,
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
