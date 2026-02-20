use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    Extension,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthContext;
use crate::AppState;

#[derive(Debug, FromRow)]
pub struct Route {
    pub id: Uuid,
    pub sketch_id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub geojson: Value,
    pub metadata: Value,
    pub notes: Option<String>,
    pub version: i32,
    #[allow(dead_code)]
    pub deleted_at: Option<chrono::DateTime<Utc>>,
    #[allow(dead_code)]
    pub created_at: chrono::DateTime<Utc>,
    #[allow(dead_code)]
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RouteResponse {
    pub id: Uuid,
    pub sketch_id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub geojson: Value,
    pub metadata: Value,
    pub notes: Option<String>,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRouteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geojson: Value,
    pub metadata: Option<Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRouteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geojson: Option<Value>,
    pub metadata: Option<Value>,
    pub notes: Option<String>,
}

pub async fn list_routes(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(sketch_id): Path<Uuid>,
) -> AppResult<Json<Vec<RouteResponse>>> {
    verify_sketch_ownership(&state, sketch_id, auth.user_id).await?;

    let routes: Vec<Route> = sqlx::query_as(
        "SELECT * FROM routes WHERE sketch_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"
    )
    .bind(sketch_id)
    .fetch_all(&state.db)
    .await?;

    let responses: Vec<RouteResponse> = routes
        .into_iter()
        .map(|route| RouteResponse {
            id: route.id,
            sketch_id: route.sketch_id,
            name: route.name,
            description: route.description,
            geojson: route.geojson,
            metadata: route.metadata,
            notes: route.notes,
            version: route.version,
            created_at: route.created_at.to_rfc3339(),
            updated_at: route.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(responses))
}

pub async fn get_route(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<RouteResponse>> {
    let route: Option<Route> = sqlx::query_as(
        r#"
        SELECT r.* FROM routes r
        JOIN sketches s ON r.sketch_id = s.id
        WHERE r.id = $1 AND s.user_id = $2 AND r.deleted_at IS NULL
        "#
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    match route {
        Some(route) => Ok(Json(RouteResponse {
            id: route.id,
            sketch_id: route.sketch_id,
            name: route.name,
            description: route.description,
            geojson: route.geojson,
            metadata: route.metadata,
            notes: route.notes,
            version: route.version,
            created_at: route.created_at.to_rfc3339(),
            updated_at: route.updated_at.to_rfc3339(),
        })),
        None => Err(AppError::NotFound("Route not found".to_string())),
    }
}

pub async fn create_route(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(sketch_id): Path<Uuid>,
    Json(body): Json<CreateRouteRequest>,
) -> AppResult<(StatusCode, Json<RouteResponse>)> {
    body.validate()?;
    verify_sketch_ownership(&state, sketch_id, auth.user_id).await?;

    let route: Route = sqlx::query_as(
        r#"
        INSERT INTO routes (sketch_id, name, description, geojson, metadata, notes)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#
    )
    .bind(sketch_id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.geojson)
    .bind(body.metadata.as_ref().unwrap_or(&serde_json::json!({})))
    .bind(&body.notes)
    .fetch_one(&state.db)
    .await?;

    let response = RouteResponse {
        id: route.id,
        sketch_id: route.sketch_id,
        name: route.name,
        description: route.description,
        geojson: route.geojson,
        metadata: route.metadata,
        notes: route.notes,
        version: route.version,
        created_at: route.created_at.to_rfc3339(),
        updated_at: route.updated_at.to_rfc3339(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn update_route(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateRouteRequest>,
) -> AppResult<Json<RouteResponse>> {
    body.validate()?;

    let existing: Option<Route> = sqlx::query_as(
        r#"
        SELECT r.* FROM routes r
        JOIN sketches s ON r.sketch_id = s.id
        WHERE r.id = $1 AND s.user_id = $2 AND r.deleted_at IS NULL
        "#
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Route not found".to_string()));
    }

    let route: Route = sqlx::query_as(
        r#"
        UPDATE routes
        SET name = COALESCE($1, name),
            description = COALESCE($2, description),
            geojson = COALESCE($3, geojson),
            metadata = COALESCE($4, metadata),
            notes = COALESCE($5, notes),
            version = version + 1,
            updated_at = NOW()
        WHERE id = $6
        RETURNING *
        "#
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.geojson)
    .bind(&body.metadata)
    .bind(&body.notes)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let response = RouteResponse {
        id: route.id,
        sketch_id: route.sketch_id,
        name: route.name,
        description: route.description,
        geojson: route.geojson,
        metadata: route.metadata,
        notes: route.notes,
        version: route.version,
        created_at: route.created_at.to_rfc3339(),
        updated_at: route.updated_at.to_rfc3339(),
    };

    Ok(Json(response))
}

pub async fn delete_route(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let result = sqlx::query(
        r#"
        UPDATE routes
        SET deleted_at = NOW()
        FROM sketches
        WHERE routes.id = $1 AND routes.sketch_id = sketches.id
        AND sketches.user_id = $2 AND routes.deleted_at IS NULL
        "#
    )
    .bind(id)
    .bind(auth.user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Route not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn verify_sketch_ownership(state: &AppState, sketch_id: Uuid, user_id: Uuid) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sketches WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL)"
    )
    .bind(sketch_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    if !exists {
        return Err(AppError::NotFound("Sketch not found".to_string()));
    }

    Ok(())
}
