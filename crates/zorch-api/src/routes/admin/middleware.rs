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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiddlewarePluginResponse {
    pub id: String,
    pub plugin_key: String,
    pub name: String,
    pub description: String,
    pub runtime: String,
    pub version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiddlewareConfigResponse {
    pub id: String,
    pub plugin_key: String,
    pub enabled: bool,
    pub phase: String,
    pub priority: i32,
    pub failure_mode: String,
    pub scope: serde_json::Value,
    pub config: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiddlewareRunResponse {
    pub id: String,
    pub request_id: Option<String>,
    pub plugin_key: String,
    pub phase: String,
    pub status: String,
    pub action: String,
    pub duration_ms: i32,
    pub body_changed: bool,
    pub metadata: serde_json::Value,
    pub error: Option<String>,
    pub created_at: String,
}

pub async fn get_middleware_plugins(
    State(state): State<AppState>,
) -> Result<Json<Vec<MiddlewarePluginResponse>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT id::text, plugin_key, name, description, runtime, version
        FROM middleware_plugins
        ORDER BY name
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to list middleware plugins: {}", e)))?;

    let plugins = rows
        .into_iter()
        .map(|row| MiddlewarePluginResponse {
            id: row.try_get("id").unwrap_or_default(),
            plugin_key: row.try_get("plugin_key").unwrap_or_default(),
            name: row.try_get("name").unwrap_or_default(),
            description: row.try_get("description").unwrap_or_default(),
            runtime: row.try_get("runtime").unwrap_or_default(),
            version: row.try_get("version").unwrap_or_default(),
        })
        .collect();

    Ok(Json(plugins))
}

pub async fn get_middleware_configs(
    State(state): State<AppState>,
) -> Result<Json<Vec<MiddlewareConfigResponse>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id::text,
            plugin_key,
            enabled,
            phase,
            priority,
            failure_mode,
            scope,
            config,
            created_at::text,
            updated_at::text
        FROM middleware_configs
        ORDER BY priority ASC, plugin_key ASC
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to list middleware configs: {}", e)))?;

    let configs = rows
        .into_iter()
        .map(|row| MiddlewareConfigResponse {
            id: row.try_get("id").unwrap_or_default(),
            plugin_key: row.try_get("plugin_key").unwrap_or_default(),
            enabled: row.try_get("enabled").unwrap_or(true),
            phase: row.try_get("phase").unwrap_or_default(),
            priority: row.try_get("priority").unwrap_or(100),
            failure_mode: row.try_get("failure_mode").unwrap_or_default(),
            scope: row.try_get("scope").unwrap_or_else(|_| serde_json::json!({})),
            config: row.try_get("config").unwrap_or_else(|_| serde_json::json!({})),
            created_at: row.try_get("created_at").unwrap_or_default(),
            updated_at: row.try_get("updated_at").unwrap_or_default(),
        })
        .collect();

    Ok(Json(configs))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMiddlewareConfigRequest {
    pub plugin_key: String,
    pub enabled: bool,
    pub phase: String,
    pub priority: i32,
    pub failure_mode: String,
    #[serde(default)]
    pub scope: serde_json::Value,
    #[serde(default)]
    pub config: serde_json::Value,
}

pub async fn create_middleware_config(
    State(state): State<AppState>,
    Json(req): Json<CreateMiddlewareConfigRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    validate_middleware_config(&req)?;

    let id: Uuid = sqlx::query(
        r#"
        INSERT INTO middleware_configs
            (plugin_key, enabled, phase, priority, failure_mode, scope, config)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(&req.plugin_key)
    .bind(req.enabled)
    .bind(&req.phase)
    .bind(req.priority)
    .bind(&req.failure_mode)
    .bind(&req.scope)
    .bind(&req.config)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to create middleware config: {}", e)))?
    .try_get("id")
    .map_err(|e| AppError::Internal(format!("Failed to get created id: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id.to_string(),
            "message": "Middleware config created"
        })),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMiddlewareConfigRequest {
    pub enabled: bool,
    pub phase: String,
    pub priority: i32,
    pub failure_mode: String,
    #[serde(default)]
    pub scope: serde_json::Value,
    #[serde(default)]
    pub config: serde_json::Value,
}

pub async fn update_middleware_config(
    State(state): State<AppState>,
    Path(config_id): Path<String>,
    Json(req): Json<UpdateMiddlewareConfigRequest>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::parse_str(&config_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let plugin_key: String = sqlx::query("SELECT plugin_key FROM middleware_configs WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to load config: {}", e)))?
        .ok_or_else(|| AppError::NotFound("Middleware config not found".to_string()))?
        .try_get("plugin_key")
        .map_err(|e| AppError::Internal(format!("Failed to get plugin_key: {}", e)))?;

    let temp_req = CreateMiddlewareConfigRequest {
        plugin_key,
        enabled: req.enabled,
        phase: req.phase,
        priority: req.priority,
        failure_mode: req.failure_mode,
        scope: req.scope,
        config: req.config,
    };
    validate_middleware_config(&temp_req)?;

    let result = sqlx::query(
        r#"
        UPDATE middleware_configs
        SET enabled = $1, phase = $2, priority = $3, failure_mode = $4, scope = $5, config = $6, updated_at = NOW()
        WHERE id = $7
        "#,
    )
    .bind(req.enabled)
    .bind(&temp_req.phase)
    .bind(req.priority)
    .bind(&temp_req.failure_mode)
    .bind(&temp_req.scope)
    .bind(&temp_req.config)
    .bind(id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to update middleware config: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Middleware config not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_middleware_config(
    State(state): State<AppState>,
    Path(config_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::parse_str(&config_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let result = sqlx::query("DELETE FROM middleware_configs WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete middleware config: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Middleware config not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_middleware_runs(
    State(state): State<AppState>,
) -> Result<Json<Vec<MiddlewareRunResponse>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id::text,
            request_id,
            plugin_key,
            phase,
            status,
            action,
            duration_ms,
            body_changed,
            metadata,
            error,
            created_at::text
        FROM middleware_runs
        ORDER BY created_at DESC
        LIMIT 200
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to list middleware runs: {}", e)))?;

    let runs = rows
        .into_iter()
        .map(|row| MiddlewareRunResponse {
            id: row.try_get("id").unwrap_or_default(),
            request_id: row.try_get("request_id").ok(),
            plugin_key: row.try_get("plugin_key").unwrap_or_default(),
            phase: row.try_get("phase").unwrap_or_default(),
            status: row.try_get("status").unwrap_or_default(),
            action: row.try_get("action").unwrap_or_default(),
            duration_ms: row.try_get("duration_ms").unwrap_or(0),
            body_changed: row.try_get("body_changed").unwrap_or(false),
            metadata: row.try_get("metadata").unwrap_or_else(|_| serde_json::json!({})),
            error: row.try_get("error").ok(),
            created_at: row.try_get("created_at").unwrap_or_default(),
        })
        .collect();

    Ok(Json(runs))
}

fn validate_middleware_config(req: &CreateMiddlewareConfigRequest) -> Result<(), AppError> {
    if req.plugin_key.is_empty() {
        return Err(AppError::BadRequest("plugin_key is required".to_string()));
    }

    let valid_phases = ["request.pre_governance", "request.pre_upstream", "response.pre_client", "inspector.pre_capture"];
    if !valid_phases.contains(&req.phase.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid phase '{}'. Valid phases: {}",
            req.phase,
            valid_phases.join(", ")
        )));
    }

    let valid_failure_modes = ["fail_open", "fail_closed"];
    if !valid_failure_modes.contains(&req.failure_mode.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid failure_mode '{}'. Valid modes: {}",
            req.failure_mode,
            valid_failure_modes.join(", ")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_req(plugin_key: &str, phase: &str, failure_mode: &str) -> CreateMiddlewareConfigRequest {
        CreateMiddlewareConfigRequest {
            plugin_key: plugin_key.to_string(),
            enabled: true,
            phase: phase.to_string(),
            priority: 100,
            failure_mode: failure_mode.to_string(),
            scope: serde_json::json!({}),
            config: serde_json::json!({}),
        }
    }

    #[test]
    fn validate_rejects_empty_plugin_key() {
        let req = make_req("", "request.pre_upstream", "fail_closed");
        let result = validate_middleware_config(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("plugin_key"));
    }

    #[test]
    fn validate_rejects_invalid_phase() {
        let req = make_req("token_reducer", "invalid.phase", "fail_closed");
        let result = validate_middleware_config(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("phase"));
    }

    #[test]
    fn validate_rejects_invalid_failure_mode() {
        let req = make_req("token_reducer", "request.pre_upstream", "fail_broken");
        let result = validate_middleware_config(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("failure_mode"));
    }

    #[test]
    fn validate_accepts_valid_config() {
        let req = make_req("token_reducer", "request.pre_upstream", "fail_open");
        let result = validate_middleware_config(&req);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_accepts_all_valid_phases() {
        for phase in ["request.pre_governance", "request.pre_upstream", "response.pre_client", "inspector.pre_capture"] {
            let req = make_req("token_reducer", phase, "fail_closed");
            assert!(validate_middleware_config(&req).is_ok(), "phase {} should be valid", phase);
        }
    }

    #[test]
    fn validate_accepts_both_failure_modes() {
        for mode in ["fail_open", "fail_closed"] {
            let req = make_req("token_reducer", "request.pre_upstream", mode);
            assert!(validate_middleware_config(&req).is_ok(), "mode {} should be valid", mode);
        }
    }
}
