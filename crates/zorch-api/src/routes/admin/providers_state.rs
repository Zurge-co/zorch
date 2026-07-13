//! Helpers shared by provider-mutating admin handlers.
//!
//! Owns single-responsibility helpers for keeping in-memory state consistent
//! with Postgres after provider, target model, or API key mutations.

use crate::AppState;

pub(crate) async fn reload_provider_state(state: &AppState) -> Result<(), zorch_shared::AppError> {
    let http_client = zorch_providers::ProviderHttpClient::new(state.config.timeout_duration())
        .map_err(|e| {
            zorch_shared::AppError::Internal(format!("Failed to create HTTP client: {}", e))
        })?;
    crate::server::providers::reload_providers_and_models(
        &state.proxy_providers,
        &state.model_resolver,
        &state.config,
        &state.db_pool,
        &http_client,
        &state.vault,
    )
    .await?;
    state.model_cache.invalidate_all().await?;
    Ok(())
}
