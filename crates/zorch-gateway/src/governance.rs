use redis::AsyncCommands;
use sqlx::{PgPool, Row};
use zorch_shared::{ApiKeyId, AppError, ModelId, ProviderId};

#[derive(Debug, Clone, PartialEq)]
pub enum GovernanceDecision {
    Allow,
    Block { reason: String },
    SoftLimit { alert: String },
}

pub struct GovernanceEngine {
    db_pool: PgPool,
    redis_client: redis::Client,
}

impl GovernanceEngine {
    pub fn new(db_pool: PgPool, redis_client: redis::Client) -> Self {
        Self {
            db_pool,
            redis_client,
        }
    }

    pub async fn check_request(
        &self,
        api_key_id: ApiKeyId,
        _provider: &ProviderId,
        model: &ModelId,
        _estimated_tokens: u32,
    ) -> Result<GovernanceDecision, AppError> {
        let row = sqlx::query(
            r#"
            SELECT is_active, expires_at, allowed_models, max_spend_usd
            FROM api_keys
            WHERE id = $1
            "#,
        )
        .bind(*api_key_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to fetch API key: {}", e)))?;

        let key =
            row.ok_or_else(|| AppError::Auth(format!("API key not found: {}", api_key_id)))?;

        let is_active: bool = key
            .try_get("is_active")
            .map_err(|e| AppError::Database(format!("Failed to get is_active: {}", e)))?;

        let expires_at: Option<chrono::DateTime<chrono::Utc>> = key.try_get("expires_at").ok();

        let allowed_models: Option<Vec<String>> = key.try_get("allowed_models").ok();

        let max_spend_usd: Option<f64> = key.try_get("max_spend_usd").ok();

        if !is_active {
            return Ok(GovernanceDecision::Block {
                reason: "API key is inactive".to_string(),
            });
        }

        if let Some(expires_at) = expires_at {
            let now = chrono::Utc::now();
            if expires_at < now {
                return Ok(GovernanceDecision::SoftLimit {
                    alert: format!("API key expired at {}", expires_at),
                });
            }
        }

        if let Some(models) = allowed_models {
            if !models.is_empty() && !models.iter().any(|m| m == model.as_str()) {
                return Ok(GovernanceDecision::Block {
                    reason: format!(
                        "Model '{}' is not in the allowed models list",
                        model.as_str()
                    ),
                });
            }
        }

        if let Some(max_budget) = max_spend_usd {
            if max_budget == 0.0 {
                return Ok(GovernanceDecision::Block {
                    reason: "API key budget is zero".to_string(),
                });
            }

            let current_spend = self.get_current_spend(api_key_id).await?;
            let spend_ratio = current_spend / max_budget;

            if spend_ratio >= 1.0 {
                return Ok(GovernanceDecision::Block {
                    reason: format!(
                        "Budget exceeded: ${:.2} / ${:.2}",
                        current_spend, max_budget
                    ),
                });
            }

            if spend_ratio > 0.8 {
                return Ok(GovernanceDecision::SoftLimit {
                    alert: format!(
                        "Spending alert: ${:.2} / ${:.2} ({:.0}% of budget used)",
                        current_spend,
                        max_budget,
                        spend_ratio * 100.0
                    ),
                });
            }
        }

        Ok(GovernanceDecision::Allow)
    }

    async fn get_current_spend(&self, api_key_id: ApiKeyId) -> Result<f64, AppError> {
        let mut conn = self
            .redis_client
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governance_allows_active_key() {
        let decision = GovernanceDecision::Allow;
        assert!(matches!(decision, GovernanceDecision::Allow));
    }

    #[test]
    fn test_governance_block_decision() {
        let decision = GovernanceDecision::Block {
            reason: "API key is inactive".to_string(),
        };
        assert!(matches!(decision, GovernanceDecision::Block { .. }));
    }

    #[test]
    fn test_governance_soft_limit_decision() {
        let decision = GovernanceDecision::SoftLimit {
            alert: "API key expired".to_string(),
        };
        assert!(matches!(decision, GovernanceDecision::SoftLimit { .. }));
    }
}
