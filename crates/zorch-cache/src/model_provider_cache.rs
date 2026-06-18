use redis::AsyncCommands;
use zorch_shared::{AppError, ProviderId};

/// Redis-backed cache for model → provider_id lookups.
///
/// Stores mappings in a single hash (`zorch:model_providers`) with a TTL.
/// The cache is invalidated explicitly when providers change via the admin API.
/// It intentionally stores only provider identifiers, never API keys.
pub struct ModelProviderCache {
    client: redis::Client,
    ttl_secs: u64,
}

const KEY: &str = "zorch:model_providers";

impl ModelProviderCache {
    /// Create a new cache with an existing Redis client.
    ///
    /// # Arguments
    /// * `client` - Redis/Dragonfly client
    /// * `ttl_secs` - Time-to-live for the cached hash in seconds
    pub fn new(client: redis::Client, ttl_secs: u64) -> Result<Self, AppError> {
        Ok(Self { client, ttl_secs })
    }

    /// Look up the provider id for a model.
    pub async fn get(&self, model: &str) -> Result<Option<ProviderId>, AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                AppError::Internal(format!(
                    "Failed to get Redis connection for model cache: {}",
                    e
                ))
            })?;

        let value: Option<String> = conn.hget(KEY, model).await.map_err(|e| {
            AppError::Internal(format!("Failed to read model cache from Redis: {}", e))
        })?;

        Ok(value.map(ProviderId::from))
    }

    /// Store a model → provider_id mapping and refresh the hash TTL.
    pub async fn set(&self, model: &str, provider_id: &ProviderId) -> Result<(), AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                AppError::Internal(format!(
                    "Failed to get Redis connection for model cache: {}",
                    e
                ))
            })?;

        conn.hset::<_, _, _, ()>(KEY, model, provider_id.as_str())
            .await
            .map_err(|e| {
                AppError::Internal(format!("Failed to write model cache to Redis: {}", e))
            })?;

        redis::cmd("EXPIRE")
            .arg(KEY)
            .arg(self.ttl_secs)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to set model cache TTL: {}", e)))?;

        Ok(())
    }

    /// Remove a single model mapping.
    pub async fn invalidate(&self, model: &str) -> Result<(), AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                AppError::Internal(format!(
                    "Failed to get Redis connection for model cache: {}",
                    e
                ))
            })?;

        conn.hdel::<_, _, ()>(KEY, model).await.map_err(|e| {
            AppError::Internal(format!("Failed to invalidate model cache entry: {}", e))
        })?;

        Ok(())
    }

    /// Clear all model mappings. Used when provider configuration changes in a
    /// way that may affect unknown models (update/delete).
    pub async fn invalidate_all(&self) -> Result<(), AppError> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                AppError::Internal(format!(
                    "Failed to get Redis connection for model cache: {}",
                    e
                ))
            })?;

        conn.del::<_, ()>(KEY)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to clear model cache: {}", e)))?;

        Ok(())
    }
}
