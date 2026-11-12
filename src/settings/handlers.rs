use super::{queries, view};
use crate::{
    app::AppState,
    build_info,
    domain::look::{self, LOOKS},
    error::AppResult,
    layout::Layout,
    web,
};
use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

pub async fn page(State(st): State<AppState>) -> AppResult<Response> {
    let conn = st.conn();
    let layout = Layout::load(&conn, "settings", "Settings", vec![])?;
    web::html(&view::Page {
        cur: look::pick(Some(layout.look)),
        layout,
        looks: &LOOKS,
        google: st.settings.ical_url.is_some(),
        jira: st.settings.jira().is_some(),
        version: build_info::TAG.map_or_else(
            || format!("{} · dev build", build_info::VERSION),
            str::to_string,
        ),
        sha: build_info::SHA,
        built: build_info::BUILT,
    })
}

#[derive(Deserialize)]
pub struct LookForm {
    look: String,
}

/// Plain form POST, 303 back: the look lives on `<html>`, so the whole page has to reload anyway
/// (an htmx partial couldn't repaint the wallpaper or favicon).
pub async fn save(State(st): State<AppState>, Form(f): Form<LookForm>) -> AppResult<Response> {
    if !LOOKS.iter().any(|l| l.slug == f.look) {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, "Pick one of the looks.").into_response());
    }
    queries::set(&st.conn(), "look", &f.look)?;
    Ok(web::back("/settings"))
}

#[derive(Deserialize)]
pub struct GlassForm {
    /// Checkbox: present = on.
    glass_all: Option<String>,
    glass_opacity: Option<u8>,
    text_blur: Option<String>,
    /// Which preset button submitted the form, if one did.
    preset: Option<String>,
}

/// htmx saves on change (204, the page already previewed it); a plain form POST gets a 303 back.
/// A preset sets tint + alpha together; the slider alone only moves alpha.
pub async fn save_glass(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<GlassForm>,
) -> AppResult<Response> {
    let conn = st.conn();
    let floor = look::pick(queries::get(&conn, "look")?.as_deref()).glass_floor;
    let (tint, alpha) = match f.preset.as_deref() {
        Some("clear") => (Some("clear"), floor),
        Some("tinted") => (Some("tinted"), look::GLASS_TINTED.max(floor)),
        Some(_) => {
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, "Pick Clear or Tinted.").into_response());
        }
        None => (
            None,
            f.glass_opacity.unwrap_or(look::GLASS_DEFAULT).min(100),
        ),
    };
    queries::set(
        &conn,
        "glass_all",
        if f.glass_all.is_some() { "1" } else { "0" },
    )?;
    queries::set(
        &conn,
        "text_blur",
        if f.text_blur.is_some() { "1" } else { "0" },
    )?;
    queries::set(&conn, "glass_opacity", &alpha.to_string())?;
    if let Some(t) = tint {
        queries::set(&conn, "glass_tint", t)?;
    }
    Ok(if web::is_htmx(&headers) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        web::back("/settings")
    })
}
