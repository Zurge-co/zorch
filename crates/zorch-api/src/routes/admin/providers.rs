//! Provider management endpoints: list / create / update / delete / toggle-active.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use zorch_providers::Protocol;
use zorch_shared::AppError;

use crate::AppState;

use super::{
    providers_state::{merge_provider_config, reload_provider_state},
    types::{ProviderResponse, ProvidersResponse},
};

pub async fn get_providers(
    State(state): State<AppState>,
) -> Result<Json<ProvidersResponse>, AppError> {
    let providers = fetch_providers(&state.db_pool).await?;
    Ok(Json(ProvidersResponse { providers }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderRequest {
    pub name: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub base_url: String,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_protocol() -> String {
    Protocol::default().as_str().to_string()
}

fn parse_protocol(protocol: &str) -> Result<Protocol, AppError> {
    protocol.parse().map_err(|_| {
        AppError::BadRequest(format!(
            "Invalid provider protocol '{}'. Supported protocols: openai_compatible, anthropic",
            protocol
        ))
    })
}

fn merge_config_overlay(
    mut base: serde_json::Value,
    overlay: serde_json::Value,
) -> serde_json::Value {
    if overlay.is_null() {
        return base;
    }

    let Some(base_obj) = base.as_object_mut() else {
        return overlay;
    };
    let Some(overlay_obj) = overlay.as_object() else {
        return overlay;
    };

    for (key, value) in overlay_obj {
        base_obj.insert(key.clone(), value.clone());
    }

    base
}

pub async fn create_provider(
    State(state): State<AppState>,
    Json(req): Json<CreateProviderRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest(
            "Provider name cannot be empty".to_string(),
        ));
    }
    if req.base_url.is_empty() {
        return Err(AppError::BadRequest("Base URL cannot be empty".to_string()));
    }
    if matches!(req.api_key.as_deref(), Some(k) if k.trim().is_empty()) {
        return Err(AppError::BadRequest("API key cannot be empty".to_string()));
    }

    let protocol = parse_protocol(&req.protocol)?;
    let config = merge_provider_config(
        &state.vault,
        req.config,
        req.api_key.as_deref(),
        &req.models,
        protocol,
    )?;

    let id: Uuid = sqlx::query(
        "INSERT INTO providers (name, base_url, config, is_active) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&req.name)
    .bind(&req.base_url)
    .bind(config)
    .bind(req.is_active)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to create provider: {}", e)))?
    .try_get("id")
    .map_err(|e| AppError::Internal(format!("Failed to get created provider id: {}", e)))?;

    reload_provider_state(&state).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id.to_string(),
            "message": "Provider created and activated."
        })),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderRequest {
    pub name: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub base_url: String,
    #[serde(default)]
    pub config: serde_json::Value,
    pub is_active: bool,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

pub async fn update_provider(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Json(req): Json<UpdateProviderRequest>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    if req.name.is_empty() {
        return Err(AppError::BadRequest(
            "Provider name cannot be empty".to_string(),
        ));
    }
    if req.base_url.is_empty() {
        return Err(AppError::BadRequest("Base URL cannot be empty".to_string()));
    }

    let existing_config: serde_json::Value =
        sqlx::query("SELECT config FROM providers WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to load provider config: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Provider not found".to_string()))?
            .try_get("config")
            .unwrap_or_else(|_| serde_json::json!({}));

    let protocol = parse_protocol(&req.protocol)?;
    let config = merge_provider_config(
        &state.vault,
        merge_config_overlay(existing_config, req.config),
        req.api_key.as_deref(),
        &req.models,
        protocol,
    )?;

    let result = sqlx::query(
        "UPDATE providers SET name = $1, base_url = $2, config = $3, is_active = $4 WHERE id = $5",
    )
    .bind(&req.name)
    .bind(&req.base_url)
    .bind(config)
    .bind(req.is_active)
    .bind(id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to update provider: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Provider not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_provider(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let result = sqlx::query("DELETE FROM providers WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete provider: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Provider not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SetProviderActiveRequest {
    pub is_active: bool,
}

/// Lightweight endpoint that flips only `is_active` without touching
/// `name`, `base_url`, or `config`. Used by the per-row "Routing Enabled"
/// Switch in the admin dashboard so toggles never wipe existing config JSON.
pub async fn set_provider_active(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Json(req): Json<SetProviderActiveRequest>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::parse_str(&provider_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let result = sqlx::query("UPDATE providers SET is_active = $1 WHERE id = $2")
        .bind(req.is_active)
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to set provider active state: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Provider not found".to_string()));
    }

    reload_provider_state(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_providers(pool: &PgPool) -> Result<Vec<ProviderResponse>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            id, name, base_url, config, is_active, created_at
        FROM providers
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(row_to_provider_response).collect())
}

fn row_to_provider_response(row: sqlx::postgres::PgRow) -> ProviderResponse {
    let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::nil());
    let name: String = row
        .try_get("name")
        .unwrap_or_else(|_| "Unknown".to_string());
    let base_url: String = row.try_get("base_url").unwrap_or_else(|_| "".to_string());
    let is_active: bool = row.try_get("is_active").unwrap_or(false);
    let config: serde_json::Value = row
        .try_get("config")
        .unwrap_or_else(|_| serde_json::json!({}));

    let models = config
        .get("models")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let protocol = config
        .get("protocol")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<Protocol>().ok())
        .unwrap_or_default()
        .as_str()
        .to_string();

    ProviderResponse {
        id: id.to_string(),
        name,
        protocol,
        base_url,
        status: if is_active { "online" } else { "offline" }.to_string(),
        models,
        latency: "0ms".to_string(),
        cost_per_1m: "$0.00".to_string(),
    }
}
