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
