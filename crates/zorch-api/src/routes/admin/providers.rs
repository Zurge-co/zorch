//! Provider management endpoints: list / create / update / delete / toggle-active.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use zorch_providers::AuthType;
use zorch_shared::AppError;

use crate::AppState;

use super::providers_state::reload_provider_state;
use super::types::{ProviderResponse, ProvidersResponse};

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
    pub base_url: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default)]
    pub auth_header_name: Option<String>,
    #[serde(default)]
    pub auth_prefix: Option<String>,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

fn default_auth_type() -> String {
    AuthType::default().to_string()
}

fn parse_auth_type(
    auth_type: &str,
    auth_header_name: Option<&str>,
    auth_prefix: Option<&str>,
) -> Result<AuthType, AppError> {
    AuthType::from_config(auth_type, auth_header_name, auth_prefix).map_err(|_| {
        AppError::BadRequest(format!(
            "Invalid auth type '{}'. Supported types: bearer, anthropic, custom",
            auth_type
        ))
    })
}

fn validate_provider(req: &CreateProviderRequest) -> Result<(), AppError> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest(
            "Provider name cannot be empty".to_string(),
        ));
    }
    if req.base_url.is_empty() {
        return Err(AppError::BadRequest("Base URL cannot be empty".to_string()));
    }
    parse_auth_type(
        &req.auth_type,
        req.auth_header_name.as_deref(),
        req.auth_prefix.as_deref(),
    )?;
    Ok(())
}

pub async fn create_provider(
    State(state): State<AppState>,
    Json(req): Json<CreateProviderRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    validate_provider(&req)?;

    let id: Uuid = sqlx::query(
        "INSERT INTO providers (name, base_url, auth_type, auth_header_name, auth_prefix, is_active) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(&req.name)
    .bind(&req.base_url)
    .bind(req.auth_type.trim().to_lowercase())
    .bind(req.auth_header_name.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()))
    .bind(req.auth_prefix.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()))
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
    pub base_url: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default)]
    pub auth_header_name: Option<String>,
    #[serde(default)]
    pub auth_prefix: Option<String>,
    pub is_active: bool,
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
    parse_auth_type(
        &req.auth_type,
        req.auth_header_name.as_deref(),
        req.auth_prefix.as_deref(),
    )?;

    let result = sqlx::query(
        "UPDATE providers \
         SET name = $1, base_url = $2, auth_type = $3, auth_header_name = $4, auth_prefix = $5, is_active = $6 \
         WHERE id = $7",
    )
    .bind(&req.name)
    .bind(&req.base_url)
    .bind(req.auth_type.trim().to_lowercase())
    .bind(req.auth_header_name.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()))
    .bind(req.auth_prefix.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()))
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
/// `name`, `base_url`, or auth fields. Used by the per-row "Routing Enabled"
/// Switch in the admin dashboard so toggles never wipe existing config.
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
            id, name, base_url, auth_type, auth_header_name, auth_prefix, is_active, created_at
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
    let auth_type: String = row
        .try_get("auth_type")
        .unwrap_or_else(|_| "bearer".to_string());
    let auth_header_name: Option<String> = row.try_get("auth_header_name").ok();
    let auth_prefix: Option<String> = row.try_get("auth_prefix").ok();
    let is_active: bool = row.try_get("is_active").unwrap_or(false);

    ProviderResponse {
        id: id.to_string(),
        name,
        base_url,
        auth_type,
        auth_header_name,
        auth_prefix,
        status: if is_active { "online" } else { "offline" }.to_string(),
        models: vec![],
        latency: "0ms".to_string(),
        cost_per_1m: "$0.00".to_string(),
    }
}
