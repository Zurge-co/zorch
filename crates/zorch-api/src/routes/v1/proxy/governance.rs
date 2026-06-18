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
        &state.circuit_breaker,
    );

    let estimated_tokens = 1000;
    let _result = pipeline
        .execute(
            &ctx.api_key_id,
            &ctx.provider_id,
            &ctx.model_id,
            estimated_tokens,
            &KeyLimitConfig::default(),
        )
        .await?;
    Ok(())
}
