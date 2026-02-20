use axum::{
    extract::{Json, Path, Query, State},
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
#[sqlx(rename_all = "snake_case")]
pub struct Sketch {
    pub id: Uuid,
    #[allow(dead_code)]
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    #[allow(dead_code)]
    pub deleted_at: Option<chrono::DateTime<Utc>>,
    #[allow(dead_code)]
    pub created_at: chrono::DateTime<Utc>,
    #[allow(dead_code)]
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct SketchWithCount {
    pub id: Uuid,
    #[allow(dead_code)]
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    #[allow(dead_code)]
    pub deleted_at: Option<chrono::DateTime<Utc>>,
    #[allow(dead_code)]
    pub created_at: chrono::DateTime<Utc>,
    #[allow(dead_code)]
    pub updated_at: chrono::DateTime<Utc>,
    pub route_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SketchResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub created_at: String,
    pub updated_at: String,
    pub route_count: i64,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSketchRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSketchRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ListSketchesQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_sketches(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<ListSketchesQuery>,
) -> AppResult<Json<Vec<SketchResponse>>> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let sketches: Vec<SketchWithCount> = sqlx::query_as(
        r#"
        SELECT s.*, COUNT(r.id) as route_count
        FROM sketches s
        LEFT JOIN routes r ON r.sketch_id = s.id AND r.deleted_at IS NULL
        WHERE s.user_id = $1 AND s.deleted_at IS NULL
        GROUP BY s.id
        ORDER BY s.updated_at DESC
        LIMIT $2 OFFSET $3
        "#
    )
    .bind(auth.user_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let responses: Vec<SketchResponse> = sketches
        .into_iter()
        .map(|sketch| SketchResponse {
            id: sketch.id,
            name: sketch.name,
            description: sketch.description,
            is_public: sketch.is_public,
            created_at: sketch.created_at.to_rfc3339(),
            updated_at: sketch.updated_at.to_rfc3339(),
            route_count: sketch.route_count,
        })
        .collect();

    Ok(Json(responses))
}

pub async fn get_sketch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SketchResponse>> {
    let sketch: Option<SketchWithCount> = sqlx::query_as(
        r#"
        SELECT s.*, COUNT(r.id) as route_count
        FROM sketches s
        LEFT JOIN routes r ON r.sketch_id = s.id AND r.deleted_at IS NULL
        WHERE s.id = $1 AND s.user_id = $2 AND s.deleted_at IS NULL
        GROUP BY s.id
        "#
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    match sketch {
        Some(sketch) => Ok(Json(SketchResponse {
            id: sketch.id,
            name: sketch.name,
            description: sketch.description,
            is_public: sketch.is_public,
            created_at: sketch.created_at.to_rfc3339(),
            updated_at: sketch.updated_at.to_rfc3339(),
            route_count: sketch.route_count,
        })),
        None => Err(AppError::NotFound("Sketch not found".to_string())),
    }
}

pub async fn create_sketch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateSketchRequest>,
) -> AppResult<(StatusCode, Json<SketchResponse>)> {
    body.validate()?;

    let sketch: Sketch = sqlx::query_as(
        r#"
        INSERT INTO sketches (user_id, name, description)
        VALUES ($1, $2, $3)
        RETURNING *
        "#
    )
    .bind(auth.user_id)
    .bind(&body.name)
    .bind(&body.description)
    .fetch_one(&state.db)
    .await?;

    let response = SketchResponse {
        id: sketch.id,
        name: sketch.name,
        description: sketch.description,
        is_public: sketch.is_public,
        created_at: sketch.created_at.to_rfc3339(),
        updated_at: sketch.updated_at.to_rfc3339(),
        route_count: 0,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn update_sketch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSketchRequest>,
) -> AppResult<Json<SketchResponse>> {
    body.validate()?;

    let sketch: Option<Sketch> = sqlx::query_as(
        "SELECT * FROM sketches WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if sketch.is_none() {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    let sketch: Sketch = sqlx::query_as(
        r#"
        UPDATE sketches
        SET name = COALESCE($1, name),
            description = COALESCE($2, description),
            is_public = COALESCE($3, is_public),
            updated_at = NOW()
        WHERE id = $4 AND user_id = $5
        RETURNING *
        "#
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.is_public)
    .bind(id)
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await?;

    let route_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM routes WHERE sketch_id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let response = SketchResponse {
        id: sketch.id,
        name: sketch.name,
        description: sketch.description,
        is_public: sketch.is_public,
        created_at: sketch.created_at.to_rfc3339(),
        updated_at: sketch.updated_at.to_rfc3339(),
        route_count,
    };

    Ok(Json(response))
}

pub async fn delete_sketch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let result = sqlx::query(
        "UPDATE sketches SET deleted_at = NOW() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    )
    .bind(id)
    .bind(auth.user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}
