//! Jira read-only titles (spec P6): summary · status for keys we already have, cached in
//! `jira_issues`. Never writes to Jira, never creates our rows. No page imports this slice: pages
//! read the cache table through their own queries. The token never reaches a log line.
mod queries;

pub use queries::Issue;

use crate::{app::AppState, config::Jira, domain::plan::jira_key, error::AppResult, web};
use axum::{Form, Router, extract::State, response::Response, routing::post};
use rusqlite::Connection;
use serde::Deserialize;
use std::{collections::BTreeSet, time::Duration};

pub fn routes() -> Router<AppState> {
    Router::new().route("/jira/refresh", post(refresh_button))
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
}

#[derive(Deserialize)]
struct RawStatus {
    name: String,
}

/// One `search/jql` response body → issues.
pub fn parse(json: &str) -> Result<Vec<Issue>, String> {
    let page: SearchPage = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(page
        .issues
        .into_iter()
        .map(|i| Issue {
            key: i.key,
            summary: i.fields.summary,
            status: i.fields.status.map(|s| s.name).unwrap_or_default(),
        })
        .collect())
}

/// Parse + cache one response body; returns how many issues it held. Tests call this with a fixture.
pub fn import(conn: &mut Connection, json: &str) -> Result<usize, String> {
    let issues = parse(json)?;
    queries::upsert(conn, &issues).map_err(|e| e.to_string())?;
    Ok(issues.len())
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
    keys: &[String],
) -> Result<(u16, String), String> {
    // reqwest errors can quote the URL; ours holds no secret, but keep the habit from ical.rs.
    let hide = |e: reqwest::Error| e.without_url().to_string();
    let jql = format!("key in ({})", keys.join(","));
    let url = reqwest::Url::parse_with_params(
        &format!("{}/rest/api/3/search/jql", j.base_url),
        [
            ("jql", jql.as_str()),
            ("fields", "summary,status"),
            ("maxResults", "100"),
        ],
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
    let mut n = 0;
    for chunk in keys.chunks(BATCH) {
        let (mut code, mut body) = search(&client, &j, chunk).await?;
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
            (code, body) = search(&client, &j, &rest).await?;
        }
        if !(200..300).contains(&code) {
            // Status only: the body may echo the request, never the header, but keep logs short.
            return Err(format!("Jira answered {code}"));
        }
        n += import(&mut st.conn(), &body)?;
    }
    Ok(Some(n))
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
        Ok(_) => web::back(&back),
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
}
