use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::error::AppError;
use crate::middleware::auth::AuthContext;
use crate::AppState;

pub async fn admin_middleware(
    State(_state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_context = request
        .extensions()
        .get::<AuthContext>()
        .ok_or(AppError::Unauthorized("Authentication required".to_string()))?;

    if auth_context.role != "admin" {
        return Err(AppError::Unauthorized("Admin access required".to_string()));
    }

    Ok(next.run(request).await)
}
