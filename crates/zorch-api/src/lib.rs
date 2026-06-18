pub mod middleware;
pub mod routes;
pub mod server;

pub use server::run;

use std::sync::Arc;

use arc_swap::ArcSwap;
use sqlx::PgPool;
use zorch_inspector::InspectorHook;
use zorch_providers::ProxyProviderRegistry;
use zorch_shared::{AppConfig, SecretVault};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db_pool: PgPool,
    pub redis_client: redis::Client,
    pub proxy_providers: Arc<ArcSwap<ProxyProviderRegistry>>,
    pub model_cache: Arc<zorch_cache::ModelProviderCache>,
    pub inspector: Arc<dyn InspectorHook>,
    pub governance: Arc<zorch_gateway::GovernanceEngine>,
    pub billing: Arc<zorch_gateway::BillingEngine>,
    pub circuit_breaker: Arc<zorch_gateway::CircuitBreaker>,
    pub rate_limiter: Arc<zorch_gateway::RateLimiter>,
    pub key_limits: Arc<zorch_gateway::KeyLimits>,
    pub pricing: Arc<arc_swap::ArcSwap<zorch_gateway::PricingEngine>>,
    pub vault: SecretVault,
    pub middleware: Arc<zorch_gateway::MiddlewareEngine>,
}

pub use routes::create_router;
