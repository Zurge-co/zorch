use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;

use crate::middleware::types::{MiddlewareContext, MiddlewareError, MiddlewareInput, MiddlewareOutput};

pub struct SensitiveMarkerPlugin;

#[derive(Debug)]
struct Pattern {
    name: String,
    regex: Regex,
    replacement: String,
}

impl SensitiveMarkerPlugin {
    fn parse_patterns(config: &Value) -> Result<Vec<Pattern>, MiddlewareError> {
        let patterns_arr = config
            .get("patterns")
            .and_then(|v| v.as_array())
            .ok_or_else(|| MiddlewareError::new("sensitive_marker", "missing 'patterns' array in config"))?;

        let mut patterns = Vec::new();
        for (i, p) in patterns_arr.iter().enumerate() {
            let name = p
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MiddlewareError::new("sensitive_marker", format!("pattern {} missing 'name'", i)))?;
            let regex_str = p
                .get("regex")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MiddlewareError::new("sensitive_marker", format!("pattern {} missing 'regex'", i)))?;
            let replacement = p
                .get("replacement")
                .and_then(|v| v.as_str())
                .unwrap_or("[REDACTED]")
                .to_string();

            let regex = Regex::new(regex_str).map_err(|e| {
                MiddlewareError::new(
                    "sensitive_marker",
                    format!("invalid regex for pattern '{}': {}", name, e),
                )
            })?;

            patterns.push(Pattern {
                name: name.to_string(),
                regex,
                replacement,
            });
        }
        Ok(patterns)
    }
}

#[async_trait]
impl crate::middleware::types::MiddlewarePlugin for SensitiveMarkerPlugin {
    fn plugin_key(&self) -> &'static str {
        "sensitive_marker"
    }

    async fn run(
        &self,
        _ctx: &MiddlewareContext,
        input: MiddlewareInput,
        config: &Value,
    ) -> Result<MiddlewareOutput, MiddlewareError> {
        let patterns = Self::parse_patterns(config)?;
        let mut body = input.body.clone();
        let mut redaction_counts: serde_json::Map<String, Value> = serde_json::Map::new();
        let mut total_redactions = 0;

        if let Some(messages) = body.get_mut("messages") {
            if let Some(arr) = messages.as_array_mut() {
                for msg in arr.iter_mut() {
                    if let Some(content) = msg.get_mut("content") {
                        if let Some(s) = content.as_str() {
                            let mut result = s.to_string();
                            for pattern in &patterns {
                                let count = pattern.regex.find_iter(&result).count();
                                if count > 0 {
                                    result = pattern.regex.replace_all(&result, &pattern.replacement).to_string();
                                    *redaction_counts
                                        .entry(pattern.name.clone())
                                        .or_insert_with(|| Value::Number(0.into())) =
                                        Value::Number((count as u64).into());
                                    total_redactions += count;
                                }
                            }
                            *content = Value::String(result);
                        }
                    }
                }
            }
        }

        let metadata = serde_json::json!({
            "redactions": total_redactions,
            "by_pattern": redaction_counts,
        });

        if total_redactions > 0 {
            Ok(MiddlewareOutput::continue_with(body, metadata))
        } else {
            Ok(MiddlewareOutput::continue_unchanged(metadata))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::types::MiddlewarePlugin;

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
    async fn sensitive_marker_replaces_patterns() {
        let plugin = SensitiveMarkerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "My email is john@company.com and jane@company.com"}
            ]
        }));
        let config = serde_json::json!({
            "patterns": [
                {
                    "name": "company_email",
                    "regex": r"[a-zA-Z0-9._%+-]+@company\.com",
                    "replacement": "[COMPANY_EMAIL]"
                }
            ]
        });

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let body = out.body.unwrap();
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert_eq!(content, "My email is [COMPANY_EMAIL] and [COMPANY_EMAIL]");
        assert_eq!(out.metadata["redactions"], 2);
    }

    #[tokio::test]
    async fn sensitive_marker_rejects_invalid_regex() {
        let plugin = SensitiveMarkerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({"messages": []}));
        let config = serde_json::json!({
            "patterns": [
                {"name": "bad", "regex": "[invalid", "replacement": "x"}
            ]
        });

        let result = plugin.run(&ctx(), input, &config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn sensitive_marker_no_match_leaves_unchanged() {
        let plugin = SensitiveMarkerPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "messages": [{"role": "user", "content": "hello"}]
        }));
        let config = serde_json::json!({
            "patterns": [
                {"name": "secret", "regex": r"sk-[a-zA-Z0-9]{20,}", "replacement": "[SECRET]"}
            ]
        });

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        assert!(!out.body_changed);
    }
}
