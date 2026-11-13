//! Jira (spec P6, 0.2.5 D1–D10): summary, status, type, parent, due, resolved and updated for keys we already have,
//! plus the open issues assigned to me, cached in `jira_issues`. Never creates our rows. No page imports this slice:
//! pages read the cache table through their own queries. The token never reaches a log line.
mod moves;
mod queries;

pub use moves::{Move, parse_moves};
pub use queries::Issue;

use crate::{app::AppState, config::Jira, domain::plan::jira_key, error::AppResult, web};
use axum::{
    Form, Router,
    extract::State,
    response::Response,
    routing::{get, post},
};
use rusqlite::Connection;
use serde::Deserialize;
use std::{collections::BTreeSet, time::Duration};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/jira/refresh", post(refresh_button))
        .route("/jira/{key}/moves", get(moves::moves))
        .route("/jira/{key}/move", post(moves::move_to))
        .route("/jira/{key}/undo", post(moves::undo))
        .route("/jira/{key}/suggest", get(moves::suggest))
}

#[derive(Deserialize)]
struct SearchPage {
    #[serde(default)]
    issues: Vec<RawIssue>,
}

#[derive(Deserialize)]
struct RawIssue {
    key: String,
    fields: RawFields,
}

#[derive(Deserialize)]
struct RawFields {
    #[serde(default)]
    summary: String,
    status: Option<RawStatus>,
    issuetype: Option<RawType>,
    parent: Option<RawParent>,
    duedate: Option<String>,
    resolutiondate: Option<String>,
    updated: Option<String>,
}

#[derive(Deserialize)]
struct RawStatus {
    name: String,
    #[serde(rename = "statusCategory")]
    category: Option<RawCategory>,
}

#[derive(Deserialize)]
struct RawCategory {
    key: String,
}

#[derive(Deserialize)]
struct RawType {
    name: String,
    /// 1 = the epic level (D16), whatever the team named the type.
    #[serde(rename = "hierarchyLevel")]
    level: Option<i64>,
}

#[derive(Deserialize)]
struct RawParent {
    key: String,
    fields: Option<RawParentFields>,
}

#[derive(Deserialize)]
struct RawParentFields {
    #[serde(default)]
    summary: String,
}

/// What every search asks for.
const FIELDS: &str = "summary,status,issuetype,parent,duedate,resolutiondate,updated";
/// D5: open work assigned to me, any project, newest first.
const ASSIGNED_JQL: &str =
    "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";

/// One `search/jql` response body → issues.
pub fn parse(json: &str) -> Result<Vec<Issue>, String> {
    let page: SearchPage = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(page
        .issues
        .into_iter()
        .map(|i| {
            let f = i.fields;
            // Jira timestamps: `2026-11-12T10:20:30.000+0700`; the day and minute are enough here.
            let cut = |s: Option<String>, n: usize| s.and_then(|s| s.get(..n).map(str::to_string));
            Issue {
                key: i.key,
                summary: f.summary,
                status_cat: f
                    .status
                    .as_ref()
                    .and_then(|s| s.category.as_ref())
                    .map(|c| c.key.clone())
                    .unwrap_or_default(),
                status: f.status.map(|s| s.name).unwrap_or_default(),
                level: f.issuetype.as_ref().and_then(|t| t.level),
                issue_type: f.issuetype.map(|t| t.name).unwrap_or_default(),
                parent_summary: f
                    .parent
                    .as_ref()
                    .and_then(|p| p.fields.as_ref())
                    .map(|pf| pf.summary.clone()),
                parent_key: f.parent.map(|p| p.key),
                due: cut(f.duedate, 10),
                resolved_on: cut(f.resolutiondate, 10),
                updated_at: cut(f.updated, 16),
            }
        })
        .collect())
}

/// Parse + cache one response body; returns how many issues it held. Tests call this with a fixture.
pub fn import(conn: &mut Connection, json: &str) -> Result<usize, String> {
    Ok(import_keys(conn, json)?.len())
}

/// `import`, returning the keys it cached (the assigned search flags exactly those).
fn import_keys(conn: &mut Connection, json: &str) -> Result<Vec<String>, String> {
    let issues = parse(json)?;
    queries::upsert(conn, &issues).map_err(|e| e.to_string())?;
    Ok(issues.into_iter().map(|i| i.key).collect())
}

/// Epic keys + keys in Jira links, deduped and sorted (stable JQL, stable tests).
pub fn known_keys(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let (mut keys, links) = queries::epic_keys_and_links(conn)?;
    keys.extend(links.iter().filter_map(|l| jira_key(l)).map(str::to_string));
    Ok(keys
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

/// Keys Jira names in a 400 (`An issue with key 'OPS-9' does not exist …`), so one typo'd stub
/// epic doesn't make the whole batch fail forever.
fn rejected(body: &str, keys: &[String]) -> Vec<String> {
    keys.iter()
        .filter(|k| body.contains(&format!("'{k}'")))
        .cloned()
        .collect()
}

// Jira caps `key in (…)` lists and page size; 100 keys per call keeps one page per call.
const BATCH: usize = 100;

async fn search(
    client: &reqwest::Client,
    j: &Jira<'_>,
    jql: &str,
    max: &str,
) -> Result<(u16, String), String> {
    // reqwest errors can quote the URL; ours holds no secret, but keep the habit from ical.rs.
    let hide = |e: reqwest::Error| e.without_url().to_string();
    let url = reqwest::Url::parse_with_params(
        &format!("{}/rest/api/3/search/jql", j.base_url),
        [("jql", jql), ("fields", FIELDS), ("maxResults", max)],
    )
    .map_err(|e| format!("jira_base_url: {e}"))?;
    let res = client
        .get(url)
        .basic_auth(j.email, Some(j.token))
        .header("accept", "application/json")
        .send()
        .await
        .map_err(hide)?;
    let code = res.status().as_u16();
    Ok((code, res.text().await.map_err(hide)?))
}

/// One refresh. `Ok(None)` = Jira not configured (then no request is made). Like the calendar,
/// the DB lock is taken only between fetches, so a slow Jira never stalls page loads.
pub async fn refresh(st: &AppState) -> Result<Option<usize>, String> {
    let Some(j) = st.settings.jira() else {
        return Ok(None);
    };
    let keys = known_keys(&st.conn()).map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let by_keys = |keys: &[String]| format!("key in ({})", keys.join(","));
    let mut n = 0;
    for chunk in keys.chunks(BATCH) {
        let (mut code, mut body) = search(&client, &j, &by_keys(chunk), "100").await?;
        if code == 400 {
            let bad = rejected(&body, chunk);
            let rest: Vec<String> = chunk.iter().filter(|k| !bad.contains(k)).cloned().collect();
            if bad.is_empty() || rest.is_empty() {
                // Skipped keys refresh next time; status only, the body may hold work data.
                eprintln!(
                    "Jira answered 400 naming no key to drop; skipping {} keys",
                    chunk.len()
                );
                continue;
            }
            (code, body) = search(&client, &j, &by_keys(&rest), "100").await?;
        }
        check(st, code)?;
        n += import(&mut st.conn(), &body)?;
    }
    // D5: the open issues assigned to me, cached too (they may be keys we don't track yet) and flagged.
    let (code, body) = search(&client, &j, ASSIGNED_JQL, "50").await?;
    check(st, code)?;
    let mine = import_keys(&mut st.conn(), &body)?;
    queries::mark_assigned(&st.conn(), &mine).map_err(|e| e.to_string())?;
    Ok(Some(n + mine.len()))
}

/// A 2xx clears the stored token failure; a 401 / 403 stores it (D9: Settings and Epics name it, the cache stays).
fn check(st: &AppState, code: u16) -> Result<(), String> {
    // Status only: the body may echo the request, never the header, but keep logs short.
    moves::auth(st, code).map_err(|_| format!("Jira answered {code}"))
}

#[derive(Deserialize)]
struct Back {
    back: Option<String>,
}

/// "Refresh" on /epics. Plain form, 303 back; a failure says so on the page it returns to.
async fn refresh_button(State(st): State<AppState>, Form(f): Form<Back>) -> AppResult<Response> {
    let back = web::safe_back(f.back.as_deref(), "/epics").to_string();
    Ok(match refresh(&st).await {
        Err(e) => {
            eprintln!("jira refresh: {e}");
            let sep = if back.contains('?') { '&' } else { '?' };
            web::back(&format!("{back}{sep}jira=failed"))
        }
        Ok(_) => {
            // Say it ran: an unchanged list looked like a dead button.
            let sep = if back.contains('?') { '&' } else { '?' };
            web::back(&format!("{back}{sep}jira=ok"))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_search_page_and_names_rejected_keys() {
        let issues = parse(include_str!("../../tests/fixtures/jira_search.json")).unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(
            (
                issues[0].key.as_str(),
                issues[0].summary.as_str(),
                issues[0].status.as_str()
            ),
            ("OPS-101", "Faster checkout", "In Progress")
        );
        let keys = ["OPS-101".to_string(), "OPS-9".to_string()];
        let body =
            r#"{"errorMessages":["An issue with key 'OPS-9' does not exist for field 'key'."]}"#;
        assert_eq!(rejected(body, &keys), ["OPS-9"]);
        assert!(parse("not json").is_err());
    }

    #[test]
    fn parses_type_category_parent_and_dates() {
        let issues = parse(include_str!("../../tests/fixtures/jira_search_full.json")).unwrap();
        assert_eq!(
            issues[1],
            Issue {
                key: "OPS-131".into(),
                summary: "Alert at 6°C for 10 min".into(),
                status: "Done".into(),
                issue_type: "Story".into(),
                level: Some(0),
                status_cat: "done".into(),
                parent_key: Some("OPS-130".into()),
                parent_summary: Some("Cold-chain delivery alerts".into()),
                due: None,
                resolved_on: Some("2026-11-12".into()),
                updated_at: Some("2026-11-12T10:20".into()),
            }
        );
        assert_eq!(
            (
                issues[0].status_cat.as_str(),
                issues[0].due.as_deref(),
                issues[0].parent_key.as_deref()
            ),
            ("indeterminate", Some("2026-11-30"), None)
        );
    }
}
