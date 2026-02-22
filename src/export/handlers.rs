use axum::{
    extract::{Path, State},
    http::Response,
    response::IntoResponse,
    Json,
    Extension,
};
use serde::Serialize;
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthContext;
use crate::AppState;

#[derive(Debug, FromRow)]
struct SketchExport {
    id: Uuid,
    name: String,
    description: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, FromRow)]
struct RouteExport {
    id: Uuid,
    sketch_id: Uuid,
    name: Option<String>,
    description: Option<String>,
    geojson: serde_json::Value,
    metadata: serde_json::Value,
    notes: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExportData {
    exported_at: String,
    user: ExportUser,
    sketches: Vec<ExportSketch>,
}

#[derive(Debug, Serialize)]
struct ExportUser {
    username: String,
    email: String,
}

#[derive(Debug, Serialize)]
struct ExportSketch {
    id: String,
    name: String,
    description: Option<String>,
    created_at: String,
    updated_at: String,
    routes: Vec<ExportRoute>,
}

#[derive(Debug, Serialize)]
struct ExportRoute {
    id: String,
    name: Option<String>,
    description: Option<String>,
    geojson: serde_json::Value,
    metadata: serde_json::Value,
    notes: Option<String>,
}

pub async fn create_export(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> AppResult<impl IntoResponse> {
    let user: (String, String) = sqlx::query_as(
        "SELECT username, email FROM users WHERE id = $1"
    )
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await?;

    let sketches: Vec<SketchExport> = sqlx::query_as(
        "SELECT id, name, description, created_at, updated_at FROM sketches WHERE user_id = $1 AND deleted_at IS NULL"
    )
    .bind(auth.user_id)
    .fetch_all(&state.db)
    .await?;

    let mut export_sketches = Vec::new();

    for sketch in sketches {
        let routes: Vec<RouteExport> = sqlx::query_as(
            "SELECT id, sketch_id, name, description, geojson, metadata, notes FROM routes WHERE sketch_id = $1 AND deleted_at IS NULL"
        )
        .bind(sketch.id)
        .fetch_all(&state.db)
        .await?;

        let export_routes: Vec<ExportRoute> = routes
            .into_iter()
            .map(|r| ExportRoute {
                id: r.id.to_string(),
                name: r.name,
                description: r.description,
                geojson: r.geojson,
                metadata: r.metadata,
                notes: r.notes,
            })
            .collect();

        export_sketches.push(ExportSketch {
            id: sketch.id.to_string(),
            name: sketch.name,
            description: sketch.description,
            created_at: sketch.created_at.to_rfc3339(),
            updated_at: sketch.updated_at.to_rfc3339(),
            routes: export_routes,
        });
    }

    let export_data = ExportData {
        exported_at: chrono::Utc::now().to_rfc3339(),
        user: ExportUser {
            username: user.0,
            email: user.1,
        },
        sketches: export_sketches,
    };

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let filename = format!("cairn-export-{}.json", chrono::Utc::now().format("%Y%m%d-%H%M%S"));

    let mut response: Response<axum::body::Body> = Response::new(json.into());
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    response.headers_mut().insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    Ok(response)
}

pub async fn get_export_status(
    State(_state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    Ok(Json(serde_json::json!({
        "job_id": job_id,
        "status": "not_implemented",
        "message": "Export is synchronous for now. Use POST /api/export directly."
    })))
}
