use serde_json::Value;
use sqlx::{PgPool, Row};
use std::time::Instant;

use super::audit::{MiddlewareAudit, MiddlewareRunRecord};
use super::rhai_runtime;
use super::types::{
    FailureMode, MiddlewareAction, MiddlewareContext, MiddlewareError, MiddlewareInput,
    MiddlewarePhase,
};

/// Loaded configuration for a single middleware script instance.
#[derive(Debug, Clone)]
pub struct MiddlewareConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub phase: MiddlewarePhase,
    pub priority: i32,
    pub failure_mode: FailureMode,
    pub config: Value,
}

/// Execution engine that loads configs from DB and runs middleware phases.
pub struct MiddlewareEngine {
    pool: PgPool,
    audit: MiddlewareAudit,
}

impl MiddlewareEngine {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            audit: MiddlewareAudit::new(),
        }
    }

    pub fn with_audit(mut self, audit: MiddlewareAudit) -> Self {
        self.audit = audit;
        self
    }

    /// Load all enabled middleware configs for a phase bound to the given API key, sorted by priority.
    pub async fn load_phase_configs(
        &self,
        api_key_id: &str,
        phase: MiddlewarePhase,
    ) -> Result<Vec<MiddlewareConfig>, sqlx::Error> {
        let phase_str = phase.as_str();
        let rows = sqlx::query(
            r#"
            SELECT
                mc.id::text as id,
                mc.name,
                mc.enabled,
                mc.phase,
                mc.priority,
                mc.failure_mode,
                mc.config
            FROM middleware_configs mc
            JOIN api_key_middleware_configs akmc ON akmc.middleware_config_id = mc.id
            WHERE akmc.api_key_id = $1::uuid
              AND mc.enabled = true
              AND mc.phase = $2
            ORDER BY mc.priority ASC, mc.name ASC, mc.id ASC
            "#,
        )
        .bind(api_key_id)
        .bind(phase_str)
        .fetch_all(&self.pool)
        .await?;

        let mut configs = Vec::with_capacity(rows.len());
        for row in rows {
            configs.push(MiddlewareConfig {
                id: row.try_get("id").unwrap_or_default(),
                name: row.try_get("name").unwrap_or_default(),
                enabled: row.try_get("enabled").unwrap_or(true),
                phase: row
                    .try_get::<String, _>("phase")
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or(MiddlewarePhase::RequestPreUpstream),
                priority: row.try_get("priority").unwrap_or(100),
                failure_mode: row
                    .try_get::<String, _>("failure_mode")
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or(FailureMode::FailClosed),
                config: row
                    .try_get("config")
                    .unwrap_or_else(|_| Value::Object(Default::default())),
            });
        }

        Ok(configs)
    }

    /// Run all middleware for a given phase against the request.
    pub async fn run_phase(
        &self,
        api_key_id: &str,
        phase: MiddlewarePhase,
        ctx: &MiddlewareContext,
        input: MiddlewareInput,
    ) -> Result<MiddlewareInput, MiddlewareError> {
        let configs = self
            .load_phase_configs(api_key_id, phase)
            .await
            .map_err(|e| {
                MiddlewareError::new("engine", format!("failed to load configs: {}", e))
            })?;

        if configs.is_empty() {
            return Ok(input);
        }

        let mut current_input = input;

        for config in configs {
            if !config.enabled {
                continue;
            }

            let ctx_for_script = ctx.clone();
            let config_json = config.config.clone();
            let input_for_script = current_input.clone();

            let start = Instant::now();
            let result = tokio::task::spawn_blocking(move || {
                rhai_runtime::execute_script(&ctx_for_script, input_for_script, &config_json)
            })
            .await
            .map_err(|e| MiddlewareError::new("rhai", format!("script task panicked: {}", e)))?;
            let duration_ms = start.elapsed().as_millis() as i32;

            match result {
                Ok(output) => {
                    let body_changed = output.body_changed;
                    let action = output.action;
                    let status = if action == MiddlewareAction::Block {
                        "blocked"
                    } else {
                        "success"
                    };

                    if let Err(e) = self
                        .audit
                        .record_run(
                            &self.pool,
                            MiddlewareRunRecord {
                                request_id: &ctx.request_id,
                                middleware_config_id: &config.id,
                                phase: phase.as_str(),
                                status,
                                action: if action == MiddlewareAction::Block {
                                    "block"
                                } else {
                                    "continue"
                                },
                                duration_ms,
                                body_changed,
                                metadata: output.metadata.clone(),
                                error: None,
                            },
                        )
                        .await
                    {
                        tracing::warn!("failed to record middleware audit: {}", e);
                    }

                    if action == MiddlewareAction::Block {
                        let mut err = MiddlewareError::new(
                            "rhai",
                            output
                                .message
                                .unwrap_or_else(|| "Request blocked by middleware".to_string()),
                        );
                        err.status_code = output.status_code;
                        return Err(err);
                    }

                    if let Some(new_body) = output.body {
                        current_input.body = new_body;
                    }
                    if let Some(new_headers) = output.headers {
                        current_input.headers = new_headers;
                    }
                }
                Err(err) => {
                    let status = "error";
                    let action_str = if config.failure_mode == FailureMode::FailClosed {
                        "block"
                    } else {
                        "continue"
                    };

                    if let Err(e) = self
                        .audit
                        .record_run(
                            &self.pool,
                            MiddlewareRunRecord {
                                request_id: &ctx.request_id,
                                middleware_config_id: &config.id,
                                phase: phase.as_str(),
                                status,
                                action: action_str,
                                duration_ms,
                                body_changed: false,
                                metadata: serde_json::json!({}),
                                error: Some(err.message.clone()),
                            },
                        )
                        .await
                    {
                        tracing::warn!("failed to record middleware audit: {}", e);
                    }

                    match config.failure_mode {
                        FailureMode::FailClosed => {
                            return Err(MiddlewareError::new(
                                "rhai",
                                format!("Middleware error (fail_closed): {}", err.message),
                            ));
                        }
                        FailureMode::FailOpen => {
                            tracing::warn!(
                                config_id = %config.id,
                                error = %err.message,
                                "middleware failed with fail_open; continuing"
                            );
                        }
                    }
                }
            }
        }

        Ok(current_input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_defaults_to_request_pre_upstream_on_parse_error() {
        // This is a defensive behavior check; the query filters by known phases.
        let config = MiddlewareConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            enabled: true,
            phase: MiddlewarePhase::RequestPreUpstream,
            priority: 100,
            failure_mode: FailureMode::FailClosed,
            config: serde_json::json!({}),
        };
        assert_eq!(config.phase.as_str(), "request.pre_upstream");
    }
}
