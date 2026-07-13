use redis::AsyncCommands;
use zorch_shared::{ApiKeyId, ProviderApiKeyId, ProviderId};

/// Redis-backed sticky mapping from a Zorch client API key + provider to a
/// chosen upstream provider API key.
///
/// This lets repeated requests from the same client key route to the same
/// target API key for a configurable TTL, improving upstream cache hit rates.
pub struct StickyTargetKeyCache {
    client: redis::Client,
    ttl_secs: u64,
}

impl StickyTargetKeyCache {
    /// Create a new cache with an existing Redis client.
    ///
    /// # Arguments
    /// * `client` - Redis/Dragonfly client
    /// * `ttl_secs` - Time-to-live for each sticky mapping in seconds
    pub fn new(client: redis::Client, ttl_secs: u64) -> Result<Self, zorch_shared::AppError> {
        Ok(Self { client, ttl_secs })
    }

    fn key(&self, api_key_id: &ApiKeyId, provider_id: &ProviderId) -> String {
        format!(
            "zorch:sticky_target_key:{}:{}",
            api_key_id,
            provider_id.as_str()
        )
    }

    fn rr_key(&self, api_key_id: &ApiKeyId, provider_id: &ProviderId) -> String {
        format!(
            "zorch:target_key_rr:{}:{}",
            api_key_id,
            provider_id.as_str()
        )
    }

    /// Atomically increment and return the round-robin index modulo `key_count`.
    /// The counter shares the same TTL as sticky mappings.
    pub async fn next_key_index(
        &self,
        api_key_id: &ApiKeyId,
        provider_id: &ProviderId,
        key_count: usize,
    ) -> Result<usize, zorch_shared::AppError> {
        if key_count == 0 {
            return Ok(0);
        }

        let mut conn = self.client.get_multiplexed_async_connection().await.map_err(|e| {
            zorch_shared::AppError::Internal(format!(
                "Failed to get Redis connection for sticky target key cache: {}",
                e
            ))
        })?;

        let key = self.rr_key(api_key_id, provider_id);
        let count: u64 = redis::cmd("INCR")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                zorch_shared::AppError::Internal(format!(
                    "Failed to increment target key round-robin counter: {}",
                    e
                ))
            })?;

        // Set TTL on the counter the first time it is created.
        if count == 1 {
            redis::cmd("EXPIRE")
                .arg(&key)
                .arg(self.ttl_secs)
                .query_async::<_, ()>(&mut conn)
                .await
                .map_err(|e| {
                    zorch_shared::AppError::Internal(format!(
                        "Failed to set round-robin counter TTL: {}",
                        e
                    ))
                })?;
        }

        Ok(((count - 1) as usize) % key_count)
    }

    /// Look up the previously chosen provider API key for the given client key
    /// and provider. Returns `None` if no mapping exists.
    pub async fn get(
        &self,
        api_key_id: &ApiKeyId,
        provider_id: &ProviderId,
    ) -> Result<Option<ProviderApiKeyId>, zorch_shared::AppError> {
        let mut conn = self.client.get_multiplexed_async_connection().await.map_err(|e| {
            zorch_shared::AppError::Internal(format!(
                "Failed to get Redis connection for sticky target key cache: {}",
                e
            ))
        })?;

        let value: Option<String> = conn.get(self.key(api_key_id, provider_id)).await.map_err(|e| {
            zorch_shared::AppError::Internal(format!(
                "Failed to read sticky target key from Redis: {}",
                e
            ))
        })?;

        match value {
            Some(v) => match v.parse::<uuid::Uuid>() {
                Ok(uuid) => Ok(Some(ProviderApiKeyId::from_uuid(uuid))),
                Err(e) => {
                    tracing::warn!(value = %v, error = %e, "Invalid sticky target key id in Redis");
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    /// Store a sticky mapping from client key + provider to the chosen target
    /// API key, refreshing the TTL.
    pub async fn set(
        &self,
        api_key_id: &ApiKeyId,
        provider_id: &ProviderId,
        provider_api_key_id: &ProviderApiKeyId,
    ) -> Result<(), zorch_shared::AppError> {
        let mut conn = self.client.get_multiplexed_async_connection().await.map_err(|e| {
            zorch_shared::AppError::Internal(format!(
                "Failed to get Redis connection for sticky target key cache: {}",
                e
            ))
        })?;

        let key = self.key(api_key_id, provider_id);
        conn.set::<_, _, ()>(&key, provider_api_key_id.to_string())
            .await
            .map_err(|e| {
                zorch_shared::AppError::Internal(format!(
                    "Failed to write sticky target key to Redis: {}",
                    e
                ))
            })?;

        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(self.ttl_secs)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| {
                zorch_shared::AppError::Internal(format!(
                    "Failed to set sticky target key TTL: {}",
                    e
                ))
            })?;

        Ok(())
    }

    /// Remove a sticky mapping. Used when the chosen key is known to be bad.
    pub async fn invalidate(
        &self,
        api_key_id: &ApiKeyId,
        provider_id: &ProviderId,
    ) -> Result<(), zorch_shared::AppError> {
        let mut conn = self.client.get_multiplexed_async_connection().await.map_err(|e| {
            zorch_shared::AppError::Internal(format!(
                "Failed to get Redis connection for sticky target key cache: {}",
                e
            ))
        })?;

        conn.del::<_, ()>(self.key(api_key_id, provider_id))
            .await
            .map_err(|e| {
                zorch_shared::AppError::Internal(format!(
                    "Failed to invalidate sticky target key: {}",
                    e
                ))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests require a running Redis instance. They are guarded behind
    // the `redis` feature in CI; running them locally without Redis will fail.
    fn cache() -> StickyTargetKeyCache {
        let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
        StickyTargetKeyCache::new(client, 60).unwrap()
    }

    #[tokio::test]
    #[ignore = "requires redis"]
    async fn test_get_missing_returns_none() {
        let cache = cache();
        let api_key_id = ApiKeyId::new();
        let provider_id = ProviderId::from("openai");
        let result = cache.get(&api_key_id, &provider_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore = "requires redis"]
    async fn test_set_and_get() {
        let cache = cache();
        let api_key_id = ApiKeyId::new();
        let provider_id = ProviderId::from("openai");
        let target_key_id = ProviderApiKeyId::new();

        cache
            .set(&api_key_id, &provider_id, &target_key_id)
            .await
            .unwrap();
        let result = cache.get(&api_key_id, &provider_id).await.unwrap();
        assert_eq!(result, Some(target_key_id));
    }

    #[tokio::test]
    #[ignore = "requires redis"]
    async fn test_invalidate() {
        let cache = cache();
        let api_key_id = ApiKeyId::new();
        let provider_id = ProviderId::from("openai");
        let target_key_id = ProviderApiKeyId::new();

        cache
            .set(&api_key_id, &provider_id, &target_key_id)
            .await
            .unwrap();
        cache.invalidate(&api_key_id, &provider_id).await.unwrap();
        let result = cache.get(&api_key_id, &provider_id).await.unwrap();
        assert!(result.is_none());
    }
}
