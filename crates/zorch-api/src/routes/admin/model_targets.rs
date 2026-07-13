//! Model target management endpoints.
//!
//! A model target maps a public model to a concrete provider target model.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use zorch_shared::AppError;

use crate::AppState;

use super::providers_state::reload_provider_state;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTargetResponse {
    pub id: String,
    pub model_id: String,
    pub provider_target_model_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub target_model: String,
    pub priority: i32,
    pub is_active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTargetsResponse {
    pub targets: Vec<ModelTargetResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelTargetRequest {
    pub provider_target_model_id: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelTargetRequest {
    pub provider_target_model_id: String,
    pub priority: i32,
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

pub async fn get_model_targets(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
) -> Result<Json<ModelTargetsResponse>, AppError> {
    let id = Uuid::parse_str(&model_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let targets = fetch_model_targets(&state.db_pool, id).await?;
    Ok(Json(ModelTargetsResponse { targets }))
}

pub async fn get_provider_targets(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<Json<ModelTargetsResponse>, AppError> {
    let id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let rows = sqlx::query(
        "SELECT mt.id, mt.model_id, mt.provider_target_model_id, p.id as provider_id, \
         p.name as provider_name, ptm.target_model, mt.priority, mt.is_active \
         FROM model_targets mt \
         JOIN provider_target_models ptm ON mt.provider_target_model_id = ptm.id \
         JOIN providers p ON ptm.provider_id = p.id \
         WHERE ptm.provider_id = $1 \
         ORDER BY ptm.target_model, mt.priority DESC",
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch provider targets: {}", e)))?;

    Ok(Json(ModelTargetsResponse {
        targets: rows.into_iter().map(row_to_target_response).collect(),
    }))
}

pub async fn create_model_target(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
    Json(req): Json<CreateModelTargetRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let model_id = Uuid::parse_str(&model_id)
        .map_err(|_| AppError::BadRequest("Invalid model UUID format".to_string()))?;
    let provider_target_model_id =
        Uuid::parse_str(&req.provider_target_model_id).map_err(|_| {
            AppError::BadRequest("Invalid provider target model UUID format".to_string())
        })?;

    let id: Uuid = sqlx::query(
        "INSERT INTO model_targets (model_id, provider_target_model_id, priority, is_active) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(model_id)
    .bind(provider_target_model_id)
    .bind(req.priority)
    .bind(req.is_active)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to create model target: {}", e)))?
    .try_get("id")
    .map_err(|e| AppError::Internal(format!("Failed to get created target id: {}", e)))?;

    reload_provider_state(&state).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id.to_string(),
            "message": "Model target created."
        })),
    ))
}

pub async fn update_model_target(
    State(state): State<AppState>,
    Path((model_id, target_id)): Path<(String, String)>,
    Json(req): Json<UpdateModelTargetRequest>,
) -> Result<StatusCode, AppError> {
    let _model_id = Uuid::parse_str(&model_id)
        .map_err(|_| AppError::BadRequest("Invalid model UUID format".to_string()))?;
    let target_id = Uuid::parse_str(&target_id)
        .map_err(|_| AppError::BadRequest("Invalid target UUID format".to_string()))?;
    let provider_target_model_id =
        Uuid::parse_str(&req.provider_target_model_id).map_err(|_| {
            AppError::BadRequest("Invalid provider target model UUID format".to_string())
        })?;

    let result = sqlx::query(
        "UPDATE model_targets \
         SET provider_target_model_id = $1, priority = $2, is_active = $3 \
         WHERE id = $4",
    )
    .bind(provider_target_model_id)
    .bind(req.priority)
    .bind(req.is_active)
    .bind(target_id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to update model target: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Model target not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_model_target(
    State(state): State<AppState>,
    Path((model_id, target_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let _model_id = Uuid::parse_str(&model_id)
        .map_err(|_| AppError::BadRequest("Invalid model UUID format".to_string()))?;
    let target_id = Uuid::parse_str(&target_id)
        .map_err(|_| AppError::BadRequest("Invalid target UUID format".to_string()))?;

    let result = sqlx::query("DELETE FROM model_targets WHERE id = $1")
        .bind(target_id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete model target: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Model target not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_model_targets(
    pool: &PgPool,
    model_id: Uuid,
) -> Result<Vec<ModelTargetResponse>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT mt.id, mt.model_id, mt.provider_target_model_id, p.id as provider_id, \
         p.name as provider_name, ptm.target_model, mt.priority, mt.is_active \
         FROM model_targets mt \
         JOIN provider_target_models ptm ON mt.provider_target_model_id = ptm.id \
         JOIN providers p ON ptm.provider_id = p.id \
         WHERE mt.model_id = $1 \
         ORDER BY mt.priority DESC, mt.created_at",
    )
    .bind(model_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(row_to_target_response).collect())
}

fn row_to_target_response(row: sqlx::postgres::PgRow) -> ModelTargetResponse {
    let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::nil());
    let model_id: Uuid = row.try_get("model_id").unwrap_or_else(|_| Uuid::nil());
    let provider_target_model_id: Uuid = row
        .try_get("provider_target_model_id")
        .unwrap_or_else(|_| Uuid::nil());
    let provider_id: Uuid = row.try_get("provider_id").unwrap_or_else(|_| Uuid::nil());
    let provider_name: String = row
        .try_get("provider_name")
        .unwrap_or_else(|_| "Unknown".to_string());
    let target_model: String = row
        .try_get("target_model")
        .unwrap_or_else(|_| "".to_string());
    let priority: i32 = row.try_get("priority").unwrap_or(0);
    let is_active: bool = row.try_get("is_active").unwrap_or(false);

    ModelTargetResponse {
        id: id.to_string(),
        model_id: model_id.to_string(),
        provider_target_model_id: provider_target_model_id.to_string(),
        provider_id: provider_id.to_string(),
        provider_name,
        target_model,
        priority,
        is_active,
    }
}
