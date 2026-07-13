//! Public model management endpoints.

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
pub struct ModelResponse {
    pub id: String,
    pub public_name: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsResponse {
    pub models: Vec<ModelResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelRequest {
    pub public_name: String,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelRequest {
    pub public_name: String,
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

pub async fn get_models(State(state): State<AppState>) -> Result<Json<ModelsResponse>, AppError> {
    let models = fetch_models(&state.db_pool).await?;
    Ok(Json(ModelsResponse { models }))
}

pub async fn create_model(
    State(state): State<AppState>,
    Json(req): Json<CreateModelRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let public_name = req.public_name.trim();
    if public_name.is_empty() {
        return Err(AppError::BadRequest(
            "Public name cannot be empty".to_string(),
        ));
    }

    let id: Uuid =
        sqlx::query("INSERT INTO models (public_name, is_active) VALUES ($1, $2) RETURNING id")
            .bind(public_name)
            .bind(req.is_active)
            .fetch_one(&state.db_pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to create model: {}", e)))?
            .try_get("id")
            .map_err(|e| AppError::Internal(format!("Failed to get created model id: {}", e)))?;

    reload_provider_state(&state).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id.to_string(),
            "message": "Model created."
        })),
    ))
}

pub async fn get_model(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
) -> Result<Json<ModelResponse>, AppError> {
    let id = Uuid::parse_str(&model_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let row =
        sqlx::query("SELECT id, public_name, is_active, created_at FROM models WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to load model: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Model not found".to_string()))?;

    Ok(Json(row_to_model_response(row)))
}

pub async fn update_model(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
    Json(req): Json<UpdateModelRequest>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::parse_str(&model_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let public_name = req.public_name.trim();
    if public_name.is_empty() {
        return Err(AppError::BadRequest(
            "Public name cannot be empty".to_string(),
        ));
    }

    let result = sqlx::query("UPDATE models SET public_name = $1, is_active = $2 WHERE id = $3")
        .bind(public_name)
        .bind(req.is_active)
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update model: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Model not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_model(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::parse_str(&model_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let result = sqlx::query("DELETE FROM models WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete model: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Model not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_models(pool: &PgPool) -> Result<Vec<ModelResponse>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, public_name, is_active, created_at FROM models ORDER BY public_name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(row_to_model_response).collect())
}

fn row_to_model_response(row: sqlx::postgres::PgRow) -> ModelResponse {
    let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::nil());
    let public_name: String = row
        .try_get("public_name")
        .unwrap_or_else(|_| "Unknown".to_string());
    let is_active: bool = row.try_get("is_active").unwrap_or(false);
    let created_at: chrono::DateTime<chrono::Utc> = row
        .try_get("created_at")
        .unwrap_or_else(|_| chrono::Utc::now());

    ModelResponse {
        id: id.to_string(),
        public_name,
        is_active,
        created_at: created_at.to_rfc3339(),
    }
}
