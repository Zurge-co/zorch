use std::sync::Arc;

use arc_swap::ArcSwap;
use sqlx::{PgPool, Row};
use tracing::info;
use zorch_providers::{Protocol, ProviderHttpClient, ProxyProvider, ProxyProviderRegistry};
use zorch_shared::{AppConfig, AppError, ProviderId, SecretVault};

pub fn new_registry_swap(registry: ProxyProviderRegistry) -> Arc<ArcSwap<ProxyProviderRegistry>> {
    Arc::new(ArcSwap::new(Arc::new(registry)))
}

pub async fn load_providers(
    cfg: &AppConfig,
    db_pool: &PgPool,
    http_client: &ProviderHttpClient,
    vault: &SecretVault,
) -> Result<ProxyProviderRegistry, AppError> {
    let mut registry = ProxyProviderRegistry::new();

    let rows = sqlx::query("SELECT name, base_url, config FROM providers WHERE is_active = true")
        .fetch_all(db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to load providers from DB: {}", e)))?;

    if rows.is_empty() {
        return register_env_fallback(cfg, http_client, vault);
    }

    for row in rows {
        let name: String = row
            .try_get("name")
            .map_err(|e| AppError::Internal(format!("Missing provider name: {}", e)))?;
        let base_url: String = row
            .try_get("base_url")
            .map_err(|e| AppError::Internal(format!("Missing provider base_url: {}", e)))?;
        let config: serde_json::Value = row.try_get("config").unwrap_or(serde_json::Value::Null);

        let models = extract_models(&config);
        let protocol = extract_protocol(&config)?;
        let keys = resolve_provider_keys(&name, &config, cfg, vault)?;

        if keys.is_empty() {
            tracing::warn!("Provider '{}' has no API key, skipping", name);
            continue;
        }

        let mut provider = ProxyProvider::new(
            ProviderId::from(name.as_str()),
            base_url,
            keys[0].clone(),
            vault.clone(),
            models,
            http_client.clone(),
        )
        .with_protocol(protocol);

        for key in keys.into_iter().skip(1) {
            provider.add_key(key);
        }

        let key_count = provider.key_count();
        registry.register(ProviderId::from(name.as_str()), provider);
        info!(
            "Registered proxy provider '{}' with {} key(s)",
            name, key_count
        );
    }

    Ok(registry)
}

pub async fn reload_providers(
    swap: &ArcSwap<ProxyProviderRegistry>,
    cfg: &AppConfig,
    db_pool: &PgPool,
    http_client: &ProviderHttpClient,
    vault: &SecretVault,
) -> Result<usize, AppError> {
    let registry = load_providers(cfg, db_pool, http_client, vault).await?;
    let count = registry.list().len();
    swap.store(Arc::new(registry));
    info!("Hot-reloaded {} proxy provider(s)", count);
    Ok(count)
}

pub async fn register_providers(
    cfg: &AppConfig,
    db_pool: &PgPool,
    http_client: &ProviderHttpClient,
    vault: &SecretVault,
) -> Result<Arc<ArcSwap<ProxyProviderRegistry>>, AppError> {
    let registry = load_providers(cfg, db_pool, http_client, vault).await?;
    Ok(new_registry_swap(registry))
}

fn extract_models(config: &serde_json::Value) -> Vec<String> {
    config
        .get("models")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_protocol(config: &serde_json::Value) -> Result<Protocol, AppError> {
    let Some(protocol) = config.get("protocol").and_then(|v| v.as_str()) else {
        return Ok(Protocol::default());
    };

    protocol.parse().map_err(|_| {
        AppError::BadRequest(format!(
            "Invalid provider protocol '{}'. Supported protocols: openai_compatible, anthropic",
            protocol
        ))
    })
}

fn extract_encrypted_keys(config: &serde_json::Value) -> Vec<String> {
    let single = config
        .get("api_key_encrypted")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let mut keys: Vec<String> = config
        .get("api_keys_encrypted")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if let Some(key) = single {
        if keys.is_empty() {
            keys.push(key);
        }
    }

    keys
}

fn register_env_fallback(
    cfg: &AppConfig,
    http_client: &ProviderHttpClient,
    vault: &SecretVault,
) -> Result<ProxyProviderRegistry, AppError> {
    let mut registry = ProxyProviderRegistry::new();

    if let Some(ref key) = cfg.openai_api_key {
        let encrypted = vault
            .encrypt(key)
            .map_err(|e| AppError::Internal(format!("Failed to encrypt OpenAI key: {}", e)))?;
        let provider = ProxyProvider::new(
            ProviderId::from("openai"),
            "https://api.openai.com/v1".to_string(),
            encrypted,
            vault.clone(),
            vec!["gpt-4".to_string(), "gpt-4o".to_string()],
            http_client.clone(),
        )
        .with_protocol(Protocol::OpenAICompatible);
        registry.register(ProviderId::from("openai"), provider);
        info!("Registered OpenAI provider from env fallback");
    }

    if let Some(ref key) = cfg.anthropic_api_key {
        let encrypted = vault
            .encrypt(key)
            .map_err(|e| AppError::Internal(format!("Failed to encrypt Anthropic key: {}", e)))?;
        let provider = ProxyProvider::new(
            ProviderId::from("anthropic"),
            "https://api.anthropic.com/v1".to_string(),
            encrypted,
            vault.clone(),
            vec!["claude-3-5-sonnet".to_string(), "claude-3-opus".to_string()],
            http_client.clone(),
        )
        .with_protocol(Protocol::Anthropic);
        registry.register(ProviderId::from("anthropic"), provider);
        info!("Registered Anthropic provider from env fallback");
    }

    Ok(registry)
}

fn resolve_provider_keys(
    name: &str,
    config: &serde_json::Value,
    cfg: &AppConfig,
    vault: &SecretVault,
) -> Result<Vec<String>, AppError> {
    let encrypted = extract_encrypted_keys(config);

    let mut validated: Vec<String> = Vec::with_capacity(encrypted.len());
    for (idx, blob) in encrypted.iter().enumerate() {
        let is_valid = match vault.decrypt(blob) {
            Ok(_) => true,
            Err(e) => {
                let detail = match &e {
                    AppError::Internal(s) => s.as_str(),
                    _ => "unknown vault error",
                };
                tracing::error!(
                    provider = %name,
                    key_index = idx,
                    error = %detail,
                    "Skipping an undecryptable upstream API key for this provider. \
                     Other (healthy) keys on this provider remain in use; \
                     if all keys fail this provider will be skipped entirely."
                );
                false
            }
        };
        if is_valid {
            validated.push(blob.clone());
        }
    }

    if validated.is_empty() {
        let env_key = match name.to_lowercase().as_str() {
            "openai" => cfg.openai_api_key.as_ref(),
            "anthropic" => cfg.anthropic_api_key.as_ref(),
            _ => None,
        };
        if let Some(key) = env_key {
            let encrypted = vault.encrypt(key).map_err(|e| {
                AppError::Internal(format!("Failed to encrypt env fallback key: {}", e))
            })?;
            validated.push(encrypted);
        }
    }

    Ok(validated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_empty() {
        let registry = ProxyProviderRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_extract_protocol_defaults_to_openai_compatible() {
        let protocol = extract_protocol(&serde_json::json!({})).unwrap();
        assert_eq!(protocol, Protocol::OpenAICompatible);
    }

    #[test]
    fn test_extract_protocol_reads_config() {
        let protocol = extract_protocol(&serde_json::json!({ "protocol": "anthropic" })).unwrap();
        assert_eq!(protocol, Protocol::Anthropic);
    }

    #[test]
    fn test_extract_protocol_rejects_unknown_protocol() {
        let err = extract_protocol(&serde_json::json!({ "protocol": "gemini" })).unwrap_err();
        assert!(err.to_string().contains("Invalid provider protocol"));
    }

    #[test]
    fn test_resolve_provider_keys_returns_validated_ciphertext() {
        let vault = SecretVault::new("test-key").unwrap();
        let encrypted = vault.encrypt("sk-secret").unwrap();
        let cfg = AppConfig {
            database_url: "postgres://localhost/test".to_string(),
            clickhouse_url: String::new(),
            redis_url: String::new(),
            app_port: 8080,
            rust_log: "info".to_string(),
            encryption_key: "test-key".to_string(),
            inspector_capture_level: "metadata_only".to_string(),
            timeout_secs: 60,
            openai_api_key: Some("env-openai".to_string()),
            anthropic_api_key: None,
            admin_secret: None,
            default_org_id: None,
            cors_allowed_origins: Vec::new(),
        };
        let mut config = serde_json::json!({ "api_key_encrypted": encrypted.clone() });
        let keys = resolve_provider_keys("openai", &config, &cfg, &vault).unwrap();
        assert_eq!(keys, vec![encrypted.clone()]);
        assert_eq!(vault.decrypt(&keys[0]).unwrap(), "sk-secret");

        let encrypted2 = vault.encrypt("sk-second").unwrap();
        config["api_keys_encrypted"] = serde_json::json!([encrypted.clone(), encrypted2.clone()]);
        config
            .as_object_mut()
            .map(|obj| obj.remove("api_key_encrypted"));
        let keys = resolve_provider_keys("openai", &config, &cfg, &vault).unwrap();
        assert_eq!(keys, vec![encrypted, encrypted2]);
    }

    #[test]
    fn test_resolve_provider_keys_falls_back_to_env_returns_encrypted() {
        let vault = SecretVault::new("test-key").unwrap();
        let cfg = AppConfig {
            database_url: "postgres://localhost/test".to_string(),
            clickhouse_url: String::new(),
            redis_url: String::new(),
            app_port: 8080,
            rust_log: "info".to_string(),
            encryption_key: "test-key".to_string(),
            inspector_capture_level: "metadata_only".to_string(),
            timeout_secs: 60,
            openai_api_key: Some("env-openai".to_string()),
            anthropic_api_key: None,
            admin_secret: None,
            default_org_id: None,
            cors_allowed_origins: Vec::new(),
        };
        let config = serde_json::json!({});
        let keys = resolve_provider_keys("openai", &config, &cfg, &vault).unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(vault.decrypt(&keys[0]).unwrap(), "env-openai");
    }

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
            openai_api_key: None,
            anthropic_api_key: None,
            admin_secret: None,
            default_org_id: None,
            cors_allowed_origins: Vec::new(),
        }
    }

    #[test]
    fn test_resolve_provider_keys_keeps_healthy_keys_when_one_is_bad() {
        let vault = SecretVault::new("test-key").unwrap();
        let good = vault.encrypt("sk-good").unwrap();
        let bad = "not-valid-base64".to_string();
        let config = serde_json::json!({ "api_keys_encrypted": [good.clone(), bad] });
        let keys = resolve_provider_keys("openrouter", &config, &empty_cfg(), &vault).unwrap();
        assert_eq!(keys, vec![good]);
        assert_eq!(vault.decrypt(&keys[0]).unwrap(), "sk-good");
    }

    #[test]
    fn test_resolve_provider_keys_returns_empty_when_all_keys_bad() {
        let vault = SecretVault::new("test-key").unwrap();
        let config = serde_json::json!({
            "api_keys_encrypted": ["not-valid-base64", "YWJj"]
        });
        let keys = resolve_provider_keys("openrouter", &config, &empty_cfg(), &vault).unwrap();
        assert!(keys.is_empty());
    }
}
