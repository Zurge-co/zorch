//! Provider target API key management.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use zorch_shared::AppError;

use crate::AppState;

use super::providers_state::reload_provider_state;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderApiKeyResponse {
    pub id: String,
    pub provider_id: String,
    pub label: Option<String>,
    pub masked_key: String,
    pub priority: i32,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderApiKeysResponse {
    pub api_keys: Vec<ProviderApiKeyResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyRequest {
    #[serde(default)]
    pub label: Option<String>,
    pub api_key: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct SetApiKeyActiveRequest {
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

pub async fn get_provider_api_keys(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<Json<ProviderApiKeysResponse>, AppError> {
    let id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let rows = sqlx::query(
        "SELECT id, provider_id, label, encrypted_key, priority, is_active, created_at \
         FROM provider_api_keys \
         WHERE provider_id = $1 \
         ORDER BY priority DESC, created_at",
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch provider API keys: {}", e)))?;

    Ok(Json(ProviderApiKeysResponse {
        api_keys: rows.into_iter().map(row_to_response).collect(),
    }))
}

pub async fn create_provider_api_key(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let provider_id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let key = req.api_key.trim();
    if key.is_empty() {
        return Err(AppError::BadRequest("API key cannot be empty".to_string()));
    }

    let encrypted = state
        .vault
        .encrypt(key)
        .map_err(|e| AppError::Internal(format!("Failed to encrypt provider API key: {}", e)))?;

    let id: Uuid = sqlx::query(
        "INSERT INTO provider_api_keys (provider_id, label, encrypted_key, priority, is_active) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(provider_id)
    .bind(
        req.label
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(encrypted)
    .bind(req.priority)
    .bind(req.is_active)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to create provider API key: {}", e)))?
    .try_get("id")
    .map_err(|e| AppError::Internal(format!("Failed to get created API key id: {}", e)))?;

    reload_provider_state(&state).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id.to_string(),
            "message": "Provider API key created."
        })),
    ))
}

pub async fn delete_provider_api_key(
    State(state): State<AppState>,
    Path((provider_id, key_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let _provider_id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;
    let key_id = Uuid::parse_str(&key_id)
        .map_err(|_| AppError::BadRequest("Invalid API key UUID format".to_string()))?;

    let result = sqlx::query("DELETE FROM provider_api_keys WHERE id = $1")
        .bind(key_id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete provider API key: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Provider API key not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_provider_api_key_active(
    State(state): State<AppState>,
    Path((provider_id, key_id)): Path<(String, String)>,
    Json(req): Json<SetApiKeyActiveRequest>,
) -> Result<StatusCode, AppError> {
    let _provider_id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;
    let key_id = Uuid::parse_str(&key_id)
        .map_err(|_| AppError::BadRequest("Invalid API key UUID format".to_string()))?;

    let result = sqlx::query("UPDATE provider_api_keys SET is_active = $1 WHERE id = $2")
        .bind(req.is_active)
        .bind(key_id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            AppError::Database(format!(
                "Failed to set provider API key active state: {}",
                e
            ))
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Provider API key not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn row_to_response(row: sqlx::postgres::PgRow) -> ProviderApiKeyResponse {
    let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::nil());
    let provider_id: Uuid = row.try_get("provider_id").unwrap_or_else(|_| Uuid::nil());
    let label: Option<String> = row.try_get("label").ok();
    let encrypted_key: String = row
        .try_get("encrypted_key")
        .unwrap_or_else(|_| "".to_string());
    let priority: i32 = row.try_get("priority").unwrap_or(0);
    let is_active: bool = row.try_get("is_active").unwrap_or(false);
    let created_at: chrono::DateTime<chrono::Utc> = row
        .try_get("created_at")
        .unwrap_or_else(|_| chrono::Utc::now());

    // Mask the encrypted key; the real value never leaves the backend.
    let masked_key = mask_key(&encrypted_key);

    ProviderApiKeyResponse {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        label,
        masked_key,
        priority,
        is_active,
        created_at: created_at.to_rfc3339(),
    }
}
