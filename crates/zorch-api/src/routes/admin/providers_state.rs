//! Helpers shared by provider-mutating admin handlers.
//!
//! Owns two single-responsibility helpers that would otherwise be duplicated
//! across every provider INSERT / UPDATE / DELETE handler:
//!
//! - [`reload_provider_state`] — keep the in-memory `proxy_providers` cache
//!   consistent with Postgres after any provider-row mutation, and invalidate
//!   `model_cache` so the next request rebuilds it.
//!
//! - [`merge_provider_config`] — fold an admin request's `api_key` (which
//!   must be encrypted-at-rest) and `models` list into the persisted JSON
//!   `config` column, defensively dropping any stale `api_keys_encrypted`
//!   (plural) key the client may have sent.

use zorch_providers::Protocol;
use zorch_shared::SecretVault;

use crate::AppState;

pub async fn reload_provider_state(state: &AppState) -> Result<(), zorch_shared::AppError> {
    let http_client = zorch_providers::ProviderHttpClient::new(state.config.timeout_duration())
        .map_err(|e| {
            zorch_shared::AppError::Internal(format!("Failed to create HTTP client: {}", e))
        })?;
    let _ = crate::server::providers::reload_providers(
        &state.proxy_providers,
        &state.config,
        &state.db_pool,
        &http_client,
        &state.vault,
    )
    .await;
    state.model_cache.invalidate_all().await?;
    Ok(())
}

pub fn merge_provider_config(
    vault: &SecretVault,
    raw_config: serde_json::Value,
    api_key: Option<&str>,
    models: &[String],
    protocol: Protocol,
) -> Result<serde_json::Value, zorch_shared::AppError> {
    let mut config = if raw_config.is_null() {
        serde_json::json!({})
    } else {
        raw_config
    };

    if let Some(key) = api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            let encrypted = vault.encrypt(trimmed).map_err(|e| {
                zorch_shared::AppError::Internal(format!(
                    "Failed to encrypt provider API key: {}",
                    e
                ))
            })?;
            config["api_key_encrypted"] = serde_json::Value::String(encrypted);
        }
    }

    config
        .as_object_mut()
        .map(|obj| obj.remove("api_keys_encrypted"));
    config["protocol"] = serde_json::Value::String(protocol.as_str().to_string());

    config["models"] = serde_json::Value::Array(
        models
            .iter()
            .map(|m| serde_json::Value::String(m.clone()))
            .collect(),
    );

    Ok(config)
}
