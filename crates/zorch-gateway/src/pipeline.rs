//! Request pipeline middleware for governance, rate limiting, and circuit breaker.

use zorch_shared::{ApiKeyId, AppError, ModelId, ProviderId};

use crate::{
    CircuitBreaker, GovernanceDecision, GovernanceEngine, KeyLimitConfig, KeyLimits, RateLimiter,
};

/// Result of a pipeline check
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineResult {
    pub api_key_id: ApiKeyId,
    pub provider_id: ProviderId,
    pub model_id: ModelId,
}

/// Request pipeline that orchestrates governance, rate limiting, and circuit breaker checks
pub struct RequestPipeline<'a> {
    rate_limiter: &'a RateLimiter,
    key_limits: &'a KeyLimits,
    governance: &'a GovernanceEngine,
    circuit_breaker: &'a CircuitBreaker,
}

impl<'a> RequestPipeline<'a> {
    /// Create a new request pipeline with the required dependencies
    pub fn new(
        rate_limiter: &'a RateLimiter,
        key_limits: &'a KeyLimits,
        governance: &'a GovernanceEngine,
        circuit_breaker: &'a CircuitBreaker,
    ) -> Self {
        Self {
            rate_limiter,
            key_limits,
            governance,
            circuit_breaker,
        }
    }

    /// Execute the full pipeline: rate limit → key limits → governance → circuit breaker
    ///
    /// Returns `Ok(PipelineResult)` if all checks pass, or `Err(AppError)` if any check fails.
    pub async fn execute(
        &self,
        api_key_id: &ApiKeyId,
        provider_id: &ProviderId,
        model_id: &ModelId,
        estimated_tokens: u32,
        key_config: &KeyLimitConfig,
    ) -> Result<PipelineResult, AppError> {
        let api_key_id_str = api_key_id.to_string();
        let model_str = model_id.as_str();

        if !self
            .rate_limiter
            .check_rate_limit(&api_key_id_str, model_str, 60, 100)
            .await?
        {
            return Err(AppError::RateLimit("Rate limit exceeded".to_string()));
        }

        self.key_limits
            .check_limits(&api_key_id_str, model_str, key_config)
            .await?;

        let governance_result = self
            .governance
            .check_request(api_key_id.clone(), provider_id, model_id, estimated_tokens)
            .await?;

        match governance_result {
            GovernanceDecision::Block { reason } => {
                return Err(AppError::BadRequest(format!(
                    "Request blocked by governance policy: {}",
                    reason
                )));
            }
            GovernanceDecision::SoftLimit { alert } => {
                tracing::warn!(
                    "Request exceeded soft limit for api_key={}, provider={}, model={}",
                    api_key_id_str,
                    provider_id.as_str(),
                    model_str
                );
                tracing::warn!("Alert: {}", alert);
            }
            GovernanceDecision::Allow => {}
        }

        if !self
            .circuit_breaker
            .is_provider_healthy(provider_id)
            .await?
        {
            return Err(AppError::Provider(format!(
                "Provider '{}' is currently unavailable (circuit breaker open)",
                provider_id
            )));
        }

        Ok(PipelineResult {
            api_key_id: api_key_id.clone(),
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_result_creation() {
        let api_key_id = ApiKeyId::new();
        let provider_id = ProviderId::from("openai");
        let model_id = ModelId::from("gpt-4");

        let result = PipelineResult {
            api_key_id: api_key_id.clone(),
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
        };

        assert_eq!(result.api_key_id, api_key_id);
        assert_eq!(result.provider_id, provider_id);
        assert_eq!(result.model_id, model_id);
    }
}
