use async_trait::async_trait;
use serde_json::Value;

use crate::middleware::types::{MiddlewareContext, MiddlewareError, MiddlewareInput, MiddlewareOutput};

pub struct TokenReducerPlugin;

#[async_trait]
impl crate::middleware::types::MiddlewarePlugin for TokenReducerPlugin {
    fn plugin_key(&self) -> &'static str {
        "token_reducer"
    }

    async fn run(
        &self,
        _ctx: &MiddlewareContext,
        input: MiddlewareInput,
        config: &Value,
    ) -> Result<MiddlewareOutput, MiddlewareError> {
        let collapse_spaces = config.get("collapse_spaces").and_then(|v| v.as_bool()).unwrap_or(true);
        let trim_lines = config.get("trim_lines").and_then(|v| v.as_bool()).unwrap_or(true);
        let max_consecutive_newlines = config
            .get("max_consecutive_newlines")
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as usize;

        let mut body = input.body.clone();
        let before = serde_json::to_string(&body).unwrap_or_default().len();

        if let Some(messages) = body.get_mut("messages") {
            if let Some(arr) = messages.as_array_mut() {
                for msg in arr.iter_mut() {
                    if let Some(content) = msg.get_mut("content") {
                        if let Some(s) = content.as_str() {
                            let mut result = s.to_string();
                            if trim_lines {
                                result = result.lines().map(|l| l.trim()).collect::<Vec<_>>().join("\n");
                            }
                            if collapse_spaces {
                                result = result.split_whitespace().collect::<Vec<_>>().join(" ");
                            }
                            if max_consecutive_newlines > 0 {
                                let mut cleaned = String::new();
                                let mut newline_count = 0;
                                for c in result.chars() {
                                    if c == '\n' {
                                        newline_count += 1;
                                        if newline_count <= max_consecutive_newlines {
                                            cleaned.push(c);
                                        }
                                    } else {
                                        newline_count = 0;
                                        cleaned.push(c);
                                    }
                                }
                                result = cleaned;
                            }
                            *content = Value::String(result);
                        }
                    }
                }
            }
        }

        let after = serde_json::to_string(&body).unwrap_or_default().len();
        let bytes_saved = before.saturating_sub(after);

        let metadata = serde_json::json!({
            "before_bytes": before,
            "after_bytes": after,
            "bytes_saved": bytes_saved,
        });

        if bytes_saved > 0 {
            Ok(MiddlewareOutput::continue_with(body, metadata))
        } else {
            Ok(MiddlewareOutput::continue_unchanged(metadata))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::types::{MiddlewareAction, MiddlewareContext, MiddlewareInput, MiddlewarePlugin};

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

    #[tokio::test]
    async fn token_reducer_collapses_spaces() {
        let plugin = TokenReducerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hello   world"}
            ]
        }));
        let config = serde_json::json!({"collapse_spaces": true, "trim_lines": false, "max_consecutive_newlines": 2});

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        assert_eq!(out.action, MiddlewareAction::Continue);
        let body = out.body.unwrap();
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert_eq!(content, "Hello world");
    }

    #[tokio::test]
    async fn token_reducer_trims_lines() {
        let plugin = TokenReducerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "  Hello world  "}
            ]
        }));
        let config = serde_json::json!({"collapse_spaces": false, "trim_lines": true, "max_consecutive_newlines": 2});

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let body = out.body.unwrap();
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert_eq!(content, "Hello world");
    }

    #[tokio::test]
    async fn token_reducer_limits_newlines() {
        let plugin = TokenReducerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "line1\n\n\n\nline2"}
            ]
        }));
        let config = serde_json::json!({"collapse_spaces": false, "trim_lines": false, "max_consecutive_newlines": 2});

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let body = out.body.unwrap();
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert_eq!(content, "line1\n\nline2");
    }

    #[tokio::test]
    async fn token_reducer_leaves_non_string_content() {
        let plugin = TokenReducerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": [{"type": "image_url", "image_url": {"url": "data:image/png;base64,abc"}}]}
            ]
        }));
        let config = serde_json::json!({"collapse_spaces": true, "trim_lines": true, "max_consecutive_newlines": 2});

        let input_body = input.body.clone();
        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let body = out.body.unwrap_or(input_body);
        assert!(body["messages"][0]["content"].is_array());
    }

    #[tokio::test]
    async fn token_reducer_leaves_model_unchanged() {
        let plugin = TokenReducerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "hello"}
            ]
        }));
        let config = serde_json::json!({"collapse_spaces": true, "trim_lines": true, "max_consecutive_newlines": 2});

        let input_body = input.body.clone();
        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let body = out.body.unwrap_or(input_body);
        assert_eq!(body["model"].as_str().unwrap(), "gpt-4");
    }
}
