use askama::Template;
use axum::{
    body::Body,
    extract::{Form, Path, State},
    http::{header::SET_COOKIE, StatusCode},
    response::{Html, IntoResponse, Response},
    Extension, Json,
};
use cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::jwt::{verify_password, JwtManager};
use crate::auth::templates::LoggedOutPage;
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
            .secure(state.config.server.secure_cookies)
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
        .secure(state.config.server.secure_cookies)
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

pub async fn new_invite_form() -> AppResult<Html<String>> {
    let html = super::templates::InviteCreateForm {
        error: "",
    }
    .render()?;
    Ok(Html(html))
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

    let invites: Vec<super::templates::InviteRow> = sqlx::query_as(
        "SELECT id, sequence, code, cairn_name, used, used_by, used_at, expires_at, created_at FROM invite_codes ORDER BY sequence DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, i32, String, String, bool, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)| super::templates::InviteRow {
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

    let html = super::templates::InviteListRows { invites: &invites }.render()?;
    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct InviteEditForm {
    expires_days: Option<i32>,
}

pub async fn edit_invite_form(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let invite: Option<(Uuid, i32, String, String, bool, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, sequence, code, cairn_name, used, expires_at, created_at FROM invite_codes WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    match invite {
        Some(inv) => {
            let invite_form = super::templates::InviteFormData {
                id: inv.0,
                sequence: inv.1,
                code: inv.2,
                cairn_name: inv.3,
                used: inv.4,
                expires_at: inv.5.map(|d| d.to_rfc3339()).unwrap_or_default(),
                created_at: inv.6.to_rfc3339(),
            };
            let html = super::templates::InviteForm {
                invite: &invite_form,
                error: "",
            }
            .render()?;
            Ok(Html(html))
        }
        None => {
            let html = "Invite not found".to_string();
            Ok(Html(html))
        }
    }
}

pub async fn update_invite_form(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Form(form): Form<InviteEditForm>,
) -> AppResult<Html<String>> {
    let expires_at = form.expires_days.map(|days| chrono::Utc::now() + chrono::Duration::days(days as i64));

    sqlx::query(
        "UPDATE invite_codes SET expires_at = COALESCE($2, expires_at) WHERE id = $1"
    )
    .bind(id)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    let invites: Vec<super::templates::InviteRow> = sqlx::query_as(
        "SELECT id, sequence, code, cairn_name, used, used_by, used_at, expires_at, created_at FROM invite_codes ORDER BY sequence DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, i32, String, String, bool, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)| super::templates::InviteRow {
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

    let html = super::templates::InviteListRows { invites: &invites }.render()?;
    Ok(Html(html))
}

pub async fn delete_invite_form(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    sqlx::query(
        "DELETE FROM invite_codes WHERE id = $1 AND used = FALSE"
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    let invites: Vec<super::templates::InviteRow> = sqlx::query_as(
        "SELECT id, sequence, code, cairn_name, used, used_by, used_at, expires_at, created_at FROM invite_codes ORDER BY sequence DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, i32, String, String, bool, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)| super::templates::InviteRow {
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

    let html = super::templates::InviteListRows { invites: &invites }.render()?;
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

pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> AppResult<Response<Body>> {
    let cookie_header = headers
        .get("cookie")
        .and_then(|h| h.to_str().ok());

    if let Some(cookie_header) = cookie_header {
        if let Some(refresh_token) = cookie_header
            .split(';')
            .find_map(|c| {
                let c = c.trim();
                c.strip_prefix("refresh_token=")
            })
        {
            let token_hash = JwtManager::hash_token(refresh_token);
            sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = $1")
                .bind(&token_hash)
                .execute(&state.db)
                .await?;
        }
    }

    let cookie = Cookie::build(("refresh_token", ""))
        .http_only(true)
        .secure(state.config.server.secure_cookies)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(cookie::time::Duration::seconds(0))
        .build();

    let html = LoggedOutPage.render()?;
    
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(SET_COOKIE, cookie.to_string())
        .header("content-type", "text/html")
        .body(Body::from(html))
        .unwrap();

    Ok(response)
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub role: String,
    pub email_verified: bool,
    pub trailblazer_seq: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub username: String,
    pub password: Option<String>,
    pub role: Option<String>,
    pub email_verified: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
    pub email_verified: Option<bool>,
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<UserResponse>>> {
    let users: Vec<(Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, email, username, role, email_verified, trailblazer_seq, created_at FROM users WHERE deleted_at IS NULL ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await?;

    let responses: Vec<UserResponse> = users
        .into_iter()
        .map(|row| UserResponse {
            id: row.0,
            email: row.1,
            username: row.2,
            role: row.3,
            email_verified: row.4,
            trailblazer_seq: row.5,
            created_at: row.6.to_rfc3339(),
        })
        .collect();

    Ok(Json(responses))
}

pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserResponse>> {
    let user: (Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT id, email, username, role, email_verified, trailblazer_seq, created_at FROM users WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(UserResponse {
        id: user.0,
        email: user.1,
        username: user.2,
        role: user.3,
        email_verified: user.4,
        trailblazer_seq: user.5,
        created_at: user.6.to_rfc3339(),
    }))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateUserRequest>,
) -> AppResult<Json<UserResponse>> {
    let hashed_password = if let Some(ref password) = body.password {
        Some(crate::auth::jwt::hash_password(password)?)
    } else {
        None
    };

    let role = body.role.unwrap_or_else(|| "user".to_string());

    let user: (Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "INSERT INTO users (email, username, hashed_password, role, email_verified) VALUES ($1, $2, $3, $4, $5) RETURNING id, email, username, role, email_verified, trailblazer_seq, created_at"
    )
    .bind(&body.email)
    .bind(&body.username)
    .bind(&hashed_password)
    .bind(&role)
    .bind(body.email_verified.unwrap_or(false))
    .fetch_one(&state.db)
    .await?;

    Ok(Json(UserResponse {
        id: user.0,
        email: user.1,
        username: user.2,
        role: user.3,
        email_verified: user.4,
        trailblazer_seq: user.5,
        created_at: user.6.to_rfc3339(),
    }))
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateUserRequest>,
) -> AppResult<Json<UserResponse>> {
    let hashed_password = if let Some(ref password) = body.password {
        Some(crate::auth::jwt::hash_password(password)?)
    } else {
        None
    };

    let user: (Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        r#"
        UPDATE users SET 
            email = COALESCE($2, email),
            username = COALESCE($3, username),
            hashed_password = COALESCE($4, hashed_password),
            role = COALESCE($5, role),
            email_verified = COALESCE($6, email_verified),
            updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING id, email, username, role, email_verified, trailblazer_seq, created_at
        "#
    )
    .bind(id)
    .bind(&body.email)
    .bind(&body.username)
    .bind(&hashed_password)
    .bind(&body.role)
    .bind(body.email_verified)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(UserResponse {
        id: user.0,
        email: user.1,
        username: user.2,
        role: user.3,
        email_verified: user.4,
        trailblazer_seq: user.5,
        created_at: user.6.to_rfc3339(),
    }))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let result = sqlx::query(
        "UPDATE users SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(crate::error::AppError::NotFound("User not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn new_user_form() -> AppResult<Html<String>> {
    let html = super::templates::UserForm {
        user: &super::templates::UserFormData::empty(),
        is_new: true,
        error: "",
    }
    .render()?;
    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct UserForm {
    email: String,
    username: String,
    password: String,
    role: String,
    email_verified: bool,
}

pub async fn create_user_form(
    State(state): State<Arc<AppState>>,
    Form(form): Form<UserForm>,
) -> AppResult<Response> {
    let hashed_password = if !form.password.is_empty() {
        Some(crate::auth::jwt::hash_password(&form.password)?)
    } else {
        None
    };

    let result = sqlx::query(
        "INSERT INTO users (email, username, hashed_password, role, email_verified) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(&form.email)
    .bind(&form.username)
    .bind(&hashed_password)
    .bind(&form.role)
    .bind(form.email_verified)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let users: Vec<super::templates::UserRow> = sqlx::query_as(
                "SELECT id, email, username, role, email_verified, trailblazer_seq, created_at FROM users WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT 50"
            )
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .map(|row: (Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>)| super::templates::UserRow {
                id: row.0,
                email: row.1,
                username: row.2,
                role: row.3,
                email_verified: row.4,
                trailblazer_seq: row.5.map(|s| format!("#{}", s)).unwrap_or_default(),
                created_at: row.6.to_rfc3339(),
            })
            .collect();

            let html = super::templates::UserListRows { users: &users }.render()?;
            let response = Response::builder()
                .header("HX-Trigger", "userCreated")
                .header("Content-Type", "text/html")
                .body(Body::from(html))
                .unwrap();
            Ok(response)
        }
        Err(e) => {
            if e.as_database_error().and_then(|e| e.code()).map(|c| c.as_ref() == "23505").unwrap_or(false) {
                let html = super::templates::UserForm {
                    user: &super::templates::UserFormData::empty(),
                    is_new: true,
                    error: "Email or username already exists",
                }
                .render()?;
                return Ok((StatusCode::CONFLICT, Html(html)).into_response());
            }
            let html = super::templates::UserForm {
                user: &super::templates::UserFormData::empty(),
                is_new: true,
                error: "Failed to create user",
            }
            .render()?;
            Ok((StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response())
        }
    }
}

pub async fn edit_user_form(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    let user: Option<(Uuid, String, String, String, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, email, username, role, email_verified, created_at FROM users WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    match user {
        Some(u) => {
            let user_row = super::templates::UserFormData {
                id: u.0,
                email: u.1,
                username: u.2,
                role: u.3,
                email_verified: u.4,
                created_at: u.5.to_rfc3339(),
            };
            let html = super::templates::UserForm {
                user: &user_row,
                is_new: false,
                error: "",
            }
            .render()?;
            Ok(Html(html).into_response())
        }
        None => Ok(axum::response::Redirect::to("/admin/users").into_response()),
    }
}

pub async fn update_user_form(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Form(form): Form<UserForm>,
) -> AppResult<Response> {
    let hashed_password = if !form.password.is_empty() {
        Some(crate::auth::jwt::hash_password(&form.password)?)
    } else {
        None
    };

    let result = sqlx::query(
        r#"
        UPDATE users SET 
            email = $2,
            username = $3,
            hashed_password = COALESCE($4, hashed_password),
            role = $5,
            email_verified = $6,
            updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#
    )
    .bind(id)
    .bind(&form.email)
    .bind(&form.username)
    .bind(&hashed_password)
    .bind(&form.role)
    .bind(form.email_verified)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            let users: Vec<super::templates::UserRow> = sqlx::query_as(
                "SELECT id, email, username, role, email_verified, trailblazer_seq, created_at FROM users WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT 50"
            )
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .map(|row: (Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>)| super::templates::UserRow {
                id: row.0,
                email: row.1,
                username: row.2,
                role: row.3,
                email_verified: row.4,
                trailblazer_seq: row.5.map(|s| format!("#{}", s)).unwrap_or_default(),
                created_at: row.6.to_rfc3339(),
            })
            .collect();

            let html = super::templates::UserListRows { users: &users }.render()?;
            let response = Response::builder()
                .header("HX-Trigger", "userUpdated")
                .header("Content-Type", "text/html")
                .body(Body::from(html))
                .unwrap();
            Ok(response)
        }
        _ => {
            let user: Option<(Uuid, String, String, String, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
                "SELECT id, email, username, role, email_verified, created_at FROM users WHERE id = $1 AND deleted_at IS NULL"
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await?;

            let user_row = user.map(|u| super::templates::UserFormData {
                id: u.0,
                email: u.1,
                username: u.2,
                role: u.3,
                email_verified: u.4,
                created_at: u.5.to_rfc3339(),
            }).unwrap_or(super::templates::UserFormData::empty());

            let html = super::templates::UserForm {
                user: &user_row,
                is_new: false,
                error: "Failed to update user",
            }
            .render()?;
            Ok((StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response())
        }
    }
}

pub async fn delete_user_form(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    sqlx::query(
        "UPDATE users SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    let users: Vec<super::templates::UserRow> = sqlx::query_as(
        "SELECT id, email, username, role, email_verified, trailblazer_seq, created_at FROM users WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row: (Uuid, String, String, String, bool, Option<i32>, chrono::DateTime<chrono::Utc>)| super::templates::UserRow {
        id: row.0,
        email: row.1,
        username: row.2,
        role: row.3,
        email_verified: row.4,
        trailblazer_seq: row.5.map(|s| format!("#{}", s)).unwrap_or_default(),
        created_at: row.6.to_rfc3339(),
    })
    .collect();

    let html = super::templates::UserListRows { users: &users }.render()?;
    let response = Response::builder()
        .header("HX-Trigger", "userDeleted")
        .header("Content-Type", "text/html")
        .body(Body::from(html))
        .unwrap();
    Ok(response)
}
