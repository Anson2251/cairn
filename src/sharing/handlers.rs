use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    Extension,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthContext;
use crate::AppState;

#[derive(Debug, FromRow)]
pub struct Sketch {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
}

#[derive(Debug, Serialize)]
pub struct ShareResponse {
    pub user_id: Uuid,
    pub username: String,
    pub access_level: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateShareRequest {
    pub user_id: Uuid,
    #[validate(length(min = 1))]
    pub access_level: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateShareRequest {
    #[validate(length(min = 1))]
    pub access_level: String,
}

#[derive(Debug, Serialize)]
pub struct PublicLinkResponse {
    pub token: String,
    pub access_level: String,
    pub expires_at: Option<String>,
    pub url: String,
}

pub async fn create_share(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(sketch_id): Path<Uuid>,
    Json(body): Json<CreateShareRequest>,
) -> AppResult<Json<ShareResponse>> {
    let sketch: Option<Sketch> = sqlx::query_as(
        "SELECT * FROM sketches WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    )
    .bind(sketch_id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if sketch.is_none() {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    let target_user: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, username FROM users WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(body.user_id)
    .fetch_optional(&state.db)
    .await?;

    let target_user = target_user.ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if target_user.0 == auth.user_id {
        return Err(AppError::Validation("Cannot share with yourself".to_string()));
    }

    let access_level = if body.access_level == "edit" { "edit" } else { "view" };

    sqlx::query(
        r#"
        INSERT INTO shares (sketch_id, user_id, access_level, created_by)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (sketch_id, user_id) DO UPDATE SET access_level = EXCLUDED.access_level
        "#
    )
    .bind(sketch_id)
    .bind(body.user_id)
    .bind(access_level)
    .bind(auth.user_id)
    .execute(&state.db)
    .await?;

    Ok(Json(ShareResponse {
        user_id: target_user.0,
        username: target_user.1,
        access_level: access_level.to_string(),
        created_at: Utc::now().to_rfc3339(),
    }))
}

pub async fn list_shares(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(sketch_id): Path<Uuid>,
) -> AppResult<Json<Vec<ShareResponse>>> {
    let sketch: Option<Sketch> = sqlx::query_as(
        "SELECT * FROM sketches WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    )
    .bind(sketch_id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if sketch.is_none() {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    #[derive(Debug, FromRow)]
    struct ShareRow {
        pub user_id: Uuid,
        pub username: String,
        pub access_level: String,
        pub created_at: chrono::DateTime<Utc>,
    }

    let shares: Vec<ShareRow> = sqlx::query_as(
        r#"
        SELECT s.user_id, u.username, s.access_level, s.created_at
        FROM shares s
        JOIN users u ON u.id = s.user_id
        WHERE s.sketch_id = $1
        "#
    )
    .bind(sketch_id)
    .fetch_all(&state.db)
    .await?;

    let responses: Vec<ShareResponse> = shares
        .into_iter()
        .map(|share| ShareResponse {
            user_id: share.user_id,
            username: share.username,
            access_level: share.access_level,
            created_at: share.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(responses))
}

pub async fn update_share(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path((sketch_id, user_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateShareRequest>,
) -> AppResult<Json<ShareResponse>> {
    let sketch: Option<Sketch> = sqlx::query_as(
        "SELECT * FROM sketches WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    )
    .bind(sketch_id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if sketch.is_none() {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    let access_level = if body.access_level == "edit" { "edit" } else { "view" };

    let result = sqlx::query(
        "UPDATE shares SET access_level = $1 WHERE sketch_id = $2 AND user_id = $3"
    )
    .bind(access_level)
    .bind(sketch_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Share not found".to_string()));
    }

    let username: Option<String> = sqlx::query_scalar(
        "SELECT username FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(ShareResponse {
        user_id,
        username: username.unwrap_or_default(),
        access_level: access_level.to_string(),
        created_at: Utc::now().to_rfc3339(),
    }))
}

pub async fn delete_share(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path((sketch_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let sketch: Option<Sketch> = sqlx::query_as(
        "SELECT * FROM sketches WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    )
    .bind(sketch_id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if sketch.is_none() {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    let result = sqlx::query(
        "DELETE FROM shares WHERE sketch_id = $1 AND user_id = $2"
    )
    .bind(sketch_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Share not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct CreatePublicLinkRequest {
    pub access_level: Option<String>,
    pub expires_days: Option<i32>,
}

pub async fn create_public_link(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(sketch_id): Path<Uuid>,
    Json(body): Json<CreatePublicLinkRequest>,
) -> AppResult<Json<PublicLinkResponse>> {
    let sketch: Option<Sketch> = sqlx::query_as(
        "SELECT * FROM sketches WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    )
    .bind(sketch_id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if sketch.is_none() {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    let token = Uuid::new_v4().to_string();
    let access_level = "view";
    let expires_at = body.expires_days.map(|days| Utc::now() + chrono::Duration::days(days as i64));

    sqlx::query(
        r#"
        INSERT INTO public_links (sketch_id, token, access_level, expires_at, created_by)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (sketch_id) DO UPDATE SET token = EXCLUDED.token, expires_at = EXCLUDED.expires_at
        "#
    )
    .bind(sketch_id)
    .bind(&token)
    .bind(access_level)
    .bind(expires_at)
    .bind(auth.user_id)
    .execute(&state.db)
    .await?;

    Ok(Json(PublicLinkResponse {
        token: token.clone(),
        access_level: access_level.to_string(),
        expires_at: expires_at.map(|e| e.to_rfc3339()),
        url: format!("/api/public/{}", token),
    }))
}

pub async fn delete_public_link(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(sketch_id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let sketch: Option<Sketch> = sqlx::query_as(
        "SELECT * FROM sketches WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    )
    .bind(sketch_id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if sketch.is_none() {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    sqlx::query("DELETE FROM public_links WHERE sketch_id = $1")
        .bind(sketch_id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, FromRow)]
pub struct PublicSketch {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PublicSketchResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_username: String,
    pub created_at: String,
    pub updated_at: String,
    pub routes: Vec<PublicRouteResponse>,
}

#[derive(Debug, Serialize)]
pub struct PublicRouteResponse {
    pub id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub geojson: serde_json::Value,
    pub metadata: serde_json::Value,
    pub notes: Option<String>,
}

pub async fn get_public_sketch(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> AppResult<Json<PublicSketchResponse>> {
    #[derive(Debug, FromRow)]
    struct PublicLinkRow {
        pub sketch_id: Uuid,
        pub access_level: String,
        pub expires_at: Option<chrono::DateTime<Utc>>,
    }

    let link: Option<PublicLinkRow> = sqlx::query_as(
        "SELECT sketch_id, access_level, expires_at FROM public_links WHERE token = $1"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await?;

    let link = link.ok_or_else(|| AppError::NotFound("Public link not found".to_string()))?;

    if link.expires_at.is_some_and(|e| e < Utc::now()) {
        return Err(AppError::NotFound("Public link expired".to_string()));
    }

    let sketch: Option<PublicSketch> = sqlx::query_as(
        "SELECT * FROM sketches WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(link.sketch_id)
    .fetch_optional(&state.db)
    .await?;

    let sketch = sketch.ok_or_else(|| AppError::NotFound("Sketch not found".to_string()))?;

    let owner_username: Option<String> = sqlx::query_scalar(
        "SELECT username FROM users WHERE id = $1"
    )
    .bind(sketch.user_id)
    .fetch_optional(&state.db)
    .await?;

    #[derive(Debug, FromRow)]
    struct RouteRow {
        pub id: Uuid,
        pub name: Option<String>,
        pub description: Option<String>,
        pub geojson: serde_json::Value,
        pub metadata: serde_json::Value,
        pub notes: Option<String>,
    }

    let routes: Vec<RouteRow> = sqlx::query_as(
        "SELECT id, name, description, geojson, metadata, notes FROM routes WHERE sketch_id = $1 AND deleted_at IS NULL"
    )
    .bind(link.sketch_id)
    .fetch_all(&state.db)
    .await?;

    let route_responses: Vec<PublicRouteResponse> = routes
        .into_iter()
        .map(|r| PublicRouteResponse {
            id: r.id,
            name: r.name,
            description: r.description,
            geojson: r.geojson,
            metadata: r.metadata,
            notes: r.notes,
        })
        .collect();

    Ok(Json(PublicSketchResponse {
        id: sketch.id,
        name: sketch.name,
        description: sketch.description,
        owner_username: owner_username.unwrap_or_default(),
        created_at: sketch.created_at.to_rfc3339(),
        updated_at: sketch.updated_at.to_rfc3339(),
        routes: route_responses,
    }))
}
