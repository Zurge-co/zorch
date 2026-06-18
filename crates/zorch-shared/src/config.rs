use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

use crate::errors::AppError;

/// Application configuration loaded from Settings.toml and environment variables
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub clickhouse_url: String,
    pub redis_url: String,
    #[serde(default = "default_app_port")]
    pub app_port: u16,
    #[serde(default = "default_rust_log")]
    pub rust_log: String,
    /// Master AES-256-GCM key for `SecretVault`.
    /// Used to encrypt/decrypt provider API keys stored in the `providers.config`
    /// JSON column, as well as environment fallback keys before they are used.
    #[serde(default)]
    pub encryption_key: String,
    #[serde(default = "default_inspector_capture_level")]
    pub inspector_capture_level: String,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub openai_api_key: Option<String>,
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
    /// Admin secret for protecting /admin/* routes.
    /// If empty, admin routes will still work in development but should be set in production.
    #[serde(default)]
    pub admin_secret: Option<String>,
    #[serde(default)]
    pub default_org_id: Option<String>,
    /// Comma-separated list of origins allowed by CORS.
    /// Empty (default) means CORS allows any origin — fine for local dev,
    /// MUST be configured in production (e.g. `https://admin.example.com`).
    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,
}

fn default_timeout_secs() -> u64 {
    60
}

fn default_app_port() -> u16 {
    8080
}

fn default_rust_log() -> String {
    "info".to_string()
}

#[allow(dead_code)] // Used only in tests
fn default_inspector_capture_level() -> String {
    "metadata_only".to_string()
}

impl AppConfig {
    /// Load configuration from Settings.toml (optional) and ZORCH_ prefixed environment variables
    pub fn load() -> Result<Self, AppError> {
        let config = Config::builder()
            .add_source(File::with_name("Settings").required(false))
            .add_source(Environment::with_prefix("ZORCH"))
            .build()
            .map_err(|e| AppError::Config(format!("Failed to build config: {}", e)))?;

        config.try_deserialize().map_err(|e| match e {
            ConfigError::NotFound(msg) => AppError::Config(format!("Config not found: {}", msg)),
            ConfigError::Message(msg) => AppError::Config(msg),
            ConfigError::Foreign(msg) => AppError::Config(format!("Foreign config error: {}", msg)),
            _ => AppError::Config(format!("Config error: {}", e)),
        })
    }

    pub fn timeout_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::Config;

    #[test]
    fn test_default_app_port() {
        assert_eq!(default_app_port(), 8080);
    }

    #[test]
    fn test_default_rust_log() {
        assert_eq!(default_rust_log(), "info");
    }

    #[test]
    fn test_default_inspector_capture_level() {
        assert_eq!(default_inspector_capture_level(), "metadata_only");
    }

    #[test]
    fn test_app_config_defaults() {
        let config = Config::builder()
            .set_default("database_url", "postgres://localhost/test")
            .unwrap()
            .set_default("clickhouse_url", "clickhouse://localhost")
            .unwrap()
            .set_default("redis_url", "redis://localhost")
            .unwrap()
            .set_default("encryption_key", "test-key")
            .unwrap()
            .build()
            .unwrap();

        let config: AppConfig = config.try_deserialize().unwrap();
        assert_eq!(config.app_port, 8080);
        assert_eq!(config.rust_log, "info");
        assert_eq!(config.inspector_capture_level, "metadata_only");
    }

    #[test]
    fn test_app_config_custom_values() {
        let config = Config::builder()
            .set_default("database_url", "postgres://localhost/test")
            .unwrap()
            .set_default("clickhouse_url", "clickhouse://localhost")
            .unwrap()
            .set_default("redis_url", "redis://localhost")
            .unwrap()
            .set_default("encryption_key", "test-key")
            .unwrap()
            .set_default("app_port", 3000)
            .unwrap()
            .set_default("rust_log", "debug")
            .unwrap()
            .set_default("inspector_capture_level", "full")
            .unwrap()
            .build()
            .unwrap();

        let config: AppConfig = config.try_deserialize().unwrap();
        assert_eq!(config.app_port, 3000);
        assert_eq!(config.rust_log, "debug");
        assert_eq!(config.inspector_capture_level, "full");
    }

    #[test]
    fn test_app_config_load_from_env() {
        std::env::set_var(
            "ZORCH_DATABASE_URL",
            "postgres://user:pass@localhost:5432/zorch",
        );
        std::env::set_var("ZORCH_CLICKHOUSE_URL", "http://localhost:8123");
        std::env::set_var("ZORCH_REDIS_URL", "redis://localhost:6379");
        let config = AppConfig::load();
        std::env::remove_var("ZORCH_DATABASE_URL");
        std::env::remove_var("ZORCH_CLICKHOUSE_URL");
        std::env::remove_var("ZORCH_REDIS_URL");
        assert!(config.is_ok(), "Failed to load config: {:?}", config.err());
    }
}
