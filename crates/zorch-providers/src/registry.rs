use std::collections::HashMap;

use zorch_shared::ProviderId;

use crate::proxy::ProxyProvider;

#[derive(Clone)]
pub struct ProxyProviderRegistry {
    providers: HashMap<ProviderId, ProxyProvider>,
}

impl ProxyProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, id: ProviderId, provider: ProxyProvider) {
        self.providers.insert(id, provider);
    }

    pub fn get(&self, id: &ProviderId) -> Option<&ProxyProvider> {
        self.providers.get(id)
    }

    pub fn find_by_model(&self, model: &str) -> Option<&ProxyProvider> {
        self.providers.values().find(|p| p.supports_model(model))
    }

    pub fn list(&self) -> Vec<ProviderId> {
        self.providers.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
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
    use crate::ProviderHttpClient;
    use zorch_shared::SecretVault;

    fn make_provider(id: &str, models: &[&str]) -> ProxyProvider {
        let vault = SecretVault::new("test-key").unwrap();
        let client = ProviderHttpClient::new(std::time::Duration::from_secs(30)).unwrap();
        ProxyProvider::new(
            ProviderId::from(id),
            "https://example.com/v1".to_string(),
            vault.encrypt("sk-test").unwrap(),
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
        registry.register(
            ProviderId::from("test-provider"),
            make_provider("test-provider", &["gpt-4"]),
        );

        let retrieved = registry.get(&ProviderId::from("test-provider"));
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().provider_id(),
            ProviderId::from("test-provider")
        );
    }

    #[test]
    fn test_registry_find_by_model() {
        let mut registry = ProxyProviderRegistry::new();
        registry.register(
            ProviderId::from("kimi"),
            make_provider("kimi", &["moonshot-v1-8k"]),
        );
        registry.register(
            ProviderId::from("openai"),
            make_provider("openai", &["gpt-4"]),
        );

        let provider = registry.find_by_model("moonshot-v1-8k");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().provider_id(), ProviderId::from("kimi"));
    }

    #[test]
    fn test_registry_find_by_model_missing() {
        let registry = ProxyProviderRegistry::new();
        assert!(registry.find_by_model("unknown-model").is_none());
    }
}
