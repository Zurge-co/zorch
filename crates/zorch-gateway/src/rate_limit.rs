use redis::AsyncCommands;
use uuid::Uuid;
use zorch_shared::AppError;

/// Rate limiter using Redis for distributed sliding window rate limiting.
#[derive(Clone, Debug)]
pub struct RateLimiter {
    client: redis::Client,
}

impl RateLimiter {
    /// Create a new rate limiter with an existing Redis client.
    pub fn new(client: redis::Client) -> Result<Self, AppError> {
        Ok(Self { client })
    }

    /// Check rate limit for a given API key and model combination.
    ///
    /// Uses sliding window algorithm with Redis sorted sets:
    /// - Each request is stored with timestamp as score
    /// - Old entries outside the window are removed
    /// - Count of remaining entries is compared against max_requests
    pub async fn check_rate_limit(
        &self,
        api_key_id: &str,
        model_id: &str,
        window_secs: u64,
        max_requests: u64,
    ) -> Result<bool, AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| AppError::Internal(format!("Time error: {}", e)))?
            .as_secs();

        let key = format!("rate_limit:{}:{}", api_key_id, model_id);
        let window_start = now.saturating_sub(window_secs);

        let mut pipe = redis::pipe();
        pipe.zrembyscore(&key, 0f64, window_start as f64);
        pipe.zcard(&key);

        let results: (usize, usize) = pipe
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Redis pipeline error: {}", e)))?;

        let current_count = results.1 as u64;

        if current_count >= max_requests {
            return Ok(false);
        }

        let request_id = format!("{}:{}", now, Uuid::new_v4());
        let _: redis::Value = conn
            .zadd(&key, request_id, now as f64)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to add request: {}", e)))?;

        let expiry_secs = window_secs + 60;
        let _: redis::Value = conn
            .expire(&key, expiry_secs as i64)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to set expiry: {}", e)))?;

        Ok(true)
    }

    pub async fn reset(&self, api_key_id: &str, model_id: &str) -> Result<(), AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get Redis connection: {}", e)))?;

        let key = format!("rate_limit:{}:{}", api_key_id, model_id);
        let _: redis::Value = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to reset rate limit: {}", e)))?;

        Ok(())
    }
}

/// Configuration for rate limiting.
/// Currently used only in tests; kept as the public shape for future per-route configuration.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RateLimitConfig {
    pub window_secs: u64,
    pub max_requests: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            window_secs: 60,
            max_requests: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.window_secs, 60);
        assert_eq!(config.max_requests, 100);
    }

    #[test]
    fn test_rate_limiter_creation_invalid_client() {
        let client = redis::Client::open("invalid://url");
        assert!(client.is_err());
    }

    #[test]
    fn test_rate_limit_key_generation() {
        let api_key = "key-123";
        let model = "gpt-4";
        let expected = format!("rate_limit:{}:{}", api_key, model);
        assert_eq!(expected, "rate_limit:key-123:gpt-4");
    }

    #[test]
    fn test_window_calculation() {
        let now = 1000u64;
        let window_secs = 60u64;
        let window_start = now.saturating_sub(window_secs);
        assert_eq!(window_start, 940);
    }

    #[test]
    fn test_expiry_buffer_calculation() {
        let window_secs = 60u64;
        let expiry_secs = window_secs + 60;
        assert_eq!(expiry_secs, 120);
    }

    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_rate_limit_allows_under_limit() {
        let limiter =
            RateLimiter::new(redis::Client::open("redis://localhost:6379").unwrap()).unwrap();

        let api_key = "test-key";
        let model = "test-model";
        let window = 60;
        let max = 10;

        let result = limiter.check_rate_limit(api_key, model, window, max).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_rate_limit_blocks_over_limit() {
        let limiter =
            RateLimiter::new(redis::Client::open("redis://localhost:6379").unwrap()).unwrap();

        let api_key = "test-key-2";
        let model = "test-model";
        let window = 60;
        let max = 3;

        for _ in 0..max {
            let result = limiter.check_rate_limit(api_key, model, window, max).await;
            assert!(result.unwrap());
        }

        let result = limiter.check_rate_limit(api_key, model, window, max).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_rate_limit_per_model_isolation() {
        let limiter =
            RateLimiter::new(redis::Client::open("redis://localhost:6379").unwrap()).unwrap();

        let api_key = "test-key-3";
        let model1 = "model-1";
        let model2 = "model-2";
        let window = 60;
        let max = 2;

        for _ in 0..max {
            limiter
                .check_rate_limit(api_key, model1, window, max)
                .await
                .unwrap();
        }

        let result = limiter.check_rate_limit(api_key, model2, window, max).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_rate_limit_reset() {
        let limiter =
            RateLimiter::new(redis::Client::open("redis://localhost:6379").unwrap()).unwrap();

        let api_key = "test-key-4";
        let model = "test-model";
        let window = 60;
        let max = 2;

        for _ in 0..max {
            limiter
                .check_rate_limit(api_key, model, window, max)
                .await
                .unwrap();
        }

        let result = limiter.check_rate_limit(api_key, model, window, max).await;
        assert!(!result.unwrap());

        limiter.reset(api_key, model).await.unwrap();

        let result = limiter.check_rate_limit(api_key, model, window, max).await;
        assert!(result.unwrap());
    }
}
