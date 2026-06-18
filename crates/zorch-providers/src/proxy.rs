use bytes::Bytes;
use http::{HeaderMap, Method};
use zorch_shared::{ProviderId, SecretVault};

use crate::auth_headers::AuthHeaders;
use crate::errors::ProviderError;
use crate::http_client::ProviderHttpClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Protocol {
    #[default]
    OpenAICompatible,
    Anthropic,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Protocol::OpenAICompatible => "openai_compatible",
            Protocol::Anthropic => "anthropic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_provider(protocol: Protocol) -> ProxyProvider {
        let vault = SecretVault::new("test-key").unwrap();
        let encrypted = vault.encrypt("sk-test").unwrap();
        ProxyProvider::new(
            ProviderId::from("test"),
            "https://example.test/v1".to_string(),
            encrypted,
            vault,
            vec!["test-model".to_string()],
            ProviderHttpClient::new(Duration::from_secs(1)).unwrap(),
        )
        .with_protocol(protocol)
    }

    #[test]
    fn default_protocol_is_openai_compatible() {
        let vault = SecretVault::new("test-key").unwrap();
        let encrypted = vault.encrypt("sk-test").unwrap();
        let provider = ProxyProvider::new(
            ProviderId::from("test"),
            "https://example.test/v1".to_string(),
            encrypted,
            vault,
            vec![],
            ProviderHttpClient::new(Duration::from_secs(1)).unwrap(),
        );

        assert_eq!(provider.protocol(), Protocol::OpenAICompatible);
    }

    #[test]
    fn openai_compatible_headers_use_bearer_auth() {
        let headers = make_provider(Protocol::OpenAICompatible)
            .build_headers(0)
            .unwrap();

        assert_eq!(headers.get("authorization").unwrap(), "Bearer sk-test");
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert!(headers.get("x-api-key").is_none());
    }

    #[test]
    fn anthropic_headers_use_x_api_key_auth() {
        let headers = make_provider(Protocol::Anthropic).build_headers(0).unwrap();

        assert_eq!(headers.get("x-api-key").unwrap(), "sk-test");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn protocol_parses_config_values_and_legacy_openai() {
        assert_eq!(
            "openai_compatible".parse::<Protocol>().unwrap(),
            Protocol::OpenAICompatible
        );
        assert_eq!(
            "openai".parse::<Protocol>().unwrap(),
            Protocol::OpenAICompatible
        );
        assert_eq!(
            "anthropic".parse::<Protocol>().unwrap(),
            Protocol::Anthropic
        );
        assert!("gemini".parse::<Protocol>().is_err());
    }
}

impl std::str::FromStr for Protocol {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai_compatible" | "openai" => Ok(Protocol::OpenAICompatible),
            "anthropic" => Ok(Protocol::Anthropic),
            _ => Err(()),
        }
    }
}

#[derive(Clone)]
pub struct ProxyProvider {
    provider_id: ProviderId,
    base_url: String,
    encrypted_api_keys: Vec<String>,
    vault: SecretVault,
    models: Vec<String>,
    client: ProviderHttpClient,
    protocol: Protocol,
}

impl ProxyProvider {
    pub fn new(
        provider_id: ProviderId,
        base_url: String,
        api_key: String,
        vault: SecretVault,
        models: Vec<String>,
        client: ProviderHttpClient,
    ) -> Self {
        Self {
            provider_id,
            base_url,
            encrypted_api_keys: vec![api_key],
            vault,
            models,
            client,
            protocol: Protocol::default(),
        }
    }

    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn add_key(&mut self, api_key: String) {
        self.encrypted_api_keys.push(api_key);
    }

    pub fn key_count(&self) -> usize {
        self.encrypted_api_keys.len()
    }

    pub fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub fn models(&self) -> &[String] {
        &self.models
    }

    pub fn supports_model(&self, model: &str) -> bool {
        self.models.iter().any(|m| m == model)
    }

    fn decrypt_api_key(&self, index: usize) -> Result<String, ProviderError> {
        let encrypted = self
            .encrypted_api_keys
            .get(index)
            .ok_or_else(|| ProviderError::Internal("API key index out of range".to_string()))?;
        self.vault
            .decrypt(encrypted)
            .map_err(|e| ProviderError::Internal(format!("Failed to decrypt API key: {}", e)))
    }

    fn build_headers(&self, key_index: usize) -> Result<HeaderMap, ProviderError> {
        let api_key = self.decrypt_api_key(key_index)?;
        let headers = match self.protocol {
            Protocol::OpenAICompatible => AuthHeaders::bearer(&api_key)?,
            Protocol::Anthropic => AuthHeaders::anthropic(&api_key)?,
        };
        Ok(headers.with_content_type().build())
    }

    pub async fn proxy_request(
        &self,
        method: Method,
        path: &str,
        mut headers: HeaderMap,
        body: Bytes,
    ) -> Result<reqwest::Response, ProviderError> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);

        for key in [
            "host",
            "connection",
            "keep-alive",
            "transfer-encoding",
            "expect",
        ] {
            headers.remove(key);
        }

        let mut last_error = None;
        for i in 0..self.encrypted_api_keys.len() {
            let provider_headers = match self.build_headers(i) {
                Ok(h) => h,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            let mut attempt_headers = headers.clone();
            for (key, value) in provider_headers.iter() {
                if !attempt_headers.contains_key(key) {
                    attempt_headers.insert(key.clone(), value.clone());
                }
            }

            match self
                .client
                .request(method.clone(), &url, attempt_headers, body.clone())
                .await
            {
                Ok(response) => return Ok(response),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error
            .unwrap_or_else(|| ProviderError::Network("No API keys available".to_string())))
    }
}
