use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;

use crate::middleware::types::{MiddlewareContext, MiddlewareError, MiddlewareInput, MiddlewareOutput};

pub struct RequestBlockerPlugin;

#[derive(Debug)]
struct BlockPattern {
    name: String,
    regex: Regex,
    message: String,
}

impl RequestBlockerPlugin {
    fn parse_patterns(config: &Value) -> Result<Vec<BlockPattern>, MiddlewareError> {
        let patterns_arr = config
            .get("patterns")
            .and_then(|v| v.as_array())
            .ok_or_else(|| MiddlewareError::new("request_blocker", "missing 'patterns' array in config"))?;

        let mut patterns = Vec::new();
        for (i, p) in patterns_arr.iter().enumerate() {
            let name = p
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MiddlewareError::new("request_blocker", format!("pattern {} missing 'name'", i)))?;
            let regex_str = p
                .get("regex")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MiddlewareError::new("request_blocker", format!("pattern {} missing 'regex'", i)))?;
            let message = p
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Request contains blocked content.")
                .to_string();

            let regex = Regex::new(regex_str).map_err(|e| {
                MiddlewareError::new(
                    "request_blocker",
                    format!("invalid regex for pattern '{}': {}", name, e),
                )
            })?;

            patterns.push(BlockPattern {
                name: name.to_string(),
                regex,
                message,
            });
        }
        Ok(patterns)
    }
}

#[async_trait]
impl crate::middleware::types::MiddlewarePlugin for RequestBlockerPlugin {
    fn plugin_key(&self) -> &'static str {
        "request_blocker"
    }

    async fn run(
        &self,
        _ctx: &MiddlewareContext,
        input: MiddlewareInput,
        config: &Value,
    ) -> Result<MiddlewareOutput, MiddlewareError> {
        let patterns = Self::parse_patterns(config)?;

        let mut check_text = String::new();
        if let Some(messages) = input.body.get("messages") {
            if let Some(arr) = messages.as_array() {
                for msg in arr {
                    if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                        check_text.push_str(content);
                        check_text.push(' ');
                    }
                }
            }
        }

        for pattern in &patterns {
            if pattern.regex.is_match(&check_text) {
                return Ok(MiddlewareOutput::block(
                    403,
                    pattern.message.clone(),
                    serde_json::json!({
                        "blocked_pattern": pattern.name,
                    }),
                ));
            }
        }

        Ok(MiddlewareOutput::continue_unchanged(serde_json::json!({
            "checked_patterns": patterns.len(),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::types::{MiddlewareAction, MiddlewareContext, MiddlewarePlugin};

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
    async fn request_blocker_blocks_matching_request() {
        let plugin = RequestBlockerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "messages": [{"role": "user", "content": "My secret key is sk-abcdefghijklmnopqrstuvwxyz"}]
        }));
        let config = serde_json::json!({
            "patterns": [
                {
                    "name": "secret_key",
                    "regex": r"sk-[a-zA-Z0-9]{20,}",
                    "message": "Request appears to contain a secret key."
                }
            ]
        });

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        assert_eq!(out.action, MiddlewareAction::Block);
        assert_eq!(out.status_code, Some(403));
        assert_eq!(out.message, Some("Request appears to contain a secret key.".to_string()));
    }

    #[tokio::test]
    async fn request_blocker_allows_safe_request() {
        let plugin = RequestBlockerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "messages": [{"role": "user", "content": "Hello world"}]
        }));
        let config = serde_json::json!({
            "patterns": [
                {
                    "name": "secret_key",
                    "regex": r"sk-[a-zA-Z0-9]{20,}",
                    "message": "Request appears to contain a secret key."
                }
            ]
        });

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        assert_eq!(out.action, MiddlewareAction::Continue);
    }

    #[tokio::test]
    async fn request_blocker_does_not_echo_matched_value() {
        let plugin = RequestBlockerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "messages": [{"role": "user", "content": "sk-abc123"}]
        }));
        let config = serde_json::json!({
            "patterns": [
                {"name": "secret", "regex": r"sk-[a-zA-Z0-9]+", "message": "Blocked"}
            ]
        });

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let msg = out.message.unwrap();
        assert!(!msg.contains("sk-abc123"));
    }
}
