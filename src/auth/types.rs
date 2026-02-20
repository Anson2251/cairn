use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub email_verified: bool,
    pub username: String,
    pub hashed_password: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
    pub settings: serde_json::Value,
    pub invite_code_id: Option<Uuid>,
    pub trailblazer_seq: Option<i32>,
    #[allow(dead_code)]
    pub deleted_at: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
    #[allow(dead_code)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub email_verified: bool,
    pub username: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub settings: serde_json::Value,
    pub trailblazer_seq: Option<i32>,
    pub cairn_name: Option<String>,
    pub origin_coord: Option<(f64, f64)>,
}

#[derive(Debug, FromRow)]
pub struct InviteCode {
    pub id: Uuid,
    #[allow(dead_code)]
    pub sequence: i32,
    #[allow(dead_code)]
    pub code: String,
    pub cairn_name: String,
    pub origin_coord: Option<sqlx::postgres::types::PgPoint>,
    #[allow(dead_code)]
    pub memo: Option<String>,
    pub used: bool,
    #[allow(dead_code)]
    pub used_by: Option<Uuid>,
    #[allow(dead_code)]
    pub used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct RefreshToken {
    #[allow(dead_code)]
    pub id: Uuid,
    #[allow(dead_code)]
    pub user_id: Uuid,
    #[allow(dead_code)]
    pub token_hash: String,
    #[allow(dead_code)]
    pub client_id: Uuid,
    pub expires_at: DateTime<Utc>,
    #[allow(dead_code)]
    pub revoked: bool,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 3, max = 50, message = "Username must be 3-50 characters"))]
    pub username: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
    pub invite_code: Option<String>,
    pub client_id: Uuid,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    pub password: String,
    pub client_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub client_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: UserResponse,
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailblazer: Option<TrailblazerInfo>,
}

#[derive(Debug, Serialize)]
pub struct TrailblazerInfo {
    pub sequence: i32,
    pub cairn_name: String,
    pub origin_coord: (f64, f64),
}
