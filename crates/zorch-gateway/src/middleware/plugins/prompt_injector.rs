use async_trait::async_trait;
use serde_json::Value;

use crate::middleware::types::{MiddlewareContext, MiddlewareError, MiddlewareInput, MiddlewareOutput};

pub struct PromptInjectorPlugin;

#[async_trait]
impl crate::middleware::types::MiddlewarePlugin for PromptInjectorPlugin {
    fn plugin_key(&self) -> &'static str {
        "prompt_injector"
    }

    async fn run(
        &self,
        _ctx: &MiddlewareContext,
        input: MiddlewareInput,
        config: &Value,
    ) -> Result<MiddlewareOutput, MiddlewareError> {
        let text = config
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MiddlewareError::new("prompt_injector", "missing 'text' in config"))?;

        let position = config
            .get("position")
            .and_then(|v| v.as_str())
            .unwrap_or("system_prefix");

        let mut body = input.body.clone();
        let mut injected = false;

        if let Some(messages) = body.get_mut("messages") {
            if let Some(arr) = messages.as_array_mut() {
                match position {
                    "system_prefix" => {
                        if let Some(first) = arr.get_mut(0) {
                            if let Some(role) = first.get("role").and_then(|v| v.as_str()) {
                                if role == "system" {
                                    if let Some(content) = first.get_mut("content") {
                                        if let Some(s) = content.as_str() {
                                            let combined = format!("{}\n\n{}", text, s);
                                            *content = Value::String(combined);
                                            injected = true;
                                        }
                                    }
                                }
                            }
                        }
                        if !injected {
                            let system_msg = serde_json::json!({
                                "role": "system",
                                "content": text
                            });
                            arr.insert(0, system_msg);
                            injected = true;
                        }
                    }
                    "system_suffix" => {
                        let mut found_system = false;
                        for msg in arr.iter_mut() {
                            if let Some(role) = msg.get("role").and_then(|v| v.as_str()) {
                                if role == "system" {
                                    if let Some(content) = msg.get_mut("content") {
                                        if let Some(s) = content.as_str() {
                                            let combined = format!("{}\n\n{}", s, text);
                                            *content = Value::String(combined);
                                            found_system = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if !found_system {
                            let system_msg = serde_json::json!({
                                "role": "system",
                                "content": text
                            });
                            arr.push(system_msg);
                            injected = true;
                        } else {
                            injected = true;
                        }
                    }
                    _ => {
                        return Err(MiddlewareError::new(
                            "prompt_injector",
                            format!("unknown position '{}'; use 'system_prefix' or 'system_suffix'", position),
                        ));
                    }
                }
            }
        }

        let metadata = serde_json::json!({
            "injected": injected,
            "position": position,
            "text_length": text.len(),
        });

        if injected {
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
    async fn prompt_injector_adds_system_prefix() {
        let plugin = PromptInjectorPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }));
        let config = serde_json::json!({
            "position": "system_prefix",
            "text": "You are using company AI infrastructure. Do not reveal confidential data."
        });

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let body = out.body.unwrap();
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"].as_str().unwrap(), "You are using company AI infrastructure. Do not reveal confidential data.");
        assert_eq!(messages[1]["role"], "user");
    }

    #[tokio::test]
    async fn prompt_injector_prefixes_existing_system() {
        let plugin = PromptInjectorPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "messages": [
                {"role": "system", "content": "Be helpful."},
                {"role": "user", "content": "Hello"}
            ]
        }));
        let config = serde_json::json!({
            "position": "system_prefix",
            "text": "Company policy: do not reveal secrets."
        });

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let body = out.body.unwrap();
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        let content = messages[0]["content"].as_str().unwrap();
        assert!(content.starts_with("Company policy: do not reveal secrets."));
        assert!(content.contains("Be helpful."));
    }

    #[tokio::test]
    async fn prompt_injector_appends_system_suffix() {
        let plugin = PromptInjectorPlugin;
        let input = MiddlewareInput::new(serde_json::json!({
            "messages": [
                {"role": "system", "content": "Be helpful."},
                {"role": "user", "content": "Hello"}
            ]
        }));
        let config = serde_json::json!({
            "position": "system_suffix",
            "text": "Remember: be concise."
        });

        let out = plugin.run(&ctx(), input, &config).await.unwrap();
        let body = out.body.unwrap();
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        let content = messages[0]["content"].as_str().unwrap();
        assert!(content.starts_with("Be helpful."));
        assert!(content.ends_with("Remember: be concise."));
    }
}
