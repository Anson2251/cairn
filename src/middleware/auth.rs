use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
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

pub async fn auth_middleware_with_cookie(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let cookie_header = request
        .headers()
        .get("cookie")
        .and_then(|h| h.to_str().ok());

    let token = cookie_header
        .and_then(|c| c.split(';').find(|kv| kv.trim().starts_with("refresh_token=")))
        .and_then(|t| t.strip_prefix("refresh_token="));

    if token.is_none() {
        return Err(AppError::Auth("No refresh token".to_string()));
    }

    let refresh_token = token.unwrap();
    let claims = state.jwt.verify_refresh_token(refresh_token)?;
    
    let user: crate::auth::types::User = sqlx::query_as(
        "SELECT * FROM users WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Auth("User not found".to_string()))?;

    let auth_context = AuthContext {
        user_id: user.id,
        email: user.email,
        username: user.username,
        role: user.role,
        email_verified: user.email_verified,
    };

    request.extensions_mut().insert(auth_context);
    
    Ok(next.run(request).await)
}

pub async fn auth_middleware_with_cookie_redirect(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let cookie_header = request
        .headers()
        .get("cookie")
        .and_then(|h| h.to_str().ok());

    let token = cookie_header
        .and_then(|c| c.split(';').find(|kv| kv.trim().starts_with("refresh_token=")))
        .and_then(|t| t.strip_prefix("refresh_token="));

    if token.is_none() {
        return Ok(axum::response::Redirect::to("/admin/login").into_response());
    }

    let refresh_token = token.unwrap();
    let claims = match state.jwt.verify_refresh_token(refresh_token) {
        Ok(c) => c,
        Err(_) => return Ok(axum::response::Redirect::to("/admin/login").into_response()),
    };

    let user: crate::auth::types::User = match sqlx::query_as(
        "SELECT * FROM users WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await? {
        Some(u) => u,
        None => return Ok(axum::response::Redirect::to("/admin/login").into_response()),
    };

    let auth_context = AuthContext {
        user_id: user.id,
        email: user.email,
        username: user.username,
        role: user.role,
        email_verified: user.email_verified,
    };

    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

pub async fn optional_auth_middleware_with_cookie(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let cookie_header = request
        .headers()
        .get("cookie")
        .and_then(|h| h.to_str().ok());

    let token = cookie_header
        .and_then(|c| c.split(';').find(|kv| kv.trim().starts_with("refresh_token=" )))
        .and_then(|t| t.strip_prefix("refresh_token="));

    let auth_opt: Option<AuthContext> = if let Some(refresh_token) = token {
        if let Ok(claims) = state.jwt.verify_refresh_token(refresh_token) {
            if let Ok(Some(user)) = sqlx::query_as::<_, crate::auth::types::User>(
                "SELECT * FROM users WHERE id = $1 AND deleted_at IS NULL"
            )
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await
            {
                Some(AuthContext {
                    user_id: user.id,
                    email: user.email,
                    username: user.username,
                    role: user.role,
                    email_verified: user.email_verified,
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    request.extensions_mut().insert(auth_opt);

    Ok(next.run(request).await)
}

#[allow(dead_code)]
pub async fn optional_auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(auth_header) = request.headers().get("authorization").and_then(|h| h.to_str().ok())
        && let Some(token) = auth_header.strip_prefix("Bearer ")
        && let Ok(claims) = state.jwt.verify_access_token(token)
    {
        let auth_context = AuthContext::from(claims);
        request.extensions_mut().insert(auth_context);
    }

    next.run(request).await
}
