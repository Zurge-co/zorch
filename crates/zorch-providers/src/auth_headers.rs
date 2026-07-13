use http::{HeaderMap, HeaderValue};
use zorch_shared::ProviderApiKeyId;

use crate::errors::ProviderError;

/// Authentication style used when talking to an upstream provider.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AuthType {
    /// `Authorization: Bearer <api_key>`
    #[default]
    Bearer,
    /// Anthropic style: `x-api-key: <api_key>` plus a static version header.
    Anthropic,
    /// Arbitrary header name, with an optional prefix such as `Token`.
    Custom {
        header_name: String,
        prefix: Option<String>,
    },
}

impl AuthType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::Bearer => "bearer",
            AuthType::Anthropic => "anthropic",
            AuthType::Custom { .. } => "custom",
        }
    }
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
impl std::str::FromStr for AuthType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bearer" => Ok(AuthType::Bearer),
            "openai" | "openai_compatible" => Ok(AuthType::Bearer),
            "anthropic" => Ok(AuthType::Anthropic),
            _ => Err(()),
        }
    }
}

impl AuthType {
    /// Parse an auth type that may include optional custom header metadata from a
    /// JSON config object.
    pub fn from_config(
        auth_type: &str,
        header_name: Option<&str>,
        prefix: Option<&str>,
    ) -> Result<Self, ProviderError> {
        match auth_type.to_lowercase().as_str() {
            "bearer" | "openai" | "openai_compatible" => Ok(AuthType::Bearer),
            "anthropic" => Ok(AuthType::Anthropic),
            "custom" => {
                let header_name = header_name
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| ProviderError::Internal(
                        "Custom auth type requires auth_header_name".to_string()
                    ))?
                    .to_string();
                let prefix = prefix.filter(|s| !s.is_empty()).map(|s| s.to_string());
                Ok(AuthType::Custom { header_name, prefix })
            }
            _ => Err(ProviderError::Internal(format!("Unknown auth type: {}", auth_type))),
        }
    }
}

/// A single upstream API key along with its stable identity.
#[derive(Debug, Clone)]
pub struct ProviderApiKey {
    pub id: ProviderApiKeyId,
    pub encrypted_key: String,
}

impl ProviderApiKey {
    pub fn new(id: ProviderApiKeyId, encrypted_key: String) -> Self {
        Self { id, encrypted_key }
    }
}

/// Builder for constructing HTTP authorization headers.
pub struct AuthHeaders {
    headers: HeaderMap,
}

impl AuthHeaders {
    fn validate_key(api_key: &str) -> Result<(), ProviderError> {
        if api_key.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
            return Err(ProviderError::Internal(
                "API key contains invalid characters".to_string(),
            ));
        }
        Ok(())
    }

    /// Build headers for the given auth type and key.
    pub fn from_auth_type(api_key: &str, auth_type: &AuthType) -> Result<Self, ProviderError> {
        Self::validate_key(api_key)?;

        let mut headers = HeaderMap::new();
        match auth_type {
            AuthType::Bearer => {
                headers.insert(
                    "Authorization",
                    HeaderValue::from_str(&format!("Bearer {}", api_key))
                        .map_err(|e| ProviderError::Internal(format!("Invalid header value: {}", e)))?,
                );
            }
            AuthType::Anthropic => {
                headers.insert(
                    "x-api-key",
                    HeaderValue::from_str(api_key)
                        .map_err(|e| ProviderError::Internal(format!("Invalid header value: {}", e)))?,
                );
                headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
            }
            AuthType::Custom { header_name, prefix } => {
                let value = match prefix {
                    Some(p) => format!("{} {}", p, api_key),
                    None => api_key.to_string(),
                };
                let header_name = header_name.parse::<http::HeaderName>().map_err(|e| {
                    ProviderError::Internal(format!("Invalid custom header name: {}", e))
                })?;
                headers.insert(
                    header_name,
                    HeaderValue::from_str(&value)
                        .map_err(|e| ProviderError::Internal(format!("Invalid header value: {}", e)))?,
                );
            }
        }
        Ok(Self { headers })
    }

    /// Add Content-Type: application/json header.
    pub fn with_content_type(mut self) -> Self {
        self.headers
            .insert("Content-Type", HeaderValue::from_static("application/json"));
        self
    }

    /// Build and return the HeaderMap.
    pub fn build(self) -> HeaderMap {
        self.headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_auth_uses_authorization_header() {
        let headers = AuthHeaders::from_auth_type("sk-test", &AuthType::Bearer)
            .unwrap()
            .with_content_type()
            .build();
        assert_eq!(headers.get("authorization").unwrap(), "Bearer sk-test");
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
    }

    #[test]
    fn anthropic_auth_uses_x_api_key_and_version() {
        let headers = AuthHeaders::from_auth_type("sk-ant", &AuthType::Anthropic)
            .unwrap()
            .with_content_type()
            .build();
        assert_eq!(headers.get("x-api-key").unwrap(), "sk-ant");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn custom_auth_uses_provided_header() {
        let auth = AuthType::Custom {
            header_name: "x-my-key".to_string(),
            prefix: None,
        };
        let headers = AuthHeaders::from_auth_type("sk-custom", &auth).unwrap().build();
        assert_eq!(headers.get("x-my-key").unwrap(), "sk-custom");
    }

    #[test]
    fn custom_auth_with_prefix() {
        let auth = AuthType::Custom {
            header_name: "x-my-key".to_string(),
            prefix: Some("Token".to_string()),
        };
        let headers = AuthHeaders::from_auth_type("sk-custom", &auth).unwrap().build();
        assert_eq!(headers.get("x-my-key").unwrap(), "Token sk-custom");
    }

    #[test]
    fn auth_type_parses_strings() {
        assert_eq!("bearer".parse::<AuthType>().unwrap(), AuthType::Bearer);
        assert_eq!("openai".parse::<AuthType>().unwrap(), AuthType::Bearer);
        assert_eq!("openai_compatible".parse::<AuthType>().unwrap(), AuthType::Bearer);
        assert_eq!("anthropic".parse::<AuthType>().unwrap(), AuthType::Anthropic);
        assert!("gemini".parse::<AuthType>().is_err());
    }

    #[test]
    fn rejects_invalid_key_characters() {
        assert!(AuthHeaders::from_auth_type("sk\n-test", &AuthType::Bearer).is_err());
    }
}
