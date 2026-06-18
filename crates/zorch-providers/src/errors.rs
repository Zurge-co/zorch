use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Rate limited")]
    RateLimit { retry_after: Option<u64> },

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Request timeout")]
    Timeout,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<reqwest::Error> for ProviderError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ProviderError::Timeout
        } else if err.is_status() {
            match err.status() {
                Some(status) if status.as_u16() == 401 => {
                    ProviderError::Auth("Invalid API key".to_string())
                }
                Some(status) if status.as_u16() == 429 => {
                    ProviderError::RateLimit { retry_after: None }
                }
                _ => ProviderError::Network(err.to_string()),
            }
        } else {
            ProviderError::Network(err.to_string())
        }
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(err: serde_json::Error) -> Self {
        ProviderError::InvalidResponse(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_error_display() {
        assert_eq!(
            ProviderError::Network("connection failed".to_string()).to_string(),
            "Network error: connection failed"
        );
        assert_eq!(
            ProviderError::Auth("invalid key".to_string()).to_string(),
            "Authentication error: invalid key"
        );
        assert_eq!(
            ProviderError::RateLimit {
                retry_after: Some(60)
            }
            .to_string(),
            "Rate limited"
        );
        assert_eq!(
            ProviderError::InvalidResponse("bad json".to_string()).to_string(),
            "Invalid response: bad json"
        );
        assert_eq!(
            ProviderError::NotFound("model not found".to_string()).to_string(),
            "Not found: model not found"
        );
        assert_eq!(ProviderError::Timeout.to_string(), "Request timeout");
        assert_eq!(
            ProviderError::Internal("something broke".to_string()).to_string(),
            "Internal error: something broke"
        );
    }

    #[test]
    fn test_from_serde_json_error() {
        let invalid_json = "not valid json";
        let parse_result: Result<serde_json::Value, _> = serde_json::from_str(invalid_json);
        let serde_err = parse_result.unwrap_err();
        let provider_err: ProviderError = serde_err.into();
        assert!(matches!(provider_err, ProviderError::InvalidResponse(_)));
    }

    #[test]
    fn test_rate_limit_with_retry_after() {
        let err = ProviderError::RateLimit {
            retry_after: Some(120),
        };
        assert!(matches!(
            err,
            ProviderError::RateLimit {
                retry_after: Some(120)
            }
        ));
    }

    #[test]
    fn test_rate_limit_without_retry_after() {
        let err = ProviderError::RateLimit { retry_after: None };
        assert!(matches!(
            err,
            ProviderError::RateLimit { retry_after: None }
        ));
    }
}
