use axum::{
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
use validator::ValidationErrors;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Authorization error: {0}")]
    Unauthorized(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Invalid invite code")]
    InvalidInviteCode,

    #[error("Invite code already used")]
    InviteCodeAlreadyUsed,

    #[error("Invite code expired")]
    InviteCodeExpired,

    #[error("Email already taken")]
    EmailTaken,

    #[error("Username already taken")]
    UsernameTaken,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token")]
    InvalidToken,

    #[error("No refresh token")]
    NoRefreshToken,

    #[error("Internal server error")]
    Internal(#[source] anyhow::Error),

    #[error("JSON error: {0}")]
    JsonRejection(#[from] JsonRejection),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl From<ValidationErrors> for AppError {
    fn from(errors: ValidationErrors) -> Self {
        AppError::Validation(errors.to_string())
    }
}

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        AppError::Redis(err.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            AppError::Redis(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            AppError::Auth(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::RateLimit => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded".to_string(),
            ),
            AppError::InvalidInviteCode => {
                (StatusCode::BAD_REQUEST, "Invalid invite code".to_string())
            }
            AppError::InviteCodeAlreadyUsed => (
                StatusCode::BAD_REQUEST,
                "Invite code already used".to_string(),
            ),
            AppError::InviteCodeExpired => {
                (StatusCode::BAD_REQUEST, "Invite code expired".to_string())
            }
            AppError::EmailTaken => (StatusCode::CONFLICT, "Email already taken".to_string()),
            AppError::UsernameTaken => (StatusCode::CONFLICT, "Username already taken".to_string()),
            AppError::InvalidCredentials => {
                (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string())
            }
            AppError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token expired".to_string()),
            AppError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token".to_string()),
            AppError::NoRefreshToken => (
                StatusCode::UNAUTHORIZED,
                "No refresh token provided".to_string(),
            ),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            AppError::JsonRejection(rejection) => (StatusCode::BAD_REQUEST, rejection.body_text()),
            AppError::Config(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Configuration error: {}", msg),
            ),
            AppError::Serialization(_) => (StatusCode::BAD_REQUEST, "Invalid JSON".to_string()),
        };

        let body = Json(json!({
            "error": true,
            "message": error_message,
            "code": format!("{}", self),
        }));

        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
