pub mod api;
pub mod config;
pub mod mail;
pub mod worker;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct App {
    pub db: PgPool,
    pub config: Arc<config::Config>,
}
pub fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
#[derive(Debug)]
pub struct Error(pub StatusCode, pub String);
impl Error {
    pub fn bad(message: &str) -> Self {
        Self(StatusCode::BAD_REQUEST, message.into())
    }
    pub fn conflict(message: &str) -> Self {
        Self(StatusCode::CONFLICT, message.into())
    }
}
impl From<sqlx::Error> for Error {
    fn from(_: sqlx::Error) -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database operation failed; retry the same request ID or refresh its status".into(),
        )
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
pub type Result<T> = std::result::Result<T, Error>;
