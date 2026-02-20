use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::Response,
};
use redis::AsyncCommands;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::AppError;
use crate::middleware::auth::AuthContext;
use crate::AppState;

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let path = request.uri().path();
    let method = request.method().as_str();
    
    let (limit_key, limit, window_secs) = match (method, path) {
        ("POST", "/api/auth/login") => {
            (format!("ratelimit:login:{}", addr.ip()), state.config.rate_limit.login_per_minute, 60)
        }
        ("POST", "/api/auth/register") => {
            (format!("ratelimit:register:{}", addr.ip()), state.config.rate_limit.register_per_hour, 3600)
        }
        ("POST", "/api/auth/forgot-password") => {
            (format!("ratelimit:forgot:{}", addr.ip()), state.config.rate_limit.forgot_password_per_hour, 3600)
        }
        ("GET", path) if path.starts_with("/api/invite/") && path.ends_with("/validate") => {
            (format!("ratelimit:invite:{}", addr.ip()), state.config.rate_limit.invite_validate_per_minute, 60)
        }
        _ => {
            if let Some(auth) = request.extensions().get::<AuthContext>() {
                (format!("ratelimit:auth:{}", auth.user_id), state.config.rate_limit.authenticated_per_minute, 60)
            } else {
                return Ok(next.run(request).await);
            }
        }
    };

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let window_start = current_time - (current_time % window_secs as u64);
    let key = format!("{}:{}", limit_key, window_start);

    let mut conn = state.redis.clone();
    let count: u32 = conn.incr(&key, 1).await?;
    
    if count == 1 {
        let _: () = conn.expire(&key, window_secs as i64).await?;
    }

    if count > limit {
        return Err(AppError::RateLimit);
    }

    Ok(next.run(request).await)
}
