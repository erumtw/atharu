use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Handler failure. Local single user: no retries, log one line and answer 500 (spec "Error handling").
#[derive(Debug)]
pub enum AppError {
    Db(rusqlite::Error),
    Render(askama::Error),
    NotFound,
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e)
    }
}

impl From<askama::Error> for AppError {
    fn from(e: askama::Error) -> Self {
        AppError::Render(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found.").into_response(),
            AppError::Db(e) => fail(&e),
            AppError::Render(e) => fail(&e),
        }
    }
}

fn fail(e: &dyn std::fmt::Display) -> Response {
    eprintln!("error: {e}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something broke. Nothing was lost; try again.",
    )
        .into_response()
}

pub type AppResult<T> = Result<T, AppError>;
