use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::auth::jwt::Claims;
use crate::error::AppError;
use crate::AppState;

#[derive(Clone)]
pub struct AuthContext {
    pub user_id: uuid::Uuid,
    #[allow(dead_code)]
    pub email: String,
    #[allow(dead_code)]
    pub username: String,
    pub role: String,
    #[allow(dead_code)]
    pub email_verified: bool,
}

impl From<Claims> for AuthContext {
    fn from(claims: Claims) -> Self {
        Self {
            user_id: claims.sub,
            email: claims.email,
            username: claims.username,
            role: claims.role,
            email_verified: claims.email_verified,
        }
    }
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(AppError::Auth("Missing authorization header".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Auth("Invalid authorization header format".to_string()))?;

    let claims = state.jwt.verify_access_token(token)?;
    let auth_context = AuthContext::from(claims);

    request.extensions_mut().insert(auth_context);
    
    Ok(next.run(request).await)
}

#[allow(dead_code)]
pub async fn optional_auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(auth_header) = request.headers().get("authorization").and_then(|h| h.to_str().ok()) {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            if let Ok(claims) = state.jwt.verify_access_token(token) {
                let auth_context = AuthContext::from(claims);
                request.extensions_mut().insert(auth_context);
            }
        }
    }

    next.run(request).await
}
