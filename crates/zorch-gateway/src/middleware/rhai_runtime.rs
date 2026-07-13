use std::collections::HashMap;

use rhai::{Dynamic, Engine, EvalAltResult, Scope, AST};
use serde_json::Value;

use super::types::{MiddlewareContext, MiddlewareError, MiddlewareInput, MiddlewareOutput};

/// Runtime limits for a Rhai middleware script.
#[derive(Debug, Clone)]
pub struct RhaiRuntimeLimits {
    pub max_operations: u64,
    pub max_string_size: usize,
    pub max_array_size: usize,
    pub max_map_size: usize,
    pub max_call_stack_depth: usize,
}

impl Default for RhaiRuntimeLimits {
    fn default() -> Self {
        Self {
            max_operations: 1_000_000,
            max_string_size: 64 * 1024,
            max_array_size: 10_000,
            max_map_size: 10_000,
            max_call_stack_depth: 64,
        }
    }
}

impl RhaiRuntimeLimits {
    pub fn from_config(config: &Value) -> Self {
        let mut limits = Self::default();

        if let Some(v) = config.get("max_operations").and_then(|v| v.as_u64()) {
            limits.max_operations = v.max(1);
        }
        if let Some(v) = config.get("max_string_size").and_then(|v| v.as_u64()) {
            limits.max_string_size = v as usize;
        }
        if let Some(v) = config.get("max_array_size").and_then(|v| v.as_u64()) {
            limits.max_array_size = v as usize;
        }
        if let Some(v) = config.get("max_map_size").and_then(|v| v.as_u64()) {
            limits.max_map_size = v as usize;
        }
        if let Some(v) = config.get("max_call_stack_depth").and_then(|v| v.as_u64()) {
            limits.max_call_stack_depth = v as usize;
        }

        limits
    }
}

/// Extract the Rhai source string from the middleware config JSON.
pub fn extract_source(config: &Value) -> Result<String, MiddlewareError> {
    config
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| MiddlewareError::new("rhai", "missing or empty 'source' in config"))
}

/// Build a Rhai engine with the configured sandbox limits.
fn build_engine(limits: &RhaiRuntimeLimits) -> Engine {
    let mut engine = Engine::new();

    engine.set_max_string_size(limits.max_string_size);
    engine.set_max_array_size(limits.max_array_size);
    engine.set_max_map_size(limits.max_map_size);
    engine.set_max_call_levels(limits.max_call_stack_depth);

    let max_ops = limits.max_operations;
    engine.on_progress(move |counter| {
        if counter > max_ops {
            Some(format!("exceeded operation limit of {}", max_ops).into())
        } else {
            None
        }
    });

    engine
}

/// Compile a Rhai source string into an AST.
fn compile_source(engine: &Engine, source: &str) -> Result<AST, Box<EvalAltResult>> {
    let ast = engine.compile(source)?;
    Ok(ast)
}

/// Validate that a Rhai source string compiles and exposes a `run` function.
pub fn validate_script(source: &str) -> Result<(), String> {
    if source.trim().is_empty() {
        return Err("source is empty".to_string());
    }

    let engine = Engine::new();
    let ast = engine
        .compile(source)
        .map_err(|e| format!("compile error: {}", e))?;

    let has_run = ast
        .iter_functions()
        .any(|meta| meta.name == "run" && meta.params.len() == 3);
    if !has_run {
        return Err("missing required function 'run(ctx, input, config)'".to_string());
    }

    Ok(())
}

/// Execute a Rhai middleware script synchronously.
pub fn execute_script(
    ctx: &MiddlewareContext,
    input: MiddlewareInput,
    config: &Value,
) -> Result<MiddlewareOutput, MiddlewareError> {
    let source = extract_source(config)?;
    let limits = RhaiRuntimeLimits::from_config(config);

    let engine = build_engine(&limits);
    let ast = compile_source(&engine, &source)
        .map_err(|e| MiddlewareError::new("rhai", format!("compile error: {}", e)))?;

    let ctx_dynamic = rhai::serde::to_dynamic(serde_json::json!({
        "request_id": &ctx.request_id,
        "org_id": &ctx.org_id,
        "api_key_id": &ctx.api_key_id,
        "provider_id": &ctx.provider_id,
        "model_id": &ctx.model_id,
        "route": &ctx.route,
    }))
    .map_err(|e| MiddlewareError::new("rhai", format!("failed to serialize ctx: {}", e)))?;

    let input_dynamic = rhai::serde::to_dynamic(serde_json::json!({
        "body": &input.body,
        "headers": &input.headers,
    }))
    .map_err(|e| MiddlewareError::new("rhai", format!("failed to serialize input: {}", e)))?;

    let config_dynamic = rhai::serde::to_dynamic(config)
        .map_err(|e| MiddlewareError::new("rhai", format!("failed to serialize config: {}", e)))?;

    let mut scope = Scope::new();
    let result = engine
        .call_fn::<Dynamic>(
            &mut scope,
            &ast,
            "run",
            (ctx_dynamic, input_dynamic, config_dynamic),
        )
        .map_err(|e| MiddlewareError::new("rhai", format!("execution error: {}", e)))?;

    let result_value: Value = rhai::serde::from_dynamic(&result).map_err(|e| {
        MiddlewareError::new("rhai", format!("failed to deserialize result: {}", e))
    })?;

    parse_result(input, result_value)
}

/// Parse the Rhai return value into a `MiddlewareOutput`.
fn parse_result(
    input: MiddlewareInput,
    result: Value,
) -> Result<MiddlewareOutput, MiddlewareError> {
    let action = result
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MiddlewareError::new("rhai", "missing 'action' in script result"))?;

    let metadata = result
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));

    match action {
        "continue" => {
            let new_body = result.get("body").cloned();
            let new_headers: Option<HashMap<String, String>> = result
                .get("headers")
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            if let Some(body) = new_body {
                if body != input.body {
                    return Ok(MiddlewareOutput::continue_with(body, metadata));
                }
            }

            if let Some(headers) = new_headers {
                if !headers.is_empty() {
                    let mut output = MiddlewareOutput::continue_unchanged(metadata);
                    output.headers = Some(headers);
                    output.body_changed = false;
                    return Ok(output);
                }
            }

            Ok(MiddlewareOutput::continue_unchanged(metadata))
        }
        "block" => {
            let status_code = result
                .get("status_code")
                .and_then(|v| v.as_u64())
                .unwrap_or(403) as u16;
            let message = result
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Request blocked by middleware")
                .to_string();

            Ok(MiddlewareOutput::block(status_code, message, metadata))
        }
        _ => Err(MiddlewareError::new(
            "rhai",
            format!(
                "invalid action '{}' in script result; expected 'continue' or 'block'",
                action
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::types::{MiddlewareAction, MiddlewareContext};

    fn ctx() -> MiddlewareContext {
        MiddlewareContext {
            request_id: "req-1".to_string(),
            org_id: "org-1".to_string(),
            api_key_id: "key-1".to_string(),
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            route: "/v1/chat/completions".to_string(),
        }
    }

    fn base_config(source: &str) -> Value {
        serde_json::json!({
            "source": source,
            "max_operations": 100000,
            "max_string_size": 65536,
            "max_array_size": 10000,
            "max_map_size": 10000,
            "max_call_stack_depth": 64
        })
    }

    #[test]
    fn continue_unchanged() {
        let config =
            base_config(r#"fn run(ctx, input, config) { return #{ action: "continue" }; }"#);
        let input = MiddlewareInput::new(serde_json::json!({"messages": []}));
        let output = execute_script(&ctx(), input, &config).unwrap();
        assert_eq!(output.action, MiddlewareAction::Continue);
        assert!(!output.body_changed);
    }

    #[test]
    fn modify_body() {
        let config = base_config(
            r#"
            fn run(ctx, input, config) {
                let body = input.body;
                body.model = "gpt-4o-mini";
                return #{ action: "continue", body: body };
            }
        "#,
        );
        let input = MiddlewareInput::new(serde_json::json!({"model": "gpt-4"}));
        let output = execute_script(&ctx(), input.clone(), &config).unwrap();
        assert!(output.body_changed);
        assert_eq!(output.body.unwrap()["model"], "gpt-4o-mini");
    }

    #[test]
    fn block_request() {
        let config = base_config(
            r#"
            fn run(ctx, input, config) {
                return #{
                    action: "block",
                    status_code: 403,
                    message: "nope",
                    metadata: #{ reason: "test" }
                };
            }
        "#,
        );
        let input = MiddlewareInput::new(serde_json::json!({}));
        let output = execute_script(&ctx(), input, &config).unwrap();
        assert_eq!(output.action, MiddlewareAction::Block);
        assert_eq!(output.status_code, Some(403));
        assert_eq!(output.message, Some("nope".to_string()));
    }

    #[test]
    fn missing_source_fails() {
        let config = serde_json::json!({});
        let input = MiddlewareInput::new(serde_json::json!({}));
        assert!(execute_script(&ctx(), input, &config).is_err());
    }

    #[test]
    fn syntax_error_fails() {
        let config = base_config("fn run(ctx, input, config) { let x = ");
        let input = MiddlewareInput::new(serde_json::json!({}));
        assert!(execute_script(&ctx(), input, &config).is_err());
    }

    #[test]
    fn missing_run_function_fails() {
        let config = base_config("let x = 1;");
        let input = MiddlewareInput::new(serde_json::json!({}));
        assert!(execute_script(&ctx(), input, &config).is_err());
    }

    #[test]
    fn operation_limit_terminates_infinite_loop() {
        let config = base_config(
            r#"
            fn run(ctx, input, config) {
                let i = 0;
                while true {
                    i += 1;
                }
            }
        "#,
        );
        let input = MiddlewareInput::new(serde_json::json!({}));
        let result = execute_script(&ctx(), input, &config);
        assert!(result.is_err());
    }
}
