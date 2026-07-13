//! Provider target model management and upstream model discovery.

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
pub struct ProviderTargetModelResponse {
    pub id: String,
    pub provider_id: String,
    pub target_model: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTargetModelsResponse {
    pub target_models: Vec<ProviderTargetModelResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTargetModelRequest {
    pub target_model: String,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

pub async fn get_provider_target_models(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<Json<ProviderTargetModelsResponse>, AppError> {
    let id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let rows = sqlx::query(
        "SELECT id, provider_id, target_model, is_active, created_at \
         FROM provider_target_models \
         WHERE provider_id = $1 \
         ORDER BY target_model",
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch provider target models: {}", e)))?;

    Ok(Json(ProviderTargetModelsResponse {
        target_models: rows.into_iter().map(row_to_response).collect(),
    }))
}

pub async fn create_provider_target_model(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Json(req): Json<CreateTargetModelRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let provider_id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let target_model = req.target_model.trim();
    if target_model.is_empty() {
        return Err(AppError::BadRequest(
            "Target model cannot be empty".to_string(),
        ));
    }

    let id: Uuid = sqlx::query(
        "INSERT INTO provider_target_models (provider_id, target_model, is_active) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(provider_id)
    .bind(target_model)
    .bind(req.is_active)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to create provider target model: {}", e)))?
    .try_get("id")
    .map_err(|e| AppError::Internal(format!("Failed to get created target model id: {}", e)))?;

    reload_provider_state(&state).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id.to_string(),
            "message": "Provider target model created."
        })),
    ))
}

pub async fn delete_provider_target_model(
    State(state): State<AppState>,
    Path((provider_id, target_model_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let _provider_id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;
    let target_model_id = Uuid::parse_str(&target_model_id)
        .map_err(|_| AppError::BadRequest("Invalid target model UUID format".to_string()))?;

    let result = sqlx::query("DELETE FROM provider_target_models WHERE id = $1")
        .bind(target_model_id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            AppError::Database(format!("Failed to delete provider target model: {}", e))
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Provider target model not found".to_string(),
        ));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct SyncedModelsResponse {
    pub added: Vec<String>,
}

/// Fetches the live model list from the provider's `/models` endpoint and adds
/// any models not already present in `provider_target_models`.
pub async fn sync_provider_target_models(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<Json<SyncedModelsResponse>, AppError> {
    let provider_id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let row = sqlx::query(
        "SELECT base_url, auth_type, auth_header_name, auth_prefix \
         FROM providers WHERE id = $1 AND is_active = true",
    )
    .bind(provider_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load provider for sync: {}", e)))?
    .ok_or_else(|| AppError::NotFound("Provider not found".to_string()))?;

    let base_url: String = row
        .try_get("base_url")
        .map_err(|e| AppError::Internal(format!("Missing provider base_url for sync: {}", e)))?;
    let auth_type: String = row
        .try_get("auth_type")
        .map_err(|e| AppError::Internal(format!("Missing provider auth_type for sync: {}", e)))?;
    let auth_header_name: Option<String> = row.try_get("auth_header_name").ok();
    let auth_prefix: Option<String> = row.try_get("auth_prefix").ok();

    // Load one active key to use for the preview request.
    let key_row = sqlx::query(
        "SELECT encrypted_key FROM provider_api_keys \
         WHERE provider_id = $1 AND is_active = true ORDER BY priority DESC, created_at LIMIT 1",
    )
    .bind(provider_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load provider API key for sync: {}", e)))?;

    let api_key = match key_row {
        Some(row) => Some(
            row.try_get::<String, _>("encrypted_key")
                .map_err(|e| AppError::Internal(format!("Missing encrypted_key: {}", e)))?,
        ),
        None => None,
    };

    tracing::info!(
        provider_id = %provider_id,
        base_url = %base_url,
        has_api_key = api_key.is_some(),
        "sync_provider_target_models: fetching upstream models"
    );

    let models = preview_upstream_models(
        &state.vault,
        &base_url,
        &auth_type,
        auth_header_name.as_deref(),
        auth_prefix.as_deref(),
        api_key.as_deref(),
    )
    .await?;

    tracing::info!(
        provider_id = %provider_id,
        found_models = models.len(),
        "sync_provider_target_models: models received from upstream"
    );

    let mut added: Vec<String> = Vec::new();
    for model in models {
        let result = sqlx::query(
            "INSERT INTO provider_target_models (provider_id, target_model, is_active) \
             VALUES ($1, $2, true) \
             ON CONFLICT (provider_id, target_model) DO NOTHING",
        )
        .bind(provider_id)
        .bind(&model)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to insert synced target model: {}", e)))?;

        if result.rows_affected() > 0 {
            added.push(model);
        }
    }

    reload_provider_state(&state).await?;

    Ok(Json(SyncedModelsResponse { added }))
}

async fn preview_upstream_models(
    vault: &zorch_shared::SecretVault,
    base_url: &str,
    auth_type: &str,
    auth_header_name: Option<&str>,
    auth_prefix: Option<&str>,
    encrypted_api_key: Option<&str>,
) -> Result<Vec<String>, AppError> {
    use zorch_providers::{AuthHeaders, AuthType};

    let url = format!("{}/models", base_url.trim().trim_end_matches('/'));
    let auth = AuthType::from_config(auth_type, auth_header_name, auth_prefix)
        .map_err(|e| AppError::Internal(format!("Invalid auth config for preview: {}", e)))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build preview client: {}", e)))?;

    let mut rb = client.get(&url);
    if let Some(encrypted) = encrypted_api_key {
        let decrypted = vault.decrypt(encrypted).map_err(|e| {
            AppError::Internal(format!("Failed to decrypt API key for preview: {}", e))
        })?;
        let headers = AuthHeaders::from_auth_type(&decrypted, &auth)
            .map_err(|e| {
                AppError::Internal(format!("Failed to build auth headers for sync: {}", e))
            })?
            .build();
        for (key, value) in headers.iter() {
            rb = rb.header(key.clone(), value.clone());
        }
    }

    let resp = rb.send().await.map_err(|e| {
        tracing::warn!(base_url = %url, error = %e, "preview_upstream_models: request failed");
        AppError::Provider(format!("Preview request failed: {}", e))
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        tracing::warn!(
            base_url = %url,
            upstream_status = %status,
            body = %body_text,
            "preview_upstream_models: non-success upstream"
        );
        return Err(AppError::Provider(format!(
            "Upstream returned status {}: {}",
            status.as_u16(),
            body_text
        )));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| {
        tracing::warn!(base_url = %url, error = %e, "preview_upstream_models: non-JSON body");
        AppError::Provider(format!("Preview returned non-JSON body: {}", e))
    })?;

    tracing::debug!(base_url = %url, body = %body, "preview_upstream_models: upstream response body");

    let raw = extract_models(&body).ok_or_else(|| {
        tracing::warn!(base_url = %url, body = %body, "preview_upstream_models: unrecognized models list shape");
        AppError::Provider(
            "could not recognize models list shape (expected object with data[]/models[], or array)"
                .to_string(),
        )
    })?;
    let (models, _truncated) = normalize_models(raw);
    tracing::info!(base_url = %url, models_found = models.len(), "preview_upstream_models: extracted models");
    Ok(models)
}

fn normalize_models(raw: Vec<String>) -> (Vec<String>, usize) {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::with_capacity(raw.len().min(1000));
    let mut truncated: usize = 0;
    const MAX: usize = 1000;
    for m in raw {
        let trimmed = m.trim().to_string();
        if trimmed.is_empty() || !seen.insert(trimmed.clone()) {
            continue;
        }
        if out.len() >= MAX {
            truncated += 1;
            continue;
        }
        out.push(trimmed);
    }
    (out, truncated)
}

fn extract_models(body: &serde_json::Value) -> Option<Vec<String>> {
    fn ids_from(arr: &serde_json::Value) -> Vec<String> {
        arr.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        v.get("id")
                            .or_else(|| v.get("name"))
                            .and_then(|i| i.as_str())
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn strings_from(arr: &serde_json::Value) -> Vec<String> {
        arr.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    if let Some(data) = body.get("data") {
        if data.is_array() {
            return Some(ids_from(data));
        }
    }

    if let Some(models) = body.get("models") {
        if models.is_array() {
            let ids = ids_from(models);
            if !ids.is_empty() {
                return Some(ids);
            }
            let strs = strings_from(models);
            if !strs.is_empty() {
                return Some(strs);
            }
        }
    }

    let ids = ids_from(body);
    if !ids.is_empty() {
        return Some(ids);
    }
    let strs = strings_from(body);
    if !strs.is_empty() {
        return Some(strs);
    }

    None
}

fn row_to_response(row: sqlx::postgres::PgRow) -> ProviderTargetModelResponse {
    let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::nil());
    let provider_id: Uuid = row.try_get("provider_id").unwrap_or_else(|_| Uuid::nil());
    let target_model: String = row
        .try_get("target_model")
        .unwrap_or_else(|_| "".to_string());
    let is_active: bool = row.try_get("is_active").unwrap_or(false);
    let created_at: chrono::DateTime<chrono::Utc> = row
        .try_get("created_at")
        .unwrap_or_else(|_| chrono::Utc::now());

    ProviderTargetModelResponse {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        target_model,
        is_active,
        created_at: created_at.to_rfc3339(),
    }
}
