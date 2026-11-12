//! Small helpers every slice uses: render a template, tell htmx from a plain form, redirect back.
use crate::error::AppResult;
use askama::Template;
use axum::{
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};

pub fn html(t: &impl Template) -> AppResult<Response> {
    Ok(Html(t.render()?).into_response())
}

pub fn html_status(status: StatusCode, t: &impl Template) -> AppResult<Response> {
    Ok((status, Html(t.render()?)).into_response())
}

/// htmx sends `HX-Request: true`; anything else is a plain form post that wants a redirect.
pub fn is_htmx(headers: &HeaderMap) -> bool {
    headers.contains_key("hx-request")
}

/// A form's `back` if it is a same-site path, else `fallback`. Browsers read `\` as `/`, so
/// `/\evil.com` would be `//evil.com` (another host): refuse a backslash anywhere.
pub fn safe_back<'a>(back: Option<&'a str>, fallback: &'a str) -> &'a str {
    match back {
        Some(b) if b.starts_with('/') && !b.starts_with("//") && !b.contains('\\') => b,
        _ => fallback,
    }
}

/// A query param of the page an htmx request came from (`HX-Current-URL`), so a swap keeps that
/// page's filter / grouping. Values here are tag lists and modes: only `%2C` (comma) needs decoding.
pub fn page_param(headers: &HeaderMap, key: &str) -> Option<String> {
    let url = headers.get("hx-current-url")?.to_str().ok()?;
    let query = url.split_once('?')?.1;
    query
        .split('&')
        .find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
        .map(|v| v.replace("%2C", ",").replace("%2c", ","))
}

/// 303 so the browser follows with GET (no "resubmit form?" on refresh).
pub fn back(to: &str) -> Response {
    Redirect::to(to).into_response()
}

/// htmx answer to deleting a note / meeting / topic / epic: the page's `<main>` becomes this
/// Undo toast. The deleted row travels in `snap` (JSON from SQLite) to `restore`, so nothing
/// needs a `deleted_at` column and every other query stays as it is.
#[derive(Template)]
#[template(path = "partials/deleted.html")]
pub struct Deleted {
    /// `Note “Plan”`, shown as "… deleted".
    pub what: String,
    pub restore: String,
    pub snap: String,
    pub back: &'static str,
    pub back_label: &'static str,
    /// Why the undo didn't work (the name was taken again, …).
    pub error: Option<String>,
}

/// After an undo: htmx follows `HX-Redirect`, a plain form follows the 303.
pub fn restored(headers: &HeaderMap, to: &str) -> Response {
    if is_htmx(headers) {
        ([("hx-redirect", to.to_string())], StatusCode::OK).into_response()
    } else {
        back(to)
    }
}

#[cfg(test)]
mod tests {
    use super::safe_back;

    #[test]
    fn back_only_follows_same_site_paths() {
        assert_eq!(safe_back(Some("/todos?x=1"), "/"), "/todos?x=1");
        for bad in [
            "//evil.com",
            "/\\evil.com",
            "/a\\b",
            "https://evil.com",
            "evil",
            "",
        ] {
            assert_eq!(safe_back(Some(bad), "/home"), "/home", "{bad}");
        }
        assert_eq!(safe_back(None, "/home"), "/home");
    }
}
