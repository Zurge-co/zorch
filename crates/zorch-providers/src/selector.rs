use rand::{seq::SliceRandom, thread_rng};

use crate::proxy::ProxyProvider;

/// Routing strategy for selecting a backend from a pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoutingStrategy {
    /// Pick a backend uniformly at random from the candidates.
    #[default]
    Random,
}

/// Selects a concrete backend from a pool of candidates.
///
/// Callers are expected to filter out unhealthy backends before passing the
/// pool, but if the filtered pool is empty the selector will fall back to the
/// original pool so that a fully degraded pool can still attempt recovery.
#[derive(Clone, Default)]
pub struct BackendSelector {
    strategy: RoutingStrategy,
}

impl BackendSelector {
    pub fn new() -> Self {
        Self {
            strategy: RoutingStrategy::Random,
        }
    }

    /// Pick one backend from `candidates`.
    pub fn select<'a>(&self, candidates: &[&'a ProxyProvider]) -> Option<&'a ProxyProvider> {
        if candidates.is_empty() {
            return None;
        }

        match self.strategy {
            RoutingStrategy::Random => candidates.choose(&mut thread_rng()).copied(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderApiKey, ProviderHttpClient};
    use std::collections::HashSet;
    use zorch_shared::{BackendId, ProviderApiKeyId, ProviderId, SecretVault};

    fn make_provider(backend_id: BackendId, provider_id: &str, model: &str) -> ProxyProvider {
        let vault = SecretVault::new("test-key").unwrap();
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
            vec![model.to_string()],
            ProviderHttpClient::new(std::time::Duration::from_secs(1)).unwrap(),
        )
    }

    #[test]
    fn test_select_returns_none_for_empty_pool() {
        let selector = BackendSelector::new();
        assert!(selector.select(&[]).is_none());
    }

    #[test]
    fn test_select_distribution_over_pool() {
        let selector = BackendSelector::new();
        let p1 = make_provider(BackendId::new(), "openai", "gpt-5");
        let p2 = make_provider(BackendId::new(), "openai", "gpt-5");

        let candidates: Vec<&ProxyProvider> = vec![&p1, &p2];
        let seen: HashSet<BackendId> = (0..40)
            .filter_map(|_| selector.select(&candidates))
            .map(|p| p.backend_id())
            .collect();

        assert_eq!(seen.len(), 2);
    }
}
