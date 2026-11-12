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
