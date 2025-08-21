use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("database error")]
    Db,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!("Request failed with error: {:?}", self);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self.to_string())).into_response()
    }
}

impl From<surrealdb::Error> for Error {
    fn from(err: surrealdb::Error) -> Self {
        error!("Database error occurred: {:?}", err);
        Self::Db
    }
}
