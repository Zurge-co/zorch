//! Per-model pricing engine for cost calculation.
//!
//! Provides configurable pricing for different provider/model combinations
//! with optional markup support.

use std::collections::HashMap;
use zorch_shared::{ModelId, ProviderId};

/// Struct name intentionally stays `ModelPricing` (not `ModelConfig`): this
/// struct feeds the `PricingEngine`, which is specifically for cost calculation.
/// Even though the DB table `provider_model_config` now stores more, the engine
/// only consumes pricing fields. Renaming would invite unrelated dependencies
/// to start reading non-pricing fields from this struct.
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub provider: ProviderId,
    pub model: ModelId,
    pub input_cost_per_1m: f64,
    pub output_cost_per_1m: f64,
    pub markup_percent: f64,
    pub max_context_tokens: u64,
}

/// Engine for managing and calculating model pricing.
pub struct PricingEngine {
    prices: HashMap<(ProviderId, ModelId), ModelPricing>,
}

impl PricingEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            prices: HashMap::new(),
        };

        engine.register(ModelPricing {
            provider: ProviderId::from("openai"),
            model: ModelId::from("gpt-4o"),
            input_cost_per_1m: 2500.0,
            output_cost_per_1m: 10000.0,
            markup_percent: 0.0,
            max_context_tokens: 0,
        });

        engine.register(ModelPricing {
            provider: ProviderId::from("openai"),
            model: ModelId::from("gpt-4o-mini"),
            input_cost_per_1m: 150.0,
            output_cost_per_1m: 600.0,
            markup_percent: 0.0,
            max_context_tokens: 0,
        });

        engine.register(ModelPricing {
            provider: ProviderId::from("anthropic"),
            model: ModelId::from("claude-3-5-sonnet"),
            input_cost_per_1m: 3000.0,
            output_cost_per_1m: 15000.0,
            markup_percent: 0.0,
            max_context_tokens: 0,
        });

        engine.register(ModelPricing {
            provider: ProviderId::from("anthropic"),
            model: ModelId::from("claude-3-opus"),
            input_cost_per_1m: 15000.0,
            output_cost_per_1m: 75000.0,
            markup_percent: 0.0,
            max_context_tokens: 0,
        });

        engine
    }

    /// Register pricing for a provider/model combination.
    pub fn register(&mut self, pricing: ModelPricing) {
        let key = (pricing.provider.clone(), pricing.model.clone());
        self.prices.insert(key, pricing);
    }

    /// Calculate cost for a given request.
    /// Returns (provider_cost, total_cost_with_markup)
    pub fn calculate_cost(
        &self,
        provider: &ProviderId,
        model: &ModelId,
        input_tokens: u32,
        output_tokens: u32,
    ) -> (f64, f64) {
        let key = (provider.clone(), model.clone());

        if let Some(pricing) = self.prices.get(&key) {
            let input_cost = (input_tokens as f64 / 1_000_000.0) * pricing.input_cost_per_1m;
            let output_cost = (output_tokens as f64 / 1_000_000.0) * pricing.output_cost_per_1m;
            let provider_cost = input_cost + output_cost;
            let total_cost = provider_cost * (1.0 + pricing.markup_percent / 100.0);
            (provider_cost, total_cost)
        } else {
            (0.0, 0.0)
        }
    }

    /// Get pricing for a specific provider/model combination.
    pub fn get_pricing(&self, provider: &ProviderId, model: &ModelId) -> Option<&ModelPricing> {
        let key = (provider.clone(), model.clone());
        self.prices.get(&key)
    }
}

impl Default for PricingEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost_known_model() {
        let engine = PricingEngine::new();
        let provider = ProviderId::from("openai");
        let model = ModelId::from("gpt-4o");

        let (provider_cost, total_cost) = engine.calculate_cost(&provider, &model, 1000, 500);

        assert!((provider_cost - 7.50).abs() < f64::EPSILON);
        assert!((total_cost - 7.50).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_cost_unknown_model() {
        let engine = PricingEngine::new();
        let provider = ProviderId::from("unknown");
        let model = ModelId::from("unknown-model");

        let (provider_cost, total_cost) = engine.calculate_cost(&provider, &model, 1000, 500);

        assert!((provider_cost - 0.0).abs() < f64::EPSILON);
        assert!((total_cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_markup_calculation() {
        let mut engine = PricingEngine::new();

        engine.register(ModelPricing {
            provider: ProviderId::from("test"),
            model: ModelId::from("test-model"),
            input_cost_per_1m: 1000.0,
            output_cost_per_1m: 2000.0,
            markup_percent: 20.0,
            max_context_tokens: 0,
        });

        let provider = ProviderId::from("test");
        let model = ModelId::from("test-model");

        let (provider_cost, total_cost) = engine.calculate_cost(&provider, &model, 1000, 1000);

        assert!((provider_cost - 3.0).abs() < 1e-6);
        assert!((total_cost - 3.6).abs() < 1e-6);
    }

    #[test]
    fn test_get_pricing() {
        let engine = PricingEngine::new();
        let provider = ProviderId::from("openai");
        let model = ModelId::from("gpt-4o");

        let pricing = engine.get_pricing(&provider, &model);
        assert!(pricing.is_some());

        let pricing = pricing.unwrap();
        assert!((pricing.input_cost_per_1m - 2500.0).abs() < f64::EPSILON);
        assert!((pricing.output_cost_per_1m - 10000.0).abs() < f64::EPSILON);
        assert_eq!(pricing.max_context_tokens, 0);
    }

    #[test]
    fn test_get_pricing_unknown_model() {
        let engine = PricingEngine::new();
        let provider = ProviderId::from("unknown");
        let model = ModelId::from("unknown");

        let pricing = engine.get_pricing(&provider, &model);
        assert!(pricing.is_none());
    }

    #[test]
    fn test_cost_serialization() {
        let engine = PricingEngine::new();
        let provider = ProviderId::from("anthropic");
        let model = ModelId::from("claude-3-opus");

        let (provider_cost, total_cost) = engine.calculate_cost(&provider, &model, 100000, 50000);

        assert!((provider_cost - 5250.0).abs() < f64::EPSILON);
        assert!((total_cost - 5250.0).abs() < f64::EPSILON);
    }
}
