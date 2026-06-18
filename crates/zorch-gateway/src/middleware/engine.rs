use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;

use super::audit::{MiddlewareAudit, MiddlewareRunRecord};
use super::types::{
    FailureMode, MiddlewareAction, MiddlewareContext, MiddlewareError, MiddlewareInput,
    MiddlewarePhase, MiddlewarePlugin, MiddlewareScope,
};

/// Loaded configuration for a single middleware plugin instance.
#[derive(Debug, Clone)]
pub struct MiddlewareConfig {
    pub id: String,
    pub plugin_key: String,
    pub enabled: bool,
    pub phase: MiddlewarePhase,
    pub priority: i32,
    pub failure_mode: FailureMode,
    pub scope: MiddlewareScope,
    pub config: Value,
}

/// Execution engine that loads configs from DB and runs middleware phases.
pub struct MiddlewareEngine {
    pool: PgPool,
    plugins: HashMap<String, Arc<dyn MiddlewarePlugin>>,
    audit: MiddlewareAudit,
}

impl MiddlewareEngine {
    pub fn new(pool: PgPool, plugins: Vec<Arc<dyn MiddlewarePlugin>>) -> Self {
        let mut plugin_map = HashMap::new();
        for p in plugins {
            plugin_map.insert(p.plugin_key().to_string(), p);
        }
        Self {
            pool,
            plugins: plugin_map,
            audit: MiddlewareAudit::new(),
        }
    }

    pub fn with_audit(mut self, audit: MiddlewareAudit) -> Self {
        self.audit = audit;
        self
    }

    /// Load all enabled middleware configs for a phase, sorted by priority.
    pub async fn load_phase_configs(
        &self,
        phase: MiddlewarePhase,
    ) -> Result<Vec<MiddlewareConfig>, sqlx::Error> {
        let phase_str = phase.as_str();
        let rows = sqlx::query(
            r#"
            SELECT
                id::text as id,
                plugin_key,
                enabled,
                phase,
                priority,
                failure_mode,
                scope,
                config
            FROM middleware_configs
            WHERE enabled = true AND phase = $1
            ORDER BY priority ASC, plugin_key ASC, id ASC
            "#,
        )
        .bind(phase_str)
        .fetch_all(&self.pool)
        .await?;

        let mut configs = Vec::with_capacity(rows.len());
        for row in rows {
            let scope_json: Value = row.try_get("scope").unwrap_or_else(|_| Value::Object(Default::default()));
            let scope: MiddlewareScope = serde_json::from_value(scope_json).unwrap_or_default();

            configs.push(MiddlewareConfig {
                id: row.try_get("id").unwrap_or_default(),
                plugin_key: row.try_get("plugin_key").unwrap_or_default(),
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
                scope,
                config: row.try_get("config").unwrap_or_else(|_| Value::Object(Default::default())),
            });
        }

        Ok(configs)
    }

    /// Run all middleware for a given phase against the request.
    pub async fn run_phase(
        &self,
        phase: MiddlewarePhase,
        ctx: &MiddlewareContext,
        input: MiddlewareInput,
    ) -> Result<MiddlewareInput, MiddlewareError> {
        let configs = self
            .load_phase_configs(phase)
            .await
            .map_err(|e| MiddlewareError::new("engine", format!("failed to load configs: {}", e)))?;

        let mut current_input = input;

        for config in configs {
            if !config.enabled {
                continue;
            }

            if !config.scope.matches(
                &ctx.org_id,
                &ctx.api_key_id,
                &ctx.provider_id,
                &ctx.model_id,
                &ctx.route,
            ) {
                continue;
            }

            let plugin = match self.plugins.get(&config.plugin_key) {
                Some(p) => p.clone(),
                None => {
                    tracing::warn!("middleware plugin '{}' not registered", config.plugin_key);
                    continue;
                }
            };

            let start = std::time::Instant::now();
            let result = plugin.run(ctx, current_input.clone(), &config.config).await;
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
                                plugin_key: &config.plugin_key,
                                phase: phase.as_str(),
                                status,
                                action: if action == MiddlewareAction::Block { "block" } else { "continue" },
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
                        return Err(MiddlewareError::new(
                            &config.plugin_key,
                            output.message.unwrap_or_else(|| "Request blocked by middleware".to_string()),
                        ));
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
                                plugin_key: &config.plugin_key,
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
                                &config.plugin_key,
                                format!("Middleware error (fail_closed): {}", err.message),
                            ));
                        }
                        FailureMode::FailOpen => {
                            tracing::warn!(
                                plugin = %config.plugin_key,
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
    use crate::middleware::plugins::{
        request_blocker::RequestBlockerPlugin, sensitive_marker::SensitiveMarkerPlugin,
        token_reducer::TokenReducerPlugin,
    };
    use crate::middleware::types::MiddlewarePlugin;

    #[test]
    fn test_middleware_config_scope_global() {
        let scope = MiddlewareScope::default();
        assert!(scope.is_global());
    }

    #[tokio::test]
    async fn test_token_reducer_then_sensitive_marker_pipeline() {
        let ctx = MiddlewareContext {
            request_id: "req-1".to_string(),
            org_id: "org-1".to_string(),
            api_key_id: "key-1".to_string(),
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            route: "/v1/chat/completions".to_string(),
        };

        let input = MiddlewareInput::new(serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "My email is john@company.com   and I need help"}
            ]
        }));

        let reducer = TokenReducerPlugin;
        let marker = SensitiveMarkerPlugin;

        let reducer_config = serde_json::json!({
            "collapse_spaces": true,
            "trim_lines": true,
            "max_consecutive_newlines": 2
        });

        let marker_config = serde_json::json!({
            "patterns": [
                {
                    "name": "company_email",
                    "regex": r"[a-zA-Z0-9._%+-]+@company\.com",
                    "replacement": "[COMPANY_EMAIL]"
                }
            ]
        });

        let after_reducer = reducer.run(&ctx, input, &reducer_config).await.unwrap();
        let after_reducer_body = after_reducer.body.unwrap();

        let after_marker = marker.run(&ctx, MiddlewareInput::new(after_reducer_body), &marker_config).await.unwrap();
        let final_body = after_marker.body.unwrap();

        let content = final_body["messages"][0]["content"].as_str().unwrap();
        assert_eq!(content, "My email is [COMPANY_EMAIL] and I need help");
    }

    #[tokio::test]
    async fn test_sensitive_marker_blocks_request_with_secret() {
        let ctx = MiddlewareContext {
            request_id: "req-1".to_string(),
            org_id: "org-1".to_string(),
            api_key_id: "key-1".to_string(),
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            route: "/v1/chat/completions".to_string(),
        };

        let input = MiddlewareInput::new(serde_json::json!({
            "messages": [
                {"role": "user", "content": "My secret key is sk-abcdefghijklmnopqrstuvwxyz"}
            ]
        }));

        let blocker = RequestBlockerPlugin;
        let blocker_config = serde_json::json!({
            "patterns": [
                {
                    "name": "secret_key",
                    "regex": r"sk-[a-zA-Z0-9]{20,}",
                    "message": "Request appears to contain a secret key."
                }
            ]
        });

        let result = blocker.run(&ctx, input, &blocker_config).await.unwrap();
        assert_eq!(result.action, MiddlewareAction::Block);
        assert_eq!(result.status_code, Some(403));
    }
}
