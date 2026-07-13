//! OpenTelemetry tracing and metrics initialization.
//!
//! Exposes a single [`init_telemetry`] entry-point that wires up the global
//! tracing subscriber and the Prometheus-compatible metrics recorder.
//! All telemetry state is externalised (no local files) so that the
//! application nodes remain stateless.

pub mod metrics;
pub mod tracing;

pub use metrics::{init_metrics, metrics_snapshot, record_http_request};

use zorch_shared::{AppConfig, AppError};

pub fn init_telemetry(cfg: &AppConfig) -> Result<(), AppError> {
    let filter = cfg.rust_log.as_str();
    tracing::init_tracing_subscriber(filter)?;
    metrics::init_metrics()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_telemetry_does_not_panic() {
        let config = AppConfig {
            database_url: "postgres://localhost/test".to_string(),
            clickhouse_url: "clickhouse://localhost".to_string(),
            redis_url: "redis://localhost".to_string(),
            app_port: 8080,
            rust_log: "info".to_string(),
            encryption_key: "test-key".to_string(),
            inspector_capture_level: "metadata_only".to_string(),
            timeout_secs: 60,
            circuit_breaker_timeout_secs: 30,
            openai_api_key: None,
            anthropic_api_key: None,
            admin_secret: None,
            default_org_id: None,
            cors_allowed_origins: Vec::new(),
            enforce_per_key_governance: true,
            sticky_target_key_ttl_secs: None,
        };
        let result = init_telemetry(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tracing_subscriber_init() {
        let config = AppConfig {
            database_url: "postgres://localhost/test".to_string(),
            clickhouse_url: "clickhouse://localhost".to_string(),
            redis_url: "redis://localhost".to_string(),
            app_port: 8080,
            rust_log: "info".to_string(),
            encryption_key: "test-key".to_string(),
            inspector_capture_level: "metadata_only".to_string(),
            timeout_secs: 60,
            circuit_breaker_timeout_secs: 30,
            openai_api_key: None,
            anthropic_api_key: None,
            admin_secret: None,
            default_org_id: None,
            cors_allowed_origins: Vec::new(),
            enforce_per_key_governance: true,
            sticky_target_key_ttl_secs: None,
        };
        let init_result = init_telemetry(&config);
        assert!(init_result.is_ok());
    }
}
