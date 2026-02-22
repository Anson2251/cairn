use askama::Template;
use axum::{
    extract::{Form, State},
    http::{header::SET_COOKIE, StatusCode},
    response::{Html, IntoResponse, Response},
    Extension,
};
use cookie::{Cookie, SameSite};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::jwt::{verify_password, JwtManager};
use crate::error::AppResult;
use crate::middleware::auth::AuthContext;
use crate::AppState;

use super::templates::{AdminPage, InviteRow, InviteList, UserRow, UserList, StatsData, StatsStats, LoginPage};

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
}

pub async fn login_page() -> AppResult<Html<String>> {
    let html = LoginPage { error: "" }.render()?;
    Ok(Html(html))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Form(form): Form<LoginForm>,
) -> AppResult<Response> {
    let is_default_admin = form.email == "admin@example.com" && form.password == "12345678";
    
    if is_default_admin {
        let user_exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM users WHERE email = 'admin@example.com' AND deleted_at IS NULL"
        )
        .fetch_optional(&state.db)
        .await?;

        let user_id = if let Some(uid) = user_exists {
            uid
        } else {
            let hashed_password = crate::auth::jwt::hash_password("12345678")?;
            let new_user: (Uuid,) = sqlx::query_as(
                "INSERT INTO users (email, username, hashed_password, role, email_verified) VALUES ($1, $2, $3, $4, $5) RETURNING id"
            )
            .bind("admin@example.com")
            .bind("admin")
            .bind(&hashed_password)
            .bind("admin")
            .bind(true)
            .fetch_one(&state.db)
            .await?;
            new_user.0
        };

        let (refresh_token, _) = state.jwt.generate_refresh_token(user_id, uuid::Uuid::new_v4())?;
        let refresh_hash = JwtManager::hash_token(&refresh_token);

        let refresh_expiry = chrono::Utc::now() + chrono::Duration::days(state.config.jwt.refresh_expiry_days);
        sqlx::query(
            "INSERT INTO refresh_tokens (user_id, token_hash, client_id, expires_at) VALUES ($1, $2, $3, $4)"
        )
        .bind(user_id)
        .bind(&refresh_hash)
        .bind(uuid::Uuid::new_v4())
        .bind(refresh_expiry)
        .execute(&state.db)
        .await?;

        let cookie = Cookie::build(("refresh_token", refresh_token))
            .http_only(true)
            .secure(true)
            .same_site(SameSite::Strict)
            .path("/")
            .max_age(cookie::time::Duration::days(state.config.jwt.refresh_expiry_days))
            .build();

        let mut response = axum::response::Redirect::to("/admin/dashboard").into_response();
        response.headers_mut().insert(SET_COOKIE, cookie.to_string().parse().unwrap());
        return Ok(response);
    }

    let user: Option<(Uuid, String, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT id, email, username, hashed_password, email_verified FROM users WHERE email = $1 AND deleted_at IS NULL"
    )
    .bind(&form.email)
    .fetch_optional(&state.db)
    .await?;

    let user = match user {
        Some(u) => u,
        None => {
            let html = LoginPage { error: "Invalid credentials" }.render()?;
            return Ok((StatusCode::UNAUTHORIZED, Html(html)).into_response());
        }
    };

    let password_hash = match user.3.as_ref() {
        Some(h) => h,
        None => {
            let html = LoginPage { error: "Invalid credentials" }.render()?;
            return Ok((StatusCode::UNAUTHORIZED, Html(html)).into_response());
        }
    };

    if !verify_password(&form.password, password_hash)? {
        let html = LoginPage { error: "Invalid credentials" }.render()?;
        return Ok((StatusCode::UNAUTHORIZED, Html(html)).into_response());
    }

    let (refresh_token, _) = state.jwt.generate_refresh_token(user.0, uuid::Uuid::new_v4())?;
    let refresh_hash = JwtManager::hash_token(&refresh_token);

    let refresh_expiry = chrono::Utc::now() + chrono::Duration::days(state.config.jwt.refresh_expiry_days);
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, client_id, expires_at) VALUES ($1, $2, $3, $4)"
    )
    .bind(user.0)
    .bind(&refresh_hash)
    .bind(uuid::Uuid::new_v4())
    .bind(refresh_expiry)
    .execute(&state.db)
    .await?;

    let cookie = Cookie::build(("refresh_token", refresh_token))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(cookie::time::Duration::days(state.config.jwt.refresh_expiry_days))
        .build();

    let mut response = axum::response::Redirect::to("/admin/dashboard").into_response();
    response.headers_mut().insert(SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

pub async fn admin_index(
    Extension(auth): Extension<Option<AuthContext>>,
) -> AppResult<Response> {
    if auth.is_some() {
        Ok(axum::response::Redirect::to("/admin/dashboard").into_response())
    } else {
        Ok(axum::response::Redirect::to("/admin/login").into_response())
    }
}

pub async fn admin_page(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> AppResult<Html<String>> {
    let invites: Vec<InviteRow> = sqlx::query_as(
        "SELECT id, sequence, code, cairn_name, used, used_by, used_at, expires_at, created_at FROM invite_codes ORDER BY sequence DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, i32, String, String, bool, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)| InviteRow {
        id: row.0,
        sequence: row.1,
        code: row.2,
        cairn_name: row.3,
        used: row.4,
        used_by: row.5,
        used_at: row.6.map(|d| d.to_rfc3339()).unwrap_or_default(),
        expires_at: row.7.map(|d| d.to_rfc3339()).unwrap_or_default(),
        created_at: row.8.to_rfc3339(),
    })
    .collect();

    let users: Vec<UserRow> = sqlx::query_as(
        "SELECT id, email, username, role, email_verified, trailblazer_seq, created_at FROM users WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>)| UserRow {
        id: row.0,
        email: row.1,
        username: row.2,
        role: row.3,
        email_verified: row.4,
        trailblazer_seq: row.5.map(|s| format!("#{}", s)).unwrap_or_default(),
        created_at: row.6.to_rfc3339(),
    })
    .collect();

    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL")
        .fetch_one(&state.db)
        .await?;
    let total_invites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invite_codes")
        .fetch_one(&state.db)
        .await?;
    let used_invites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invite_codes WHERE used = TRUE")
        .fetch_one(&state.db)
        .await?;
    let total_sketches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sketches WHERE deleted_at IS NULL")
        .fetch_one(&state.db)
        .await?;

    let stats = StatsData {
        total_users,
        total_invites,
        used_invites,
        total_sketches,
    };

    let is_default_admin = auth.email == "admin@example.com";

    let html = AdminPage {
        username: &auth.username,
        email: &auth.email,
        is_default_admin,
        invites: &invites,
        users: &users,
        stats: &stats,
    }
    .render()?;

    Ok(Html(html))
}

pub async fn invites(
    State(state): State<Arc<AppState>>,
) -> AppResult<Html<String>> {
    let invites: Vec<InviteRow> = sqlx::query_as(
        "SELECT id, sequence, code, cairn_name, used, used_by, used_at, expires_at, created_at FROM invite_codes ORDER BY sequence DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, i32, String, String, bool, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)| InviteRow {
        id: row.0,
        sequence: row.1,
        code: row.2,
        cairn_name: row.3,
        used: row.4,
        used_by: row.5,
        used_at: row.6.map(|d| d.to_rfc3339()).unwrap_or_default(),
        expires_at: row.7.map(|d| d.to_rfc3339()).unwrap_or_default(),
        created_at: row.8.to_rfc3339(),
    })
    .collect();

    let html = InviteList { invites: &invites }.render()?;
    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteForm {
    count: Option<i32>,
    expires_days: Option<i32>,
}

pub async fn create_invites(
    State(state): State<Arc<AppState>>,
    Form(form): Form<CreateInviteForm>,
) -> AppResult<Html<String>> {
    let count = form.count.unwrap_or(1).clamp(1, 100);
    let expires_at = form.expires_days.map(|days| chrono::Utc::now() + chrono::Duration::days(days as i64));

    let max_sequence: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(sequence) FROM invite_codes"
    )
    .fetch_one(&state.db)
    .await?;

    let start_sequence = max_sequence.unwrap_or(0) + 1;

    for i in 0..count {
        let sequence = start_sequence + i;
        let code_data = crate::invite::generator::generate_invite_code(sequence, &state.config.invite.salt);

        let point = sqlx::postgres::types::PgPoint {
            x: code_data.origin_coord.0,
            y: code_data.origin_coord.1,
        };

        sqlx::query(
            r#"
            INSERT INTO invite_codes (sequence, code, cairn_name, origin_coord, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(sequence)
        .bind(&code_data.code)
        .bind(&code_data.cairn_name)
        .bind(point)
        .bind(expires_at)
        .execute(&state.db)
        .await?;
    }

    let invites: Vec<InviteRow> = sqlx::query_as(
        "SELECT id, sequence, code, cairn_name, used, used_by, used_at, expires_at, created_at FROM invite_codes ORDER BY sequence DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, i32, String, String, bool, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)| InviteRow {
        id: row.0,
        sequence: row.1,
        code: row.2,
        cairn_name: row.3,
        used: row.4,
        used_by: row.5,
        used_at: row.6.map(|d| d.to_rfc3339()).unwrap_or_default(),
        expires_at: row.7.map(|d| d.to_rfc3339()).unwrap_or_default(),
        created_at: row.8.to_rfc3339(),
    })
    .collect();

    let html = InviteList { invites: &invites }.render()?;
    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct RevokeInviteForm {
    id: Uuid,
}

pub async fn revoke_invite(
    State(state): State<Arc<AppState>>,
    Form(form): Form<RevokeInviteForm>,
) -> AppResult<Html<String>> {
    sqlx::query(
        "DELETE FROM invite_codes WHERE id = $1 AND used = FALSE"
    )
    .bind(form.id)
    .execute(&state.db)
    .await?;

    let invites: Vec<InviteRow> = sqlx::query_as(
        "SELECT id, sequence, code, cairn_name, used, used_by, used_at, expires_at, created_at FROM invite_codes ORDER BY sequence DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, i32, String, String, bool, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)| InviteRow {
        id: row.0,
        sequence: row.1,
        code: row.2,
        cairn_name: row.3,
        used: row.4,
        used_by: row.5,
        used_at: row.6.map(|d| d.to_rfc3339()).unwrap_or_default(),
        expires_at: row.7.map(|d| d.to_rfc3339()).unwrap_or_default(),
        created_at: row.8.to_rfc3339(),
    })
    .collect();

    let html = InviteList { invites: &invites }.render()?;
    Ok(Html(html))
}

pub async fn users_list(
    State(state): State<Arc<AppState>>,
) -> AppResult<Html<String>> {
    let users: Vec<UserRow> = sqlx::query_as(
        "SELECT id, email, username, role, email_verified, trailblazer_seq, created_at FROM users WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>)| UserRow {
        id: row.0,
        email: row.1,
        username: row.2,
        role: row.3,
        email_verified: row.4,
        trailblazer_seq: row.5.map(|s| format!("#{}", s)).unwrap_or_default(),
        created_at: row.6.to_rfc3339(),
    })
    .collect();

    let html = UserList { users: &users }.render()?;
    Ok(Html(html))
}

pub async fn stats(
    State(state): State<Arc<AppState>>,
) -> AppResult<Html<String>> {
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL")
        .fetch_one(&state.db)
        .await?;
    let total_invites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invite_codes")
        .fetch_one(&state.db)
        .await?;
    let used_invites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invite_codes WHERE used = TRUE")
        .fetch_one(&state.db)
        .await?;
    let total_sketches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sketches WHERE deleted_at IS NULL")
        .fetch_one(&state.db)
        .await?;

    let stats = StatsData {
        total_users,
        total_invites,
        used_invites,
        total_sketches,
    };

    let html = StatsStats { stats: &stats }.render()?;
    Ok(Html(html))
}
