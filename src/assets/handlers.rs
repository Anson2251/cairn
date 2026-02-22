use axum::{
    extract::{Path, State},
    http::Response,
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthContext;
use crate::AppState;

const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB

#[derive(Debug, FromRow)]
pub struct Asset {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub original_filename: Option<String>,
    pub mime_type: String,
    pub size: i64,
    pub hash: String,
    pub data: Vec<u8>,
    pub created_at: chrono::DateTime<Utc>,
}

pub async fn upload(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    mut multipart: axum::extract::Multipart,
) -> AppResult<axum::Json<serde_json::Value>> {
    let field = multipart
        .next_field()
        .await?;

    let mut field = field.ok_or_else(|| AppError::Validation("No file provided".to_string()))?;

    let filename = field.file_name().map(|s| s.to_string());
    let mime_type = field
        .content_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let mut bytes = Vec::new();
    let mut size: usize = 0;

    while let Some(chunk) = field.chunk().await? {
        size += chunk.len();
        if size > MAX_FILE_SIZE {
            return Err(AppError::Validation("File too large (max 10MB)".to_string()));
        }
        bytes.extend_from_slice(&chunk);
    }

    let hash = format!("{:x}", Sha256::digest(&bytes));

    let ext = mime_type
        .split('/')
        .nth(1)
        .map(|e| {
            if e == "jpeg" {
                "jpg".to_string()
            } else {
                e.to_string()
            }
        })
        .unwrap_or_else(|| "bin".to_string());

    let stored_filename = format!("{}.{}", &hash[..64], ext);

    let existing: Option<Asset> = sqlx::query_as(
        "SELECT id, owner_id, original_filename, mime_type, size, hash, created_at FROM assets WHERE hash = $1 AND owner_id = $2"
    )
    .bind(&hash)
    .bind(auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_none() {
        sqlx::query(
            r#"
            INSERT INTO assets (owner_id, original_filename, mime_type, size, hash, data)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(auth.user_id)
        .bind(&filename)
        .bind(&mime_type)
        .bind(size as i64)
        .bind(&hash)
        .bind(&bytes)
        .execute(&state.db)
        .await?;
    }

    let url = format!("/assets/{}.{}", &hash[..64], ext);

    Ok(axum::Json(serde_json::json!({
        "asset": {
            "id": existing.map(|e| e.id).unwrap_or_else(|| Uuid::new_v4()),
            "hash": hash,
            "filename": stored_filename,
            "mime_type": mime_type,
            "size": size as i64,
            "url": url,
        }
    })))
}

pub async fn serve(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let hash = filename
        .rsplit('.')
        .next()
        .ok_or_else(|| AppError::NotFound("Invalid filename".to_string()))?;

    let asset: Option<Asset> = sqlx::query_as(
        "SELECT * FROM assets WHERE hash = $1"
    )
    .bind(hash)
    .fetch_optional(&state.db)
    .await?;

    let asset = asset.ok_or_else(|| AppError::NotFound("Asset not found".to_string()))?;

    if asset.owner_id != auth.user_id {
        let has_shared_access = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM shares
                JOIN sketches ON shares.sketch_id = sketches.id
                WHERE sketches.user_id = $1 AND shares.user_id = $2
            )
            "#
        )
        .bind(auth.user_id)
        .bind(asset.owner_id)
        .fetch_one(&state.db)
        .await?;

        if !has_shared_access {
            return Err(AppError::Unauthorized("Access denied".to_string()));
        }
    }

    let mut response: Response<axum::body::Body> = Response::new(asset.data.into());
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        asset.mime_type.parse().unwrap(),
    );
    response.headers_mut().insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("inline; filename=\"{}\"", asset.original_filename.unwrap_or_else(|| filename.clone()))
            .parse()
            .unwrap(),
    );

    Ok(response)
}
