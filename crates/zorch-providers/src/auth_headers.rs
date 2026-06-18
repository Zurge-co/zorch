use http::{HeaderMap, HeaderValue};

use crate::errors::ProviderError;

/// Builder for constructing HTTP authorization headers
pub struct AuthHeaders {
    headers: HeaderMap,
}

impl AuthHeaders {
    /// Create a new AuthHeaders builder with Bearer token authorization.
    pub fn bearer(api_key: &str) -> Result<Self, ProviderError> {
        if api_key.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
            return Err(ProviderError::Internal(
                "API key contains invalid characters".to_string(),
            ));
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| ProviderError::Internal(format!("Invalid header value: {}", e)))?,
        );
        Ok(Self { headers })
    }

    /// Create a new AuthHeaders builder with Anthropic x-api-key authorization.
    pub fn anthropic(api_key: &str) -> Result<Self, ProviderError> {
        if api_key.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
            return Err(ProviderError::Internal(
                "API key contains invalid characters".to_string(),
            ));
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(api_key)
                .map_err(|e| ProviderError::Internal(format!("Invalid header value: {}", e)))?,
        );
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        Ok(Self { headers })
    }

    /// Add Content-Type: application/json header
    pub fn with_content_type(mut self) -> Self {
        self.headers
            .insert("Content-Type", HeaderValue::from_static("application/json"));
        self
    }

    /// Build and return the HeaderMap
    pub fn build(self) -> HeaderMap {
        self.headers
    }
}
