use std::collections::HashMap;

use rand::seq::SliceRandom;
use rand::thread_rng;
use zorch_shared::{ModelId, ProviderId};

/// A single upstream target for a public model.
#[derive(Debug, Clone)]
pub struct Target {
    pub provider_id: ProviderId,
    pub target_model: ModelId,
    pub priority: i32,
}

/// Resolves a public model name to a set of upstream targets.
///
/// Targets are sorted by priority descending. Targets with equal priority are
/// shuffled so that load is distributed across peers.
#[derive(Clone, Default)]
pub struct ModelResolver {
    index: HashMap<ModelId, Vec<Target>>,
}

impl ModelResolver {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    /// Insert a target for the given public model.
    pub fn add_target(&mut self, public_model: ModelId, target: Target) {
        self.index.entry(public_model).or_default().push(target);
    }

    /// Returns the active targets for a public model, sorted by priority
    /// descending. Targets with equal priority are randomly ordered.
    pub fn resolve(&self, public_model: &ModelId) -> Vec<&Target> {
        let Some(targets) = self.index.get(public_model) else {
            return Vec::new();
        };

        let mut active: Vec<&Target> = targets
            .iter()
            .filter(|t| !t.provider_id.as_str().is_empty())
            .collect();

        // Stable sort by priority desc. Equal-priority items retain relative
        // order from the random shuffle below.
        active.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Shuffle contiguous equal-priority groups.
        let mut start = 0;
        while start < active.len() {
            let mut end = start + 1;
            while end < active.len() && active[end].priority == active[start].priority {
                end += 1;
            }
            active[start..end].shuffle(&mut thread_rng());
            start = end;
        }

        active
    }

    /// All public model names known to the resolver.
    pub fn public_models(&self) -> Vec<ModelId> {
        self.index.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_missing_model() {
        let resolver = ModelResolver::new();
        let targets = resolver.resolve(&ModelId::from("missing"));
        assert!(targets.is_empty());
    }

    #[test]
    fn test_resolve_sorts_by_priority_desc() {
        let mut resolver = ModelResolver::new();
        let public = ModelId::from("gpt5");

        resolver.add_target(
            public.clone(),
            Target {
                provider_id: ProviderId::from("azure"),
                target_model: ModelId::from("gpt-5"),
                priority: 5,
            },
        );
        resolver.add_target(
            public.clone(),
            Target {
                provider_id: ProviderId::from("openai"),
                target_model: ModelId::from("gpt-5"),
                priority: 10,
            },
        );
        resolver.add_target(
            public.clone(),
            Target {
                provider_id: ProviderId::from("local"),
                target_model: ModelId::from("gpt-5"),
                priority: 1,
            },
        );

        let targets = resolver.resolve(&public);
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].provider_id, ProviderId::from("openai"));
        assert_eq!(targets[1].provider_id, ProviderId::from("azure"));
        assert_eq!(targets[2].provider_id, ProviderId::from("local"));
    }

    #[test]
    fn test_resolve_equal_priority_shuffled() {
        let mut resolver = ModelResolver::new();
        let public = ModelId::from("gpt5");

        for provider in ["a", "b", "c"] {
            resolver.add_target(
                public.clone(),
                Target {
                    provider_id: ProviderId::from(provider),
                    target_model: ModelId::from("gpt-5"),
                    priority: 10,
                },
            );
        }

        let targets = resolver.resolve(&public);
        assert_eq!(targets.len(), 3);
        let providers: Vec<_> = targets.iter().map(|t| t.provider_id.clone()).collect();
        assert!(providers.contains(&ProviderId::from("a")));
        assert!(providers.contains(&ProviderId::from("b")));
        assert!(providers.contains(&ProviderId::from("c")));
    }
}
