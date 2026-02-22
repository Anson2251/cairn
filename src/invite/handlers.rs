use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::invite::generator::generate_invite_code;
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct InviteCodeResponse {
    pub id: Uuid,
    pub sequence: i32,
    pub code: String,
    pub cairn_name: String,
    pub origin_coord: Option<(f64, f64)>,
    pub used: bool,
    pub used_by: Option<Uuid>,
    pub used_at: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, FromRow)]
pub struct InviteCodeRow {
    pub id: Uuid,
    pub sequence: i32,
    pub code: String,
    pub cairn_name: String,
    pub origin_coord: Option<sqlx::postgres::types::PgPoint>,
    pub used: bool,
    pub used_by: Option<Uuid>,
    pub used_at: Option<chrono::DateTime<Utc>>,
    pub expires_at: Option<chrono::DateTime<Utc>>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub count: Option<i32>,
    pub expires_days: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CreateInviteResponse {
    pub codes: Vec<InviteCodeResponse>,
}

pub async fn validate_invite(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let invite: Option<InviteCodeRow> = sqlx::query_as(
        "SELECT * FROM invite_codes WHERE code = $1"
    )
    .bind(&code)
    .fetch_optional(&state.db)
    .await?;

    match invite {
        Some(inv) => {
            if inv.used {
                return Err(AppError::InviteCodeAlreadyUsed);
            }
            if inv.expires_at.is_some_and(|e| e < Utc::now()) {
                return Err(AppError::InviteCodeExpired);
            }

            Ok(Json(serde_json::json!({
                "valid": true,
                "cairn_name": inv.cairn_name,
            })))
        }
        None => Err(AppError::InvalidInviteCode),
    }
}

pub async fn list_invites(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<InviteCodeResponse>>> {
    let invites: Vec<InviteCodeRow> = sqlx::query_as(
        "SELECT * FROM invite_codes ORDER BY sequence DESC"
    )
    .fetch_all(&state.db)
    .await?;

    let responses: Vec<InviteCodeResponse> = invites
        .into_iter()
        .map(|inv| InviteCodeResponse {
            id: inv.id,
            sequence: inv.sequence,
            code: inv.code,
            cairn_name: inv.cairn_name,
            origin_coord: inv.origin_coord.map(|p| (p.x, p.y)),
            used: inv.used,
            used_by: inv.used_by,
            used_at: inv.used_at.map(|d| d.to_rfc3339()),
            expires_at: inv.expires_at.map(|d| d.to_rfc3339()),
            created_at: inv.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(responses))
}

pub async fn create_invites(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateInviteRequest>,
) -> AppResult<Json<CreateInviteResponse>> {
    let count = body.count.unwrap_or(1).clamp(1, 100);
    let expires_at = body.expires_days.map(|days| Utc::now() + Duration::days(days as i64));

    let max_sequence: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(sequence) FROM invite_codes"
    )
    .fetch_one(&state.db)
    .await?;

    let start_sequence = max_sequence.unwrap_or(0) + 1;
    let mut codes = Vec::new();

    for i in 0..count {
        let sequence = start_sequence + i;
        let code_data = generate_invite_code(sequence, &state.config.invite.salt);

        let point = sqlx::postgres::types::PgPoint {
            x: code_data.origin_coord.0,
            y: code_data.origin_coord.1,
        };

        let invite_row: InviteCodeRow = sqlx::query_as(
            r#"
            INSERT INTO invite_codes (sequence, code, cairn_name, origin_coord, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#
        )
        .bind(sequence)
        .bind(&code_data.code)
        .bind(&code_data.cairn_name)
        .bind(point)
        .bind(expires_at)
        .fetch_one(&state.db)
        .await?;

        codes.push(InviteCodeResponse {
            id: invite_row.id,
            sequence: invite_row.sequence,
            code: invite_row.code,
            cairn_name: invite_row.cairn_name,
            origin_coord: invite_row.origin_coord.map(|p| (p.x, p.y)),
            used: invite_row.used,
            used_by: invite_row.used_by,
            used_at: invite_row.used_at.map(|d| d.to_rfc3339()),
            expires_at: invite_row.expires_at.map(|d| d.to_rfc3339()),
            created_at: invite_row.created_at.to_rfc3339(),
        });
    }

    Ok(Json(CreateInviteResponse { codes }))
}

pub async fn revoke_invite(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let result = sqlx::query(
        "DELETE FROM invite_codes WHERE id = $1 AND used = FALSE"
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Invite code not found or already used".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct TrailblazerResponse {
    pub sequence: i32,
    pub cairn_name: String,
    pub origin_coord: Option<(f64, f64)>,
    pub joined_at: String,
}

pub async fn list_trailblazers(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<TrailblazerResponse>>> {
    let trailblazers: Vec<(i32, String, Option<sqlx::postgres::types::PgPoint>, chrono::DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT ic.sequence, ic.cairn_name, ic.origin_coord, u.created_at
        FROM invite_codes ic
        JOIN users u ON u.invite_code_id = ic.id
        WHERE ic.used = TRUE
        ORDER BY ic.sequence ASC
        "#
    )
    .fetch_all(&state.db)
    .await?;

    let responses: Vec<TrailblazerResponse> = trailblazers
        .into_iter()
        .map(|(sequence, cairn_name, origin_coord, joined_at)| TrailblazerResponse {
            sequence,
            cairn_name,
            origin_coord: origin_coord.map(|p| (p.x, p.y)),
            joined_at: joined_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(responses))
}
