use axum::{
    extract::{Json, Path, State},
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthContext;
use crate::AppState;

type UpdatedRouteRow = (Uuid, i32, Option<String>, Option<String>, Value, Value, Option<String>);

#[derive(Debug, Deserialize)]
pub struct PushRequest {
    pub client_id: Uuid,
    pub changes: Vec<RouteChange>,
}

#[derive(Debug, Deserialize)]
pub struct RouteChange {
    pub route_id: Uuid,
    pub base_version: i32,
    pub data: RouteData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RouteData {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geojson: Value,
    pub metadata: Value,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PushResponse {
    pub accepted: Vec<Uuid>,
    pub conflicts: Vec<ConflictInfo>,
}

#[derive(Debug, Serialize)]
pub struct ConflictInfo {
    pub route_id: Uuid,
    pub local_version: i32,
    pub server_version: i32,
    pub server_data: ServerRouteData,
}

#[derive(Debug, Serialize)]
pub struct ServerRouteData {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geojson: Value,
    pub metadata: Value,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub client_id: Uuid,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub known_versions: HashMap<Uuid, i32>,
}

#[derive(Debug, Serialize)]
pub struct PullResponse {
    pub updated: Vec<UpdatedRoute>,
    pub deleted: Vec<Uuid>,
    pub server_time: String,
}

#[derive(Debug, Serialize)]
pub struct UpdatedRoute {
    pub route_id: Uuid,
    pub version: i32,
    pub data: RouteData,
}

#[derive(Debug, Deserialize)]
pub struct ResolveRequest {
    pub resolution: String,
    pub base_version: i32,
    pub data: RouteData,
}

pub async fn push(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<PushRequest>,
) -> AppResult<Json<PushResponse>> {
    let mut accepted = Vec::new();
    let mut conflicts = Vec::new();

    for change in body.changes {
        let route: Option<(Uuid, i32)> = sqlx::query_as(
            r#"
            SELECT r.id, r.version
            FROM routes r
            JOIN sketches s ON r.sketch_id = s.id
            WHERE r.id = $1 AND s.user_id = $2 AND r.deleted_at IS NULL
            "#
        )
        .bind(change.route_id)
        .bind(auth.user_id)
        .fetch_optional(&state.db)
        .await?;

        match route {
            Some((route_id, server_version)) => {
                if server_version == change.base_version {
                    sqlx::query(
                        r#"
                        UPDATE routes
                        SET name = $1,
                            description = $2,
                            geojson = $3,
                            metadata = $4,
                            notes = $5,
                            version = version + 1,
                            updated_at = NOW()
                        WHERE id = $6
                        "#
                    )
                    .bind(&change.data.name)
                    .bind(&change.data.description)
                    .bind(&change.data.geojson)
                    .bind(&change.data.metadata)
                    .bind(&change.data.notes)
                    .bind(route_id)
                    .execute(&state.db)
                    .await?;

                    sqlx::query(
                        "INSERT INTO sync_log (route_id, user_id, action, version_before, version_after, client_id) VALUES ($1, $2, 'push', $3, $4, $5)"
                    )
                    .bind(route_id)
                    .bind(auth.user_id)
                    .bind(change.base_version)
                    .bind(server_version + 1)
                    .bind(body.client_id)
                    .execute(&state.db)
                    .await?;

                    accepted.push(route_id);
                } else {
                    let server_data: (Option<String>, Option<String>, Value, Value, Option<String>) = sqlx::query_as(
                        "SELECT name, description, geojson, metadata, notes FROM routes WHERE id = $1"
                    )
                    .bind(route_id)
                    .fetch_one(&state.db)
                    .await?;

                    conflicts.push(ConflictInfo {
                        route_id,
                        local_version: change.base_version,
                        server_version,
                        server_data: ServerRouteData {
                            name: server_data.0,
                            description: server_data.1,
                            geojson: server_data.2,
                            metadata: server_data.3,
                            notes: server_data.4,
                        },
                    });
                }
            }
            None => {
                conflicts.push(ConflictInfo {
                    route_id: change.route_id,
                    local_version: change.base_version,
                    server_version: 0,
                    server_data: ServerRouteData {
                        name: None,
                        description: None,
                        geojson: serde_json::json!({}),
                        metadata: serde_json::json!({}),
                        notes: None,
                    },
                });
            }
        }
    }

    Ok(Json(PushResponse { accepted, conflicts }))
}

pub async fn pull(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<PullRequest>,
) -> AppResult<Json<PullResponse>> {
    let last_sync = body.last_synced_at.unwrap_or(DateTime::UNIX_EPOCH);

    // Fetch all routes updated since last sync
    let all_updated: Vec<UpdatedRouteRow> = sqlx::query_as(
        r#"
        SELECT r.id, r.version, r.name, r.description, r.geojson, r.metadata, r.notes
        FROM routes r
        JOIN sketches s ON r.sketch_id = s.id
        WHERE s.user_id = $1 AND r.updated_at > $2 AND r.deleted_at IS NULL
        "#
    )
    .bind(auth.user_id)
    .bind(last_sync)
    .fetch_all(&state.db)
    .await?;

    // Filter out routes where client already has the same version
    let updated_routes: Vec<UpdatedRoute> = all_updated
        .into_iter()
        .filter_map(|(route_id, version, name, description, geojson, metadata, notes)| {
            // Skip if client already has this exact version
            if let Some(&client_version) = body.known_versions.get(&route_id)
                && client_version == version
            {
                return None; // Client already has this version, skip it
            }
            Some(UpdatedRoute {
                route_id,
                version,
                data: RouteData {
                    name,
                    description,
                    geojson,
                    metadata,
                    notes,
                },
            })
        })
        .collect();

    // Log the sync operation
    sqlx::query(
        "INSERT INTO sync_log (route_id, user_id, action, version_before, version_after, client_id) VALUES ($1, $2, 'pull', $3, $4, $5)"
    )
    .bind(Uuid::nil()) // System route_id for pull operations
    .bind(auth.user_id)
    .bind(0)
    .bind(updated_routes.len() as i32)
    .bind(body.client_id)
    .execute(&state.db)
    .await?;

    let deleted: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT r.id
        FROM routes r
        JOIN sketches s ON r.sketch_id = s.id
        WHERE s.user_id = $1 AND r.deleted_at > $2
        "#
    )
    .bind(auth.user_id)
    .bind(last_sync)
    .fetch_all(&state.db)
    .await?;

    let deleted_ids: Vec<Uuid> = deleted.into_iter().map(|(id,)| id).collect();

    Ok(Json(PullResponse {
        updated: updated_routes,
        deleted: deleted_ids,
        server_time: Utc::now().to_rfc3339(),
    }))
}

pub async fn resolve(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(route_id): Path<Uuid>,
    Json(body): Json<ResolveRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let route: Option<(Uuid, i32)> = sqlx::query_as(
        r#"
        SELECT r.id, r.version
        FROM routes r
        JOIN sketches s ON r.sketch_id = s.id
        WHERE r.id = $1 AND s.user_id = $2 AND r.deleted_at IS NULL
        "#
    )
    .bind(route_id)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    match route {
        Some((_, server_version)) => {
            if server_version != body.base_version {
                return Err(AppError::Conflict("Version mismatch, please pull latest changes".to_string()));
            }

            sqlx::query(
                r#"
                UPDATE routes
                SET name = $1,
                    description = $2,
                    geojson = $3,
                    metadata = $4,
                    notes = $5,
                    version = version + 1,
                    updated_at = NOW()
                WHERE id = $6
                "#
            )
            .bind(&body.data.name)
            .bind(&body.data.description)
            .bind(&body.data.geojson)
            .bind(&body.data.metadata)
            .bind(&body.data.notes)
            .bind(route_id)
            .execute(&state.db)
            .await?;

            sqlx::query(
                "INSERT INTO sync_log (route_id, user_id, action, version_before, version_after, resolution, client_id) VALUES ($1, $2, 'conflict_resolve', $3, $4, $5, $6)"
            )
            .bind(route_id)
            .bind(auth.user_id)
            .bind(body.base_version)
            .bind(server_version + 1)
            .bind(&body.resolution)
            .bind(Uuid::new_v4())
            .execute(&state.db)
            .await?;

            Ok(Json(serde_json::json!({ "resolved": true })))
        }
        None => Err(AppError::NotFound("Route not found".to_string())),
    }
}
