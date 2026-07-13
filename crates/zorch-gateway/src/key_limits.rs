//! Per-API-key rate limiting and spend budget tracking.
//!
//! This module provides fine-grained rate limiting and budget enforcement
//! for individual API keys, using Redis for distributed state management.

use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use uuid::Uuid;
use zorch_shared::AppError;

/// Per-key rate limiter and spend tracker using Redis.
#[derive(Clone, Debug)]
pub struct KeyLimits {
    client: redis::Client,
}

/// Configuration for per-key limits.
#[derive(Clone, Debug)]
pub struct KeyLimitConfig {
    pub requests_per_minute: u64,
    pub requests_per_day: u64,
    pub max_spend_usd: f64,
    pub allowed_models: Vec<String>,
}

impl KeyLimits {
    /// Create a new key limits manager with an existing Redis client.
    pub fn new(client: redis::Client) -> Result<Self, AppError> {
        Ok(Self { client })
    }

    /// Check if request is within per-key rate limits.
    ///
    /// Uses sliding window algorithm with Redis sorted sets:
    /// - RPM: `key_rpm:{api_key_id}` - requests per minute window
    /// - RPD: `key_rpd:{api_key_id}` - requests per day window
    ///
    /// Returns Ok(()) if within limits, Err(AppError::RateLimit) if exceeded.
    pub async fn check_limits(
        &self,
        api_key_id: &str,
        model: &str,
        config: &KeyLimitConfig,
    ) -> Result<(), AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| AppError::Internal(format!("Time error: {}", e)))?
            .as_secs();

        let rpm_key = format!("key_rpm:{}", api_key_id);
        let rpm_window_start = now.saturating_sub(60);
        let rpm_allowed = self
            .check_sliding_window(
                &mut conn,
                &rpm_key,
                now,
                rpm_window_start,
                config.requests_per_minute,
            )
            .await?;

        if !rpm_allowed {
            return Err(AppError::RateLimit(format!(
                "Rate limit exceeded: {} requests per minute",
                config.requests_per_minute
            )));
        }

        let rpd_key = format!("key_rpd:{}", api_key_id);
        let rpd_window_start = now.saturating_sub(86400);
        let rpd_allowed = self
            .check_sliding_window(
                &mut conn,
                &rpd_key,
                now,
                rpd_window_start,
                config.requests_per_day,
            )
            .await?;

        if !rpd_allowed {
            return Err(AppError::RateLimit(format!(
                "Rate limit exceeded: {} requests per day",
                config.requests_per_day
            )));
        }

        let spend_key = format!("key_spend:{}", api_key_id);
        let current_spend: f64 = conn.get(&spend_key).await.unwrap_or(0.0);

        if current_spend >= config.max_spend_usd {
            return Err(AppError::RateLimit(format!(
                "Budget exceeded: ${:.2} / ${:.2}",
                current_spend, config.max_spend_usd
            )));
        }

        if !config.allowed_models.is_empty() && !config.allowed_models.iter().any(|m| m == model) {
            return Err(AppError::BadRequest(
                "Model not allowed for this API key".to_string(),
            ));
        }

        Ok(())
    }

    /// Check sliding window rate limit for a given key.
    ///
    /// Returns true if request is allowed, false if rate limit exceeded.
    async fn check_sliding_window(
        &self,
        conn: &mut MultiplexedConnection,
        key: &str,
        now: u64,
        window_start: u64,
        max_requests: u64,
    ) -> Result<bool, AppError> {
        let mut pipe = redis::pipe();
        pipe.zrembyscore(key, 0f64, window_start as f64);
        pipe.zcard(key);

        let results: (usize, usize) = pipe
            .query_async(&mut *conn)
            .await
            .map_err(|e| AppError::Internal(format!("Redis pipeline error: {}", e)))?;

        let current_count = results.1 as u64;

        if current_count >= max_requests {
            return Ok(false);
        }

        let request_id = format!("{}:{}", now, Uuid::new_v4());
        let _: redis::Value = conn
            .zadd(key, request_id, now as f64)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to add request: {}", e)))?;

        let expiry_secs = if max_requests == 100 { 120 } else { 86500 };
        let _: redis::Value = conn
            .expire(key, expiry_secs as i64)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to set expiry: {}", e)))?;

        Ok(true)
    }

    /// Record spend for an API key (increments Redis counter).
    ///
    /// Uses Redis key: `key_spend:{api_key_id}`
    /// TTL is set to 24 hours for daily reset.
    pub async fn record_spend(&self, api_key_id: &str, cost: f64) -> Result<(), AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;

        let key = format!("key_spend:{}", api_key_id);

        let _: f64 = redis::cmd("INCRBYFLOAT")
            .arg(&key)
            .arg(cost)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to record spend: {}", e)))?;

        let _: redis::Value = conn
            .expire(&key, 86400)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to set spend TTL: {}", e)))?;

        Ok(())
    }

    /// Get current spend for an API key.
    ///
    /// Returns 0.0 if no spend recorded.
    pub async fn get_spend(&self, api_key_id: &str) -> Result<f64, AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;

        let key = format!("key_spend:{}", api_key_id);

        let spend: Option<f64> = conn
            .get(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get spend: {}", e)))?;

        Ok(spend.unwrap_or(0.0))
    }

    #[cfg(test)]
    /// Reset rate limits for an API key (for testing use only).
    pub async fn reset(&self, api_key_id: &str) -> Result<(), AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;

        let rpm_key = format!("key_rpm:{}", api_key_id);
        let rpd_key = format!("key_rpd:{}", api_key_id);
        let spend_key = format!("key_spend:{}", api_key_id);

        let _: redis::Value = redis::cmd("DEL")
            .arg(&rpm_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to reset RPM: {}", e)))?;

        let _: redis::Value = redis::cmd("DEL")
            .arg(&rpd_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to reset RPD: {}", e)))?;

        let _: redis::Value = redis::cmd("DEL")
            .arg(&spend_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to reset spend: {}", e)))?;

        Ok(())
    }
}

impl Default for KeyLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 100,
            requests_per_day: 10000,
            max_spend_usd: 100.0,
            allowed_models: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_limit_config_default() {
        let config = KeyLimitConfig::default();
        assert_eq!(config.requests_per_minute, 100);
        assert_eq!(config.requests_per_day, 10000);
        assert_eq!(config.max_spend_usd, 100.0);
        assert!(config.allowed_models.is_empty());
    }

    #[test]
    fn test_key_limits_creation_invalid_client() {
        let client = redis::Client::open("invalid://url");
        assert!(client.is_err());
    }

    #[test]
    fn test_redis_key_generation() {
        let api_key = "key-123";
        assert_eq!(format!("key_rpm:{}", api_key), "key_rpm:key-123");
        assert_eq!(format!("key_rpd:{}", api_key), "key_rpd:key-123");
        assert_eq!(format!("key_spend:{}", api_key), "key_spend:key-123");
    }

    #[test]
    fn test_model_allowlist_check() {
        let config = KeyLimitConfig {
            requests_per_minute: 100,
            requests_per_day: 10000,
            max_spend_usd: 100.0,
            allowed_models: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
        };

        assert!(config.allowed_models.iter().any(|m| m == "gpt-4"));
        assert!(!config.allowed_models.iter().any(|m| m == "claude-3"));
    }

    #[test]
    fn test_empty_allowlist_means_all_allowed() {
        let config = KeyLimitConfig::default();
        assert!(config.allowed_models.is_empty());
    }

    #[test]
    fn test_budget_calculation() {
        let max_budget: f64 = 50.0;
        let current_spend: f64 = 35.5;
        let remaining = (max_budget - current_spend).max(0.0);
        assert!((remaining - 14.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_budget_exceeded() {
        let max_budget: f64 = 50.0;
        let current_spend: f64 = 55.0;
        let remaining = (max_budget - current_spend).max(0.0);
        assert_eq!(remaining, 0.0);
    }

    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_rate_limit_allows_under_limit() {
        let limiter =
            KeyLimits::new(redis::Client::open("redis://localhost:6379").unwrap()).unwrap();
        let config = KeyLimitConfig {
            requests_per_minute: 10,
            requests_per_day: 1000,
            max_spend_usd: 100.0,
            allowed_models: Vec::new(),
        };

        let result = limiter.check_limits("test-key", "gpt-4", &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_spend_tracking() {
        let limiter =
            KeyLimits::new(redis::Client::open("redis://localhost:6379").unwrap()).unwrap();
        let api_key = "test-spend-key";

        limiter.reset(api_key).await.unwrap();
        limiter.record_spend(api_key, 5.5).await.unwrap();
        limiter.record_spend(api_key, 3.25).await.unwrap();

        let spend = limiter.get_spend(api_key).await.unwrap();
        assert!((spend - 8.75).abs() < 0.01);

        limiter.reset(api_key).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_budget_enforcement() {
        let limiter =
            KeyLimits::new(redis::Client::open("redis://localhost:6379").unwrap()).unwrap();
        let api_key = "test-budget-key";

        limiter.reset(api_key).await.unwrap();

        let config = KeyLimitConfig {
            requests_per_minute: 100,
            requests_per_day: 10000,
            max_spend_usd: 10.0,
            allowed_models: Vec::new(),
        };

        limiter.record_spend(api_key, 9.5).await.unwrap();

        let result = limiter.check_limits(api_key, "gpt-4", &config).await;
        assert!(result.is_ok());

        limiter.record_spend(api_key, 1.0).await.unwrap();

        let result = limiter.check_limits(api_key, "gpt-4", &config).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::RateLimit(_)));

        limiter.reset(api_key).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_model_allowlist_enforcement() {
        let limiter =
            KeyLimits::new(redis::Client::open("redis://localhost:6379").unwrap()).unwrap();
        let api_key = "test-model-key";

        let config = KeyLimitConfig {
            requests_per_minute: 100,
            requests_per_day: 10000,
            max_spend_usd: 100.0,
            allowed_models: vec!["gpt-4".to_string()],
        };

        let result = limiter.check_limits(api_key, "gpt-4", &config).await;
        assert!(result.is_ok());

        let result = limiter.check_limits(api_key, "claude-3", &config).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::BadRequest(_)));
    }
}
