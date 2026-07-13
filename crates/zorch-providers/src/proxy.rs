use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode};
use zorch_shared::{BackendId, ProviderApiKeyId, ProviderId, SecretVault};

use crate::auth_headers::{AuthHeaders, AuthType, ProviderApiKey};
use crate::errors::ProviderError;
use crate::http_client::ProviderHttpClient;

/// A successful upstream response together with the API key that produced it.
pub struct ProxiedResponse {
    pub api_key_id: ProviderApiKeyId,
    pub response: reqwest::Response,
}

#[derive(Clone)]
pub struct ProxyProvider {
    backend_id: BackendId,
    provider_id: ProviderId,
    base_url: String,
    api_keys: Vec<ProviderApiKey>,
    vault: SecretVault,
    models: Vec<String>,
    client: ProviderHttpClient,
    auth_type: AuthType,
}

impl ProxyProvider {
    pub fn new(
        backend_id: BackendId,
        provider_id: ProviderId,
        base_url: String,
        api_keys: Vec<ProviderApiKey>,
        vault: SecretVault,
        models: Vec<String>,
        client: ProviderHttpClient,
    ) -> Self {
        Self {
            backend_id,
            provider_id,
            base_url,
            api_keys,
            vault,
            models,
            client,
            auth_type: AuthType::default(),
        }
    }

    pub fn with_auth_type(mut self, auth_type: AuthType) -> Self {
        self.auth_type = auth_type;
        self
    }

    pub fn key_count(&self) -> usize {
        self.api_keys.len()
    }

    pub fn backend_id(&self) -> BackendId {
        self.backend_id
    }

    pub fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn auth_type(&self) -> &AuthType {
        &self.auth_type
    }

    pub fn models(&self) -> &[String] {
        &self.models
    }

    pub fn api_keys(&self) -> &[ProviderApiKey] {
        &self.api_keys
    }

    #[cfg(test)]
    pub fn supports_model(&self, model: &str) -> bool {
        self.models.iter().any(|m| m == model)
    }

    /// Look up the index of the key with the given identity, if it exists.
    pub fn find_key_index(&self, key_id: &ProviderApiKeyId) -> Option<usize> {
        self.api_keys.iter().position(|k| k.id == *key_id)
    }

    fn decrypt_api_key(&self, index: usize) -> Result<String, ProviderError> {
        let key = self
            .api_keys
            .get(index)
            .ok_or_else(|| ProviderError::Internal("API key index out of range".to_string()))?;
        self.vault
            .decrypt(&key.encrypted_key)
            .map_err(|e| ProviderError::Internal(format!("Failed to decrypt API key: {}", e)))
    }

    fn build_headers(&self, key_index: usize) -> Result<HeaderMap, ProviderError> {
        let api_key = self.decrypt_api_key(key_index)?;
        let headers = AuthHeaders::from_auth_type(&api_key, &self.auth_type)?;
        Ok(headers.with_content_type().build())
    }

    /// Returns true for HTTP status codes that should trigger key-level failover.
    fn is_retryable_status(status: StatusCode) -> bool {
        status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
    }

    /// Send the request upstream, optionally starting at a preferred key.
    ///
    /// Iterates through all available keys starting from the preferred one. If a
    /// key cannot be used (decryption/header error), or if the upstream returns
    /// a retryable status, the next key is tried. Returns the first successful
    /// response along with the key id that produced it.
    pub async fn proxy_request(
        &self,
        preferred_key_id: Option<ProviderApiKeyId>,
        method: Method,
        path: &str,
        mut headers: HeaderMap,
        body: Bytes,
    ) -> Result<ProxiedResponse, ProviderError> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);

        tracing::debug!(
            provider_id = %self.provider_id,
            backend_id = %self.backend_id,
            url = %url,
            body_bytes = body.len(),
            "proxy_request: sending request upstream"
        );

        for key in [
            "host",
            "connection",
            "keep-alive",
            "transfer-encoding",
            "expect",
        ] {
            headers.remove(key);
        }

        let start_index = preferred_key_id
            .as_ref()
            .and_then(|id| self.find_key_index(id))
            .unwrap_or(0);

        let mut last_error: Option<ProviderError> = None;
        for offset in 0..self.api_keys.len() {
            let index = (start_index + offset) % self.api_keys.len();

            let provider_headers = match self.build_headers(index) {
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
                Ok(response) => {
                    let status = response.status();
                    if Self::is_retryable_status(status) {
                        tracing::warn!(
                            backend_id = %self.backend_id,
                            provider_id = %self.provider_id,
                            key_index = index,
                            status = %status,
                            "Upstream returned retryable status, failing over to next API key"
                        );
                        last_error = Some(ProviderError::Network(format!(
                            "Upstream returned retryable status {}",
                            status
                        )));
                        continue;
                    }
                    return Ok(ProxiedResponse {
                        api_key_id: self.api_keys[index].id,
                        response,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        backend_id = %self.backend_id(),
                        provider_id = %self.provider_id,
                        key_index = index,
                        error = %e,
                        "Upstream request failed, failing over to next API key"
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ProviderError::Network("No API keys available".to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_provider(auth_type: AuthType, keys: Vec<&str>) -> ProxyProvider {
        let vault = SecretVault::new("test-key").unwrap();
        let encrypted_keys: Vec<ProviderApiKey> = keys
            .iter()
            .map(|k| ProviderApiKey::new(ProviderApiKeyId::new(), vault.encrypt(k).unwrap()))
            .collect();
        ProxyProvider::new(
            BackendId::new(),
            ProviderId::from("test"),
            "https://example.test/v1".to_string(),
            encrypted_keys,
            vault,
            vec!["test-model".to_string()],
            ProviderHttpClient::new(Duration::from_secs(1)).unwrap(),
        )
        .with_auth_type(auth_type)
    }

    #[test]
    fn default_auth_type_is_bearer() {
        let vault = SecretVault::new("test-key").unwrap();
        let key = ProviderApiKey::new(ProviderApiKeyId::new(), vault.encrypt("sk-test").unwrap());
        let provider = ProxyProvider::new(
            BackendId::new(),
            ProviderId::from("test"),
            "https://example.test/v1".to_string(),
            vec![key],
            vault,
            vec![],
            ProviderHttpClient::new(Duration::from_secs(1)).unwrap(),
        );
        assert_eq!(*provider.auth_type(), AuthType::Bearer);
    }

    #[test]
    fn bearer_headers_use_bearer_auth() {
        let provider = make_provider(AuthType::Bearer, vec!["sk-test"]);
        let headers = provider.build_headers(0).unwrap();
        assert_eq!(headers.get("authorization").unwrap(), "Bearer sk-test");
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert!(headers.get("x-api-key").is_none());
    }

    #[test]
    fn anthropic_headers_use_x_api_key_auth() {
        let provider = make_provider(AuthType::Anthropic, vec!["sk-test"]);
        let headers = provider.build_headers(0).unwrap();
        assert_eq!(headers.get("x-api-key").unwrap(), "sk-test");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn custom_headers_use_provided_header_name() {
        let provider = make_provider(
            AuthType::Custom {
                header_name: "x-my-key".to_string(),
                prefix: Some("Token".to_string()),
            },
            vec!["sk-test"],
        );
        let headers = provider.build_headers(0).unwrap();
        assert_eq!(headers.get("x-my-key").unwrap(), "Token sk-test");
    }

    #[test]
    fn find_key_index_returns_index_for_known_key() {
        let vault = SecretVault::new("test-key").unwrap();
        let id = ProviderApiKeyId::new();
        let key = ProviderApiKey::new(id, vault.encrypt("sk-test").unwrap());
        let provider = ProxyProvider::new(
            BackendId::new(),
            ProviderId::from("test"),
            "https://example.test/v1".to_string(),
            vec![key],
            vault,
            vec![],
            ProviderHttpClient::new(Duration::from_secs(1)).unwrap(),
        );
        assert_eq!(provider.find_key_index(&id), Some(0));
        assert!(provider.find_key_index(&ProviderApiKeyId::new()).is_none());
    }
}
