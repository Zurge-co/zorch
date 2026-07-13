use axum::body::Bytes;
use zorch_gateway::KeyLimitConfig;
use zorch_shared::AppError;

use super::RequestContext;
use crate::AppState;

pub async fn run_governance_pipeline(
    state: &AppState,
    ctx: &RequestContext,
    _body: &Bytes,
) -> Result<(), AppError> {
    let pipeline = zorch_gateway::RequestPipeline::new(
        &state.rate_limiter,
        &state.key_limits,
        &state.governance,
    );

    let estimated_tokens = 1000;

    let key_config = if state.config.enforce_per_key_governance {
        KeyLimitConfig {
            requests_per_minute: ctx
                .api_key
                .requests_per_minute
                .map(|v| v as u64)
                .unwrap_or(100),
            requests_per_day: ctx
                .api_key
                .requests_per_day
                .map(|v| v as u64)
                .unwrap_or(10_000),
            max_spend_usd: ctx.api_key.max_spend_usd.unwrap_or(f64::MAX),
            allowed_models: ctx.api_key.allowed_models.clone().unwrap_or_default(),
        }
    } else {
        KeyLimitConfig::default()
    };

    let _result = pipeline
        .execute(
            &ctx.api_key_id,
            &ctx.provider_id,
            &ctx.public_model_id,
            estimated_tokens,
            &key_config,
        )
        .await?;
    Ok(())
}
