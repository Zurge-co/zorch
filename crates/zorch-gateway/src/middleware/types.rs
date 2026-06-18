use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Middleware execution phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiddlewarePhase {
    RequestPreGovernance,
    RequestPreUpstream,
    ResponsePreClient,
    InspectorPreCapture,
}

impl MiddlewarePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            MiddlewarePhase::RequestPreGovernance => "request.pre_governance",
            MiddlewarePhase::RequestPreUpstream => "request.pre_upstream",
            MiddlewarePhase::ResponsePreClient => "response.pre_client",
            MiddlewarePhase::InspectorPreCapture => "inspector.pre_capture",
        }
    }
}

impl std::str::FromStr for MiddlewarePhase {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "request.pre_governance" => Ok(MiddlewarePhase::RequestPreGovernance),
            "request.pre_upstream" => Ok(MiddlewarePhase::RequestPreUpstream),
            "response.pre_client" => Ok(MiddlewarePhase::ResponsePreClient),
            "inspector.pre_capture" => Ok(MiddlewarePhase::InspectorPreCapture),
            _ => Err(format!("unknown phase: {}", s)),
        }
    }
}

/// Action a middleware can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiddlewareAction {
    Continue,
    Block,
}

/// Failure mode when a middleware errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    FailOpen,
    FailClosed,
}

impl FailureMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureMode::FailOpen => "fail_open",
            FailureMode::FailClosed => "fail_closed",
        }
    }
}

impl std::str::FromStr for FailureMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fail_open" => Ok(FailureMode::FailOpen),
            "fail_closed" => Ok(FailureMode::FailClosed),
            _ => Err(format!("unknown failure_mode: {}", s)),
        }
    }
}

/// Scope matching configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MiddlewareScope {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub organizations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<String>,
}

impl MiddlewareScope {
    /// Returns true if the scope is global (empty).
    pub fn is_global(&self) -> bool {
        self.organizations.is_empty()
            && self.api_keys.is_empty()
            && self.providers.is_empty()
            && self.models.is_empty()
            && self.routes.is_empty()
    }

    /// Returns true if the request matches this scope.
    pub fn matches(
        &self,
        org_id: &str,
        api_key_id: &str,
        provider_id: &str,
        model_id: &str,
        route: &str,
    ) -> bool {
        if self.is_global() {
            return true;
        }
        if !self.organizations.is_empty() && !self.organizations.iter().any(|o| o == org_id) {
            return false;
        }
        if !self.api_keys.is_empty() && !self.api_keys.iter().any(|k| k == api_key_id) {
            return false;
        }
        if !self.providers.is_empty() && !self.providers.iter().any(|p| p == provider_id) {
            return false;
        }
        if !self.models.is_empty() && !self.models.iter().any(|m| m == model_id) {
            return false;
        }
        if !self.routes.is_empty() && !self.routes.iter().any(|r| r == route) {
            return false;
        }
        true
    }
}

/// Middleware execution context.
#[derive(Debug, Clone)]
pub struct MiddlewareContext {
    pub request_id: String,
    pub org_id: String,
    pub api_key_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub route: String,
}

/// Input to a middleware plugin.
#[derive(Debug, Clone)]
pub struct MiddlewareInput {
    pub body: serde_json::Value,
    pub headers: HashMap<String, String>,
}

impl MiddlewareInput {
    pub fn new(body: serde_json::Value) -> Self {
        Self {
            body,
            headers: HashMap::new(),
        }
    }

    pub fn with_body(body: serde_json::Value) -> Self {
        Self {
            body,
            headers: HashMap::new(),
        }
    }
}

/// Output from a middleware plugin.
#[derive(Debug, Clone)]
pub struct MiddlewareOutput {
    pub action: MiddlewareAction,
    pub body: Option<serde_json::Value>,
    pub headers: Option<HashMap<String, String>>,
    pub metadata: serde_json::Value,
    pub body_changed: bool,
    pub message: Option<String>,
    pub status_code: Option<u16>,
}

impl MiddlewareOutput {
    pub fn continue_with(body: serde_json::Value, metadata: serde_json::Value) -> Self {
        Self {
            action: MiddlewareAction::Continue,
            body: Some(body),
            headers: None,
            metadata,
            body_changed: true,
            message: None,
            status_code: None,
        }
    }

    pub fn continue_unchanged(metadata: serde_json::Value) -> Self {
        Self {
            action: MiddlewareAction::Continue,
            body: None,
            headers: None,
            metadata,
            body_changed: false,
            message: None,
            status_code: None,
        }
    }

    pub fn block(status_code: u16, message: String, metadata: serde_json::Value) -> Self {
        Self {
            action: MiddlewareAction::Block,
            body: None,
            headers: None,
            metadata,
            body_changed: false,
            message: Some(message),
            status_code: Some(status_code),
        }
    }
}

/// Middleware error type.
#[derive(Debug, Clone)]
pub struct MiddlewareError {
    pub plugin_key: String,
    pub message: String,
}

impl MiddlewareError {
    pub fn new(plugin_key: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            plugin_key: plugin_key.into(),
            message: message.into(),
        }
    }
}

/// Trait for built-in middleware plugins.
#[async_trait::async_trait]
pub trait MiddlewarePlugin: Send + Sync {
    fn plugin_key(&self) -> &'static str;

    async fn run(
        &self,
        ctx: &MiddlewareContext,
        input: MiddlewareInput,
        config: &serde_json::Value,
    ) -> Result<MiddlewareOutput, MiddlewareError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_parsing() {
        assert_eq!(
            "request.pre_governance".parse::<MiddlewarePhase>().unwrap(),
            MiddlewarePhase::RequestPreGovernance
        );
        assert_eq!(
            "request.pre_upstream".parse::<MiddlewarePhase>().unwrap(),
            MiddlewarePhase::RequestPreUpstream
        );
        assert!("unknown".parse::<MiddlewarePhase>().is_err());
    }

    #[test]
    fn failure_mode_parsing() {
        assert_eq!(
            "fail_open".parse::<FailureMode>().unwrap(),
            FailureMode::FailOpen
        );
        assert_eq!(
            "fail_closed".parse::<FailureMode>().unwrap(),
            FailureMode::FailClosed
        );
        assert!("unknown".parse::<FailureMode>().is_err());
    }

    #[test]
    fn scope_global_matches_all() {
        let scope = MiddlewareScope::default();
        assert!(scope.matches("org1", "key1", "openai", "gpt-4", "/v1/chat/completions"));
    }

    #[test]
    fn scope_model_match() {
        let scope = MiddlewareScope {
            models: vec!["gpt-4".to_string()],
            ..Default::default()
        };
        assert!(scope.matches("org1", "key1", "openai", "gpt-4", "/v1/chat/completions"));
        assert!(!scope.matches("org1", "key1", "openai", "gpt-3", "/v1/chat/completions"));
    }

    #[test]
    fn scope_multi_field_match() {
        let scope = MiddlewareScope {
            providers: vec!["openai".to_string()],
            models: vec!["gpt-4".to_string()],
            ..Default::default()
        };
        assert!(scope.matches("org1", "key1", "openai", "gpt-4", "/v1/chat/completions"));
        assert!(!scope.matches("org1", "key1", "anthropic", "gpt-4", "/v1/chat/completions"));
        assert!(!scope.matches("org1", "key1", "openai", "claude", "/v1/chat/completions"));
    }

    #[test]
    fn output_continue_changed() {
        let body = serde_json::json!({"model": "gpt-4"});
        let out = MiddlewareOutput::continue_with(body.clone(), serde_json::json!({"saved": 10}));
        assert_eq!(out.action, MiddlewareAction::Continue);
        assert!(out.body_changed);
        assert_eq!(out.body, Some(body));
    }

    #[test]
    fn output_block() {
        let out = MiddlewareOutput::block(
            403,
            "blocked".to_string(),
            serde_json::json!({"pattern": "secret"}),
        );
        assert_eq!(out.action, MiddlewareAction::Block);
        assert_eq!(out.status_code, Some(403));
        assert_eq!(out.message, Some("blocked".to_string()));
    }
}
