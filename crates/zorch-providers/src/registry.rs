use std::collections::HashMap;

use zorch_shared::{BackendId, ModelId, ProviderId};

use crate::proxy::ProxyProvider;

#[derive(Clone)]
pub struct ProxyProviderRegistry {
    backends: HashMap<BackendId, ProxyProvider>,
    pools: HashMap<(ProviderId, ModelId), Vec<BackendId>>,
}

impl ProxyProviderRegistry {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            pools: HashMap::new(),
        }
    }

    /// Register a concrete backend and add it to every (provider, model) pool
    /// it advertises.
    pub fn register(&mut self, provider: ProxyProvider) {
        let provider_id = provider.provider_id();
        let backend_id = provider.backend_id();

        for model in provider.models().iter() {
            let model_id = ModelId::from(model.as_str());
            self.pools
                .entry((provider_id.clone(), model_id))
            .or_default()
            .push(backend_id);
        }

        self.backends.insert(backend_id, provider);
    }

    #[cfg(test)]
    pub fn get(&self, id: &BackendId) -> Option<&ProxyProvider> {
        self.backends.get(id)
    }

    #[cfg(test)]
    /// Backward-compatible lookup: returns the first backend that supports the model.
    pub fn find_by_model(&self, model: &str) -> Option<&ProxyProvider> {
        self.backends.values().find(|p| p.supports_model(model))
    }

    #[cfg(test)]
    /// Returns every backend that supports the given model, regardless of provider.
    pub fn find_all_by_model(&self, model: &str) -> Vec<&ProxyProvider> {
        self.backends
            .values()
            .filter(|p| p.supports_model(model))
            .collect()
    }

    /// Returns the pool of backends for a specific logical provider/model pair.
    pub fn find_backends(
        &self,
        provider_id: &ProviderId,
        model_id: &ModelId,
    ) -> Vec<&ProxyProvider> {
        self.pools
            .get(&(provider_id.clone(), model_id.clone()))
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.backends.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns a deduplicated list of logical provider IDs.
    pub fn list(&self) -> Vec<ProviderId> {
        let mut ids: Vec<ProviderId> = self
            .backends
            .values()
            .map(|p| p.provider_id())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        ids
    }

    #[cfg(test)]
    /// Returns every logical (provider_id, model_id) pair advertised by at least one backend.
    pub fn list_models(&self) -> Vec<(ProviderId, ModelId)> {
        self.pools.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

impl Default for ProxyProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderApiKey, ProviderHttpClient};
    use zorch_shared::{ProviderApiKeyId, SecretVault};

    fn make_provider(backend_id: BackendId, provider_id: &str, models: &[&str]) -> ProxyProvider {
        let vault = SecretVault::new("test-key").unwrap();
        let client = ProviderHttpClient::new(std::time::Duration::from_secs(30)).unwrap();
        let api_key = ProviderApiKey::new(
            ProviderApiKeyId::new(),
            vault.encrypt("sk-test").unwrap(),
        );
        ProxyProvider::new(
            backend_id,
            ProviderId::from(provider_id),
            "https://example.com/v1".to_string(),
            vec![api_key],
            vault,
            models.iter().map(|m| m.to_string()).collect(),
            client,
        )
    }

    #[test]
    fn test_registry_new() {
        let registry = ProxyProviderRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ProxyProviderRegistry::new();
        let backend_id = BackendId::new();
        let provider = make_provider(backend_id.clone(), "test-provider", &["gpt-4"]);
        registry.register(provider);

        let retrieved = registry.get(&backend_id);
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().provider_id(),
            ProviderId::from("test-provider")
        );
    }

    #[test]
    fn test_registry_find_by_model() {
        let mut registry = ProxyProviderRegistry::new();
        registry.register(make_provider(BackendId::new(), "kimi", &["moonshot-v1-8k"]));
        registry.register(make_provider(BackendId::new(), "openai", &["gpt-4"]));

        let provider = registry.find_by_model("moonshot-v1-8k");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().provider_id(), ProviderId::from("kimi"));
    }

    #[test]
    fn test_registry_find_by_model_missing() {
        let registry = ProxyProviderRegistry::new();
        assert!(registry.find_by_model("unknown-model").is_none());
    }

    #[test]
    fn test_registry_multiple_backends_same_provider_model() {
        let mut registry = ProxyProviderRegistry::new();
        let b1 = BackendId::new();
        let b2 = BackendId::new();
        registry.register(make_provider(b1, "openai", &["gpt-5"]));
        registry.register(make_provider(b2, "openai", &["gpt-5"]));

        let candidates = registry.find_backends(
            &ProviderId::from("openai"),
            &ModelId::from("gpt-5"),
        );
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|p| p.backend_id() == b1));
        assert!(candidates.iter().any(|p| p.backend_id() == b2));
    }

    #[test]
    fn test_registry_list_providers_is_deduplicated() {
        let mut registry = ProxyProviderRegistry::new();
        registry.register(make_provider(BackendId::new(), "openai", &["gpt-5"]));
        registry.register(make_provider(BackendId::new(), "openai", &["gpt-5"]));

        let providers = registry.list();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0], ProviderId::from("openai"));
    }

    #[test]
    fn test_registry_list_models() {
        let mut registry = ProxyProviderRegistry::new();
        registry.register(make_provider(BackendId::new(), "openai", &["gpt-5", "gpt-4"]));
        registry.register(make_provider(BackendId::new(), "anthropic", &["claude-3"]));

        let models = registry.list_models();
        assert_eq!(models.len(), 3);
    }
}
