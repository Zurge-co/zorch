use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;

/// Core application error types for the Zorch platform
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Request timeout")]
    Timeout,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Outside allowed hours: {start:02}:00-{end:02}:00 {timezone}")]
    AccessWindow {
        start: u8,
        end: u8,
        timezone: String,
    },
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Database(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Auth(msg) => {
                if msg.contains("forbidden") || msg.contains("permission") {
                    StatusCode::FORBIDDEN
                } else {
                    StatusCode::UNAUTHORIZED
                }
            }
            AppError::Provider(_) => StatusCode::BAD_GATEWAY,
            AppError::Timeout => StatusCode::REQUEST_TIMEOUT,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::RateLimit(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::AccessWindow { .. } => StatusCode::FORBIDDEN,
        };

        match &self {
            AppError::AccessWindow { start, end, timezone } => {
                let body = serde_json::json!({
                    "error": "outside_allowed_hours",
                    "window": {
                        "start": start,
                        "end": end,
                        "timezone": timezone,
                    }
                });
                (status, Json(body)).into_response()
            }
            _ => (status, self.to_string()).into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_display() {
        let err = AppError::Validation("invalid input".to_string());
        assert_eq!(err.to_string(), "Validation error: invalid input");
    }

    #[test]
    fn test_auth_error_display() {
        let err = AppError::Auth("invalid token".to_string());
        assert_eq!(err.to_string(), "Authentication error: invalid token");
    }

    #[test]
    fn test_provider_error_display() {
        let err = AppError::Provider("upstream failed".to_string());
        assert_eq!(err.to_string(), "Provider error: upstream failed");
    }

    #[test]
    fn test_timeout_error_display() {
        let err = AppError::Timeout;
        assert_eq!(err.to_string(), "Request timeout");
    }

    #[test]
    fn test_internal_error_display() {
        let err = AppError::Internal("something went wrong".to_string());
        assert_eq!(err.to_string(), "Internal error: something went wrong");
    }

    #[test]
    fn test_database_error_display() {
        let err = AppError::Database("connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: connection failed");
    }

    #[test]
    fn test_config_error_display() {
        let err = AppError::Config("missing field".to_string());
        assert_eq!(err.to_string(), "Configuration error: missing field");
    }

    #[test]
    fn test_validation_error_status() {
        let err = AppError::Validation("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_auth_error_unauthorized_status() {
        let err = AppError::Auth("invalid token".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_forbidden_status() {
        let err = AppError::Auth("permission denied".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_provider_error_status() {
        let err = AppError::Provider("upstream error".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_timeout_error_status() {
        let err = AppError::Timeout;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    }

    #[test]
    fn test_internal_error_status() {
        let err = AppError::Internal("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_database_error_status() {
        let err = AppError::Database("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_config_error_status() {
        let err = AppError::Config("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_bad_request_error_display() {
        let err = AppError::BadRequest("invalid input".to_string());
        assert_eq!(err.to_string(), "Bad request: invalid input");
    }

    #[test]
    fn test_rate_limit_error_display() {
        let err = AppError::RateLimit("too many requests".to_string());
        assert_eq!(err.to_string(), "Rate limit exceeded: too many requests");
    }

    #[test]
    fn test_bad_request_error_status() {
        let err = AppError::BadRequest("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_rate_limit_error_status() {
        let err = AppError::RateLimit("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_access_window_error_status() {
        let err = AppError::AccessWindow {
            start: 9,
            end: 18,
            timezone: "Asia/Bangkok".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
