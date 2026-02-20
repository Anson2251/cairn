use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::JwtConfig;
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    #[allow(dead_code)]
    pub email: String,
    #[allow(dead_code)]
    pub username: String,
    pub role: String,
    #[allow(dead_code)]
    pub email_verified: bool,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: Uuid,
    pub client_id: Uuid,
    pub jti: Uuid,
    pub iat: i64,
    pub exp: i64,
}

pub struct JwtManager {
    access_encoding_key: EncodingKey,
    access_decoding_key: DecodingKey,
    refresh_encoding_key: EncodingKey,
    refresh_decoding_key: DecodingKey,
    access_expiry: Duration,
    refresh_expiry: Duration,
}

impl JwtManager {
    pub fn new(config: &JwtConfig) -> AppResult<Self> {
        let access_encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let access_decoding_key = DecodingKey::from_secret(config.secret.as_bytes());
        let refresh_encoding_key = EncodingKey::from_secret(config.refresh_secret.as_bytes());
        let refresh_decoding_key = DecodingKey::from_secret(config.refresh_secret.as_bytes());

        Ok(Self {
            access_encoding_key,
            access_decoding_key,
            refresh_encoding_key,
            refresh_decoding_key,
            access_expiry: Duration::minutes(config.expiry_minutes),
            refresh_expiry: Duration::days(config.refresh_expiry_days),
        })
    }

    pub fn generate_access_token(
        &self,
        user_id: Uuid,
        email: &str,
        username: &str,
        role: &str,
        email_verified: bool,
    ) -> AppResult<String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            email: email.to_string(),
            username: username.to_string(),
            role: role.to_string(),
            email_verified,
            iat: now.timestamp(),
            exp: (now + self.access_expiry).timestamp(),
        };

        encode(&Header::default(), &claims, &self.access_encoding_key)
            .map_err(|e| AppError::Auth(format!("Failed to generate access token: {}", e)))
    }

    pub fn generate_refresh_token(
        &self,
        user_id: Uuid,
        client_id: Uuid,
    ) -> AppResult<(String, Uuid)> {
        let now = Utc::now();
        let jti = Uuid::new_v4();
        let claims = RefreshClaims {
            sub: user_id,
            client_id,
            jti,
            iat: now.timestamp(),
            exp: (now + self.refresh_expiry).timestamp(),
        };

        let token = encode(&Header::default(), &claims, &self.refresh_encoding_key)
            .map_err(|e| AppError::Auth(format!("Failed to generate refresh token: {}", e)))?;

        Ok((token, jti))
    }

    pub fn verify_access_token(&self, token: &str) -> AppResult<Claims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.validate_nbf = false;

        decode::<Claims>(token, &self.access_decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::TokenExpired,
                _ => AppError::InvalidToken,
            })
    }

    pub fn verify_refresh_token(&self, token: &str) -> AppResult<RefreshClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.validate_nbf = false;

        decode::<RefreshClaims>(token, &self.refresh_decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::TokenExpired,
                _ => AppError::InvalidToken,
            })
    }

    pub fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }
}

pub fn hash_password(password: &str) -> AppResult<String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to hash password: {}", e)))
}

pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    bcrypt::verify(password, hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to verify password: {}", e)))
}
