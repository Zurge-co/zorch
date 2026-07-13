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
pub struct MiddlewareConfigResponse {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub phase: String,
    pub priority: i32,
    pub failure_mode: String,
    pub config: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiddlewareRunResponse {
    pub id: String,
    pub request_id: Option<String>,
    pub middleware_config_id: Option<String>,
    pub phase: String,
    pub status: String,
    pub action: String,
    pub duration_ms: i32,
    pub body_changed: bool,
    pub metadata: serde_json::Value,
    pub error: Option<String>,
    pub created_at: String,
}

pub async fn get_middleware_configs(
    State(state): State<AppState>,
) -> Result<Json<Vec<MiddlewareConfigResponse>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id::text,
            name,
            enabled,
            phase,
            priority,
            failure_mode,
            config,
            created_at::text,
            updated_at::text
        FROM middleware_configs
        ORDER BY priority ASC, name ASC
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to list middleware configs: {}", e)))?;

    let configs = rows
        .into_iter()
        .map(|row| MiddlewareConfigResponse {
            id: row.try_get("id").unwrap_or_default(),
            name: row.try_get("name").unwrap_or_default(),
            enabled: row.try_get("enabled").unwrap_or(true),
            phase: row.try_get("phase").unwrap_or_default(),
            priority: row.try_get("priority").unwrap_or(100),
            failure_mode: row.try_get("failure_mode").unwrap_or_default(),
            config: row
                .try_get("config")
                .unwrap_or_else(|_| serde_json::json!({})),
            created_at: row.try_get("created_at").unwrap_or_default(),
            updated_at: row.try_get("updated_at").unwrap_or_default(),
        })
        .collect();

    Ok(Json(configs))
}

pub async fn get_middleware_config(
    State(state): State<AppState>,
    Path(config_id): Path<String>,
) -> Result<Json<MiddlewareConfigResponse>, AppError> {
    let id = Uuid::parse_str(&config_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let row = sqlx::query(
        r#"
        SELECT
            id::text,
            name,
            enabled,
            phase,
            priority,
            failure_mode,
            config,
            created_at::text,
            updated_at::text
        FROM middleware_configs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load middleware config: {}", e)))?
    .ok_or_else(|| AppError::NotFound("Middleware config not found".to_string()))?;

    Ok(Json(MiddlewareConfigResponse {
        id: row.try_get("id").unwrap_or_default(),
        name: row.try_get("name").unwrap_or_default(),
        enabled: row.try_get("enabled").unwrap_or(true),
        phase: row.try_get("phase").unwrap_or_default(),
        priority: row.try_get("priority").unwrap_or(100),
        failure_mode: row.try_get("failure_mode").unwrap_or_default(),
        config: row
            .try_get("config")
            .unwrap_or_else(|_| serde_json::json!({})),
        created_at: row.try_get("created_at").unwrap_or_default(),
        updated_at: row.try_get("updated_at").unwrap_or_default(),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMiddlewareConfigRequest {
    pub name: String,
    pub enabled: bool,
    pub phase: String,
    pub priority: i32,
    pub failure_mode: String,
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
            (name, enabled, phase, priority, failure_mode, config)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(&req.name)
    .bind(req.enabled)
    .bind(&req.phase)
    .bind(req.priority)
    .bind(&req.failure_mode)
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
    pub name: String,
    pub enabled: bool,
    pub phase: String,
    pub priority: i32,
    pub failure_mode: String,
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

    let temp_req = CreateMiddlewareConfigRequest {
        name: req.name,
        enabled: req.enabled,
        phase: req.phase,
        priority: req.priority,
        failure_mode: req.failure_mode,
        config: req.config,
    };
    validate_middleware_config(&temp_req)?;

    let result = sqlx::query(
        r#"
        UPDATE middleware_configs
        SET name = $1, enabled = $2, phase = $3, priority = $4, failure_mode = $5, config = $6, updated_at = NOW()
        WHERE id = $7
        "#,
    )
    .bind(&temp_req.name)
    .bind(req.enabled)
    .bind(&temp_req.phase)
    .bind(req.priority)
    .bind(&temp_req.failure_mode)
    .bind(&temp_req.config)
    .bind(id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to update middleware config: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Middleware config not found".to_string(),
        ));
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
        return Err(AppError::NotFound(
            "Middleware config not found".to_string(),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateMiddlewareScriptRequest {
    pub source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateMiddlewareScriptResponse {
    pub valid: bool,
    pub error: Option<String>,
}

pub async fn validate_middleware_script(
    State(_state): State<AppState>,
    Json(req): Json<ValidateMiddlewareScriptRequest>,
) -> Result<Json<ValidateMiddlewareScriptResponse>, AppError> {
    match zorch_gateway::middleware::rhai_runtime::validate_script(&req.source) {
        Ok(()) => Ok(Json(ValidateMiddlewareScriptResponse {
            valid: true,
            error: None,
        })),
        Err(e) => Ok(Json(ValidateMiddlewareScriptResponse {
            valid: false,
            error: Some(e),
        })),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMiddlewareScriptRequest {
    pub source: String,
    #[serde(default)]
    pub config: RunMiddlewareRuntimeConfig,
    pub context: RunMiddlewareContext,
    pub input: RunMiddlewareInput,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunMiddlewareRuntimeConfig {
    #[serde(default)]
    pub max_operations: Option<u64>,
    #[serde(default)]
    pub max_string_size: Option<usize>,
    #[serde(default)]
    pub max_array_size: Option<usize>,
    #[serde(default)]
    pub max_map_size: Option<usize>,
    #[serde(default)]
    pub max_call_stack_depth: Option<usize>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMiddlewareContext {
    pub request_id: String,
    pub org_id: String,
    pub api_key_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub route: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMiddlewareInput {
    pub body: serde_json::Value,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMiddlewareScriptResponse {
    pub success: bool,
    pub action: Option<String>,
    pub body: Option<serde_json::Value>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub metadata: Option<serde_json::Value>,
    pub body_changed: bool,
    pub message: Option<String>,
    pub status_code: Option<u16>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

async fn run_middleware_script_inner(
    req: RunMiddlewareScriptRequest,
) -> RunMiddlewareScriptResponse {
    let start = std::time::Instant::now();

    let mut full_config = serde_json::Map::new();
    full_config.insert("source".to_string(), serde_json::Value::String(req.source));

    if let Some(v) = req.config.max_operations {
        full_config.insert("max_operations".to_string(), serde_json::Value::Number(v.into()));
    }
    if let Some(v) = req.config.max_string_size {
        full_config.insert(
            "max_string_size".to_string(),
            serde_json::Value::Number(v.into()),
        );
    }
    if let Some(v) = req.config.max_array_size {
        full_config.insert(
            "max_array_size".to_string(),
            serde_json::Value::Number(v.into()),
        );
    }
    if let Some(v) = req.config.max_map_size {
        full_config.insert("max_map_size".to_string(), serde_json::Value::Number(v.into()));
    }
    if let Some(v) = req.config.max_call_stack_depth {
        full_config.insert(
            "max_call_stack_depth".to_string(),
            serde_json::Value::Number(v.into()),
        );
    }
    for (key, value) in req.config.extra {
        full_config.insert(key, value);
    }
    let config_value = serde_json::Value::Object(full_config);

    let ctx = zorch_gateway::MiddlewareContext {
        request_id: req.context.request_id,
        org_id: req.context.org_id,
        api_key_id: req.context.api_key_id,
        provider_id: req.context.provider_id,
        model_id: req.context.model_id,
        route: req.context.route,
    };

    let input = zorch_gateway::MiddlewareInput {
        body: req.input.body,
        headers: req.input.headers,
    };

    let result = tokio::task::spawn_blocking(move || {
        zorch_gateway::middleware::rhai_runtime::execute_script(&ctx, input, &config_value)
    })
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(output)) => {
            let action = match output.action {
                zorch_gateway::MiddlewareAction::Continue => "continue".to_string(),
                zorch_gateway::MiddlewareAction::Block => "block".to_string(),
            };
            RunMiddlewareScriptResponse {
                success: true,
                action: Some(action),
                body: output.body,
                headers: output.headers,
                metadata: Some(output.metadata),
                body_changed: output.body_changed,
                message: output.message,
                status_code: output.status_code,
                error: None,
                duration_ms,
            }
        }
        Ok(Err(err)) => RunMiddlewareScriptResponse {
            success: false,
            action: None,
            body: None,
            headers: None,
            metadata: None,
            body_changed: false,
            message: None,
            status_code: None,
            error: Some(err.message),
            duration_ms,
        },
        Err(join_err) => RunMiddlewareScriptResponse {
            success: false,
            action: None,
            body: None,
            headers: None,
            metadata: None,
            body_changed: false,
            message: None,
            status_code: None,
            error: Some(format!("execution task failed: {}", join_err)),
            duration_ms,
        },
    }
}

pub async fn run_middleware_script(
    State(_state): State<AppState>,
    Json(req): Json<RunMiddlewareScriptRequest>,
) -> Result<Json<RunMiddlewareScriptResponse>, AppError> {
    let response = run_middleware_script_inner(req).await;
    Ok(Json(response))
}

pub async fn get_middleware_runs(
    State(state): State<AppState>,
) -> Result<Json<Vec<MiddlewareRunResponse>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id::text,
            request_id,
            middleware_config_id::text,
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
            middleware_config_id: row.try_get("middleware_config_id").ok(),
            phase: row.try_get("phase").unwrap_or_default(),
            status: row.try_get("status").unwrap_or_default(),
            action: row.try_get("action").unwrap_or_default(),
            duration_ms: row.try_get("duration_ms").unwrap_or(0),
            body_changed: row.try_get("body_changed").unwrap_or(false),
            metadata: row
                .try_get("metadata")
                .unwrap_or_else(|_| serde_json::json!({})),
            error: row.try_get("error").ok(),
            created_at: row.try_get("created_at").unwrap_or_default(),
        })
        .collect();

    Ok(Json(runs))
}

fn validate_middleware_config(req: &CreateMiddlewareConfigRequest) -> Result<(), AppError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }

    let valid_phases = ["request.pre_governance", "request.pre_upstream"];
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

    let source = req
        .config
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if source.is_empty() {
        return Err(AppError::BadRequest("config.source is required".to_string()));
    }

    if let Err(e) = zorch_gateway::middleware::rhai_runtime::validate_script(source) {
        return Err(AppError::BadRequest(format!("Invalid Rhai script: {}", e)));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_req(
        name: &str,
        phase: &str,
        failure_mode: &str,
        source: &str,
    ) -> CreateMiddlewareConfigRequest {
        CreateMiddlewareConfigRequest {
            name: name.to_string(),
            enabled: true,
            phase: phase.to_string(),
            priority: 100,
            failure_mode: failure_mode.to_string(),
            config: serde_json::json!({"source": source}),
        }
    }

    #[test]
    fn validate_rejects_empty_name() {
        let req = make_req(
            "",
            "request.pre_upstream",
            "fail_closed",
            "fn run(ctx, input, config) {}",
        );
        let result = validate_middleware_config(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn validate_rejects_invalid_phase() {
        let req = make_req(
            "test",
            "invalid.phase",
            "fail_closed",
            "fn run(ctx, input, config) {}",
        );
        let result = validate_middleware_config(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("phase"));
    }

    #[test]
    fn validate_rejects_invalid_failure_mode() {
        let req = make_req(
            "test",
            "request.pre_upstream",
            "fail_broken",
            "fn run(ctx, input, config) {}",
        );
        let result = validate_middleware_config(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("failure_mode"));
    }

    #[test]
    fn validate_rejects_missing_source() {
        let req = CreateMiddlewareConfigRequest {
            name: "test".to_string(),
            enabled: true,
            phase: "request.pre_upstream".to_string(),
            priority: 100,
            failure_mode: "fail_open".to_string(),
            config: serde_json::json!({}),
        };
        let result = validate_middleware_config(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("source"));
    }

    #[test]
    fn validate_accepts_valid_config() {
        let req = make_req(
            "test",
            "request.pre_upstream",
            "fail_open",
            "fn run(ctx, input, config) {}",
        );
        let result = validate_middleware_config(&req);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_accepts_both_phases() {
        for phase in ["request.pre_governance", "request.pre_upstream"] {
            let req = make_req(
                "test",
                phase,
                "fail_closed",
                "fn run(ctx, input, config) {}",
            );
            assert!(
                validate_middleware_config(&req).is_ok(),
                "phase {} should be valid",
                phase
            );
        }
    }

    #[test]
    fn validate_accepts_both_failure_modes() {
        for mode in ["fail_open", "fail_closed"] {
            let req = make_req(
                "test",
                "request.pre_upstream",
                mode,
                "fn run(ctx, input, config) {}",
            );
            assert!(
                validate_middleware_config(&req).is_ok(),
                "mode {} should be valid",
                mode
            );
        }
    }

    fn make_run_req(source: &str) -> RunMiddlewareScriptRequest {
        RunMiddlewareScriptRequest {
            source: source.to_string(),
            config: RunMiddlewareRuntimeConfig {
                max_operations: Some(100_000),
                max_string_size: Some(65_536),
                max_array_size: Some(10_000),
                max_map_size: Some(10_000),
                max_call_stack_depth: Some(64),
                extra: serde_json::Map::new(),
            },
            context: RunMiddlewareContext {
                request_id: "req-test-001".to_string(),
                org_id: "org-demo".to_string(),
                api_key_id: "key-demo".to_string(),
                provider_id: "openai".to_string(),
                model_id: "gpt-4o".to_string(),
                route: "/v1/chat/completions".to_string(),
            },
            input: RunMiddlewareInput {
                body: serde_json::json!({
                    "model": "gpt-4o",
                    "messages": [
                        { "role": "system", "content": "You are helpful." },
                        { "role": "user", "content": "Hello" }
                    ],
                    "temperature": 0.7
                }),
                headers: std::collections::HashMap::from([
                    (
                        "Authorization".to_string(),
                        "Bearer sk-test".to_string(),
                    ),
                    ("Content-Type".to_string(), "application/json".to_string()),
                ]),
            },
        }
    }

    #[tokio::test]
    async fn run_script_continues_unchanged() {
        let req = make_run_req(r#"fn run(ctx, input, config) { return #{ action: "continue", metadata: #{ safe: true } }; }"#);
        let res = run_middleware_script_inner(req).await;
        assert!(res.success);
        assert_eq!(res.action.as_deref(), Some("continue"));
        assert!(!res.body_changed);
        assert_eq!(res.metadata.unwrap()["safe"], true);
    }

    #[tokio::test]
    async fn run_script_modifies_body() {
        let req = make_run_req(r#"
            fn run(ctx, input, config) {
                let body = input.body;
                body.model = "gpt-4o-mini";
                return #{ action: "continue", body: body, metadata: #{ overridden: true } };
            }
        "#);
        let res = run_middleware_script_inner(req).await;
        assert!(res.success);
        assert_eq!(res.action.as_deref(), Some("continue"));
        assert!(res.body_changed);
        assert_eq!(res.body.unwrap()["model"], "gpt-4o-mini");
    }

    #[tokio::test]
    async fn run_script_blocks_request() {
        let req = make_run_req(r#"
            fn run(ctx, input, config) {
                return #{ action: "block", status_code: 403, message: "nope", metadata: #{ reason: "test" } };
            }
        "#);
        let res = run_middleware_script_inner(req).await;
        assert!(res.success);
        assert_eq!(res.action.as_deref(), Some("block"));
        assert_eq!(res.status_code, Some(403));
        assert_eq!(res.message, Some("nope".to_string()));
    }

    #[tokio::test]
    async fn run_script_reports_syntax_error() {
        let req = make_run_req("fn run(ctx, input, config) { let x = ");
        let res = run_middleware_script_inner(req).await;
        assert!(!res.success);
        assert!(res.error.unwrap().contains("compile error"));
    }

    #[tokio::test]
    async fn run_script_reports_missing_source() {
        let mut req = make_run_req("");
        req.source = "".to_string();
        let res = run_middleware_script_inner(req).await;
        assert!(!res.success);
        assert!(res.error.unwrap().contains("missing or empty"));
    }
}
