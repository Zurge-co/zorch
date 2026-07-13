use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use arc_swap::ArcSwap;
use sqlx::{PgPool, Row};
use tracing::info;
use zorch_providers::{
    AuthType, ModelResolver, ProviderApiKey, ProviderHttpClient, ProxyProvider,
    ProxyProviderRegistry, Target,
};
use zorch_shared::{
    AppConfig, AppError, BackendId, ModelId, ProviderApiKeyId, ProviderId, SecretVault,
};

fn new_registry_swap(registry: ProxyProviderRegistry) -> Arc<ArcSwap<ProxyProviderRegistry>> {
    Arc::new(ArcSwap::new(Arc::new(registry)))
}

fn new_resolver_swap(resolver: ModelResolver) -> Arc<ArcSwap<ModelResolver>> {
    Arc::new(ArcSwap::new(Arc::new(resolver)))
}

/// Loads active providers, their target models, and their target API keys from
/// the database and builds the runtime registry + resolver.
pub async fn load_providers_and_models(
    cfg: &AppConfig,
    db_pool: &PgPool,
    http_client: &ProviderHttpClient,
    vault: &SecretVault,
) -> Result<(ProxyProviderRegistry, ModelResolver), AppError> {
    let provider_rows = sqlx::query(
        "SELECT id, name, base_url, auth_type, auth_header_name, auth_prefix \
         FROM providers WHERE is_active = true",
    )
    .fetch_all(db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load providers from DB: {}", e)))?;

    if provider_rows.is_empty() {
        return Ok(register_env_fallback(cfg, http_client, vault));
    }

    // Load API keys for all active providers in one query.
    let key_rows = sqlx::query(
        "SELECT id, provider_id, encrypted_key \
         FROM provider_api_keys \
         WHERE is_active = true",
    )
    .fetch_all(db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load provider API keys: {}", e)))?;

    // Load target models for all active providers in one query.
    let target_model_rows = sqlx::query(
        "SELECT id, provider_id, target_model \
         FROM provider_target_models \
         WHERE is_active = true",
    )
    .fetch_all(db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load provider target models: {}", e)))?;

    let mut provider_builds: HashMap<uuid::Uuid, ProviderBuildInfo> = HashMap::new();
    for row in provider_rows {
        let id: uuid::Uuid = row
            .try_get("id")
            .map_err(|e| AppError::Internal(format!("Missing provider id: {}", e)))?;
        let name: String = row
            .try_get("name")
            .map_err(|e| AppError::Internal(format!("Missing provider name: {}", e)))?;
        let base_url: String = row
            .try_get("base_url")
            .map_err(|e| AppError::Internal(format!("Missing provider base_url: {}", e)))?;
        let auth_type: String = row
            .try_get("auth_type")
            .map_err(|e| AppError::Internal(format!("Missing provider auth_type: {}", e)))?;
        let auth_header_name: Option<String> = row.try_get("auth_header_name").ok();
        let auth_prefix: Option<String> = row.try_get("auth_prefix").ok();

        let auth_type = AuthType::from_config(
            &auth_type,
            auth_header_name.as_deref(),
            auth_prefix.as_deref(),
        )
        .map_err(|e| {
            AppError::Internal(format!("Invalid auth_type for provider '{}': {}", name, e))
        })?;

        provider_builds.insert(
            id,
            ProviderBuildInfo {
                id,
                name,
                base_url,
                auth_type,
                keys: Vec::new(),
                target_models: HashSet::new(),
            },
        );
    }

    for row in key_rows {
        let id: uuid::Uuid = row
            .try_get("id")
            .map_err(|e| AppError::Internal(format!("Missing provider_api_key id: {}", e)))?;
        let provider_id: uuid::Uuid = row.try_get("provider_id").map_err(|e| {
            AppError::Internal(format!("Missing provider_api_key provider_id: {}", e))
        })?;
        let encrypted_key: String = row.try_get("encrypted_key").map_err(|e| {
            AppError::Internal(format!("Missing provider_api_key encrypted_key: {}", e))
        })?;

        if let Some(build) = provider_builds.get_mut(&provider_id) {
            build.keys.push(ProviderApiKey::new(
                ProviderApiKeyId::from_uuid(id),
                encrypted_key,
            ));
        }
    }

    for row in target_model_rows {
        let provider_id: uuid::Uuid = row.try_get("provider_id").map_err(|e| {
            AppError::Internal(format!("Missing provider_target_model provider_id: {}", e))
        })?;
        let target_model: String = row.try_get("target_model").map_err(|e| {
            AppError::Internal(format!("Missing provider_target_model target_model: {}", e))
        })?;

        if let Some(build) = provider_builds.get_mut(&provider_id) {
            build.target_models.insert(target_model);
        }
    }

    let mut resolver = ModelResolver::new();

    let alias_rows = sqlx::query(
        "SELECT m.public_name, p.name AS provider_name, ptm.target_model, mt.priority \
         FROM model_targets mt \
         JOIN models m ON mt.model_id = m.id \
         JOIN provider_target_models ptm ON mt.provider_target_model_id = ptm.id \
         JOIN providers p ON ptm.provider_id = p.id \
         WHERE p.is_active = true AND m.is_active = true AND mt.is_active = true AND ptm.is_active = true",
    )
    .fetch_all(db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load alias model targets: {}", e)))?;

    for row in alias_rows {
        let public_name: String = row.try_get("public_name").map_err(|e| {
            AppError::Internal(format!("Missing public_name in alias target: {}", e))
        })?;
        let provider_name: String = row.try_get("provider_name").map_err(|e| {
            AppError::Internal(format!("Missing provider_name in alias target: {}", e))
        })?;
        let target_model: String = row.try_get("target_model").map_err(|e| {
            AppError::Internal(format!("Missing target_model in alias target: {}", e))
        })?;
        let priority: i32 = row
            .try_get("priority")
            .map_err(|e| AppError::Internal(format!("Missing priority in alias target: {}", e)))?;

        resolver.add_target(
            ModelId::from(public_name.as_str()),
            Target {
                provider_id: ProviderId::from(provider_name.as_str()),
                target_model: ModelId::from(target_model.as_str()),
                priority,
            },
        );
    }

    let mut registry = ProxyProviderRegistry::new();
    for build in provider_builds.into_values() {
        if build.target_models.is_empty() {
            tracing::warn!(
                "Provider '{}' has no active target models, skipping",
                build.name
            );
            continue;
        }
        if build.keys.is_empty() {
            tracing::warn!("Provider '{}' has no active API keys, skipping", build.name);
            continue;
        }

        let backend_id = BackendId::from_uuid(build.id);
        let provider = ProxyProvider::new(
            backend_id,
            ProviderId::from(build.name.as_str()),
            build.base_url,
            build.keys,
            vault.clone(),
            build.target_models.into_iter().collect(),
            http_client.clone(),
        )
        .with_auth_type(build.auth_type);

        let key_count = provider.key_count();
        registry.register(provider);
        info!(
            "Registered proxy provider '{}' ({}) with {} key(s)",
            build.name, build.id, key_count
        );
    }

    Ok((registry, resolver))
}

pub async fn reload_providers_and_models(
    registry_swap: &ArcSwap<ProxyProviderRegistry>,
    resolver_swap: &ArcSwap<ModelResolver>,
    cfg: &AppConfig,
    db_pool: &PgPool,
    http_client: &ProviderHttpClient,
    vault: &SecretVault,
) -> Result<(), AppError> {
    let (registry, resolver) = load_providers_and_models(cfg, db_pool, http_client, vault).await?;
    let provider_count = registry.list().len();
    let model_count = resolver.public_models().len();
    registry_swap.store(Arc::new(registry));
    resolver_swap.store(Arc::new(resolver));
    info!(
        "Hot-reloaded {} proxy provider(s) and {} public model(s)",
        provider_count, model_count
    );
    Ok(())
}

pub async fn register_providers_and_models(
    cfg: &AppConfig,
    db_pool: &PgPool,
    http_client: &ProviderHttpClient,
    vault: &SecretVault,
) -> Result<
    (
        Arc<ArcSwap<ProxyProviderRegistry>>,
        Arc<ArcSwap<ModelResolver>>,
    ),
    AppError,
> {
    let (registry, resolver) = load_providers_and_models(cfg, db_pool, http_client, vault).await?;
    Ok((new_registry_swap(registry), new_resolver_swap(resolver)))
}

struct ProviderBuildInfo {
    id: uuid::Uuid,
    name: String,
    base_url: String,
    auth_type: AuthType,
    keys: Vec<ProviderApiKey>,
    target_models: HashSet<String>,
}

/// Fallback providers when the DB has no active providers.
fn register_env_fallback(
    cfg: &AppConfig,
    http_client: &ProviderHttpClient,
    vault: &SecretVault,
) -> (ProxyProviderRegistry, ModelResolver) {
    let mut registry = ProxyProviderRegistry::new();
    let mut resolver = ModelResolver::new();

    if let Some(ref key) = cfg.openai_api_key {
        let encrypted = match vault.encrypt(key) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to encrypt OpenAI key: {}", e);
                return (registry, resolver);
            }
        };
        let backend_id = BackendId::new();
        let provider_id = ProviderId::from("openai");
        let target_models = vec!["gpt-4".to_string(), "gpt-4o".to_string()];

        let provider = ProxyProvider::new(
            backend_id,
            provider_id.clone(),
            "https://api.openai.com/v1".to_string(),
            vec![ProviderApiKey::new(ProviderApiKeyId::new(), encrypted)],
            vault.clone(),
            target_models.clone(),
            http_client.clone(),
        )
        .with_auth_type(AuthType::Bearer);
        registry.register(provider);
        info!("Registered OpenAI provider from env fallback");

        for model in target_models {
            resolver.add_target(
                ModelId::from(model.as_str()),
                Target {
                    provider_id: provider_id.clone(),
                    target_model: ModelId::from(model.as_str()),
                    priority: 0,
                },
            );
        }
    }

    if let Some(ref key) = cfg.anthropic_api_key {
        let encrypted = match vault.encrypt(key) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to encrypt Anthropic key: {}", e);
                return (registry, resolver);
            }
        };
        let backend_id = BackendId::new();
        let provider_id = ProviderId::from("anthropic");
        let target_models = vec!["claude-3-5-sonnet".to_string(), "claude-3-opus".to_string()];

        let provider = ProxyProvider::new(
            backend_id,
            provider_id.clone(),
            "https://api.anthropic.com/v1".to_string(),
            vec![ProviderApiKey::new(ProviderApiKeyId::new(), encrypted)],
            vault.clone(),
            target_models.clone(),
            http_client.clone(),
        )
        .with_auth_type(AuthType::Anthropic);
        registry.register(provider);
        info!("Registered Anthropic provider from env fallback");

        for model in target_models {
            resolver.add_target(
                ModelId::from(model.as_str()),
                Target {
                    provider_id: provider_id.clone(),
                    target_model: ModelId::from(model.as_str()),
                    priority: 0,
                },
            );
        }
    }

    (registry, resolver)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_cfg() -> AppConfig {
        AppConfig {
            database_url: "postgres://localhost/test".to_string(),
            clickhouse_url: String::new(),
            redis_url: String::new(),
            app_port: 8080,
            rust_log: "info".to_string(),
            encryption_key: "test-key".to_string(),
            inspector_capture_level: "metadata_only".to_string(),
            timeout_secs: 60,
            circuit_breaker_timeout_secs: 30,
            openai_api_key: None,
            anthropic_api_key: None,
            admin_secret: None,
            default_org_id: None,
            cors_allowed_origins: Vec::new(),
            enforce_per_key_governance: true,
            sticky_target_key_ttl_secs: Some(300),
        }
    }

    #[test]
    fn test_env_fallback_registers_resolver_targets() {
        let cfg = AppConfig {
            openai_api_key: Some("sk-test".to_string()),
            ..empty_cfg()
        };
        let http_client = ProviderHttpClient::new(std::time::Duration::from_secs(1)).unwrap();
        let vault = SecretVault::new("test-key").unwrap();

        let (registry, resolver) = register_env_fallback(&cfg, &http_client, &vault);

        assert!(!registry.is_empty());
        let targets = resolver.resolve(&ModelId::from("gpt-4"));
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].provider_id, ProviderId::from("openai"));
        assert_eq!(targets[0].target_model, ModelId::from("gpt-4"));
    }
}
