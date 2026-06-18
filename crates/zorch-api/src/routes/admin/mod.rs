//! Router composition for admin API.

mod analytics;
mod api_keys;
mod dashboard;
mod middleware;
mod pricing;
mod provider_models;
mod providers;
mod providers_state;
mod types;

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/dashboard", get(dashboard::get_dashboard))
        .route("/api/v1/admin/api-keys", get(api_keys::get_api_keys))
        .route("/api/v1/admin/api-keys", post(api_keys::create_api_key))
        .route(
            "/api/v1/admin/api-keys/:id",
            delete(api_keys::revoke_api_key),
        )
        .route(
            "/api/v1/admin/api-keys/:id",
            put(api_keys::update_api_key),
        )
        .route(
            "/api/v1/admin/api-keys/:id/tags",
            put(api_keys::replace_api_key_tags),
        )
        .route("/api/v1/admin/providers", get(providers::get_providers))
        .route("/api/v1/admin/providers", post(providers::create_provider))
        .route(
            "/api/v1/admin/providers/:id",
            put(providers::update_provider),
        )
        .route(
            "/api/v1/admin/providers/:id",
            delete(providers::delete_provider),
        )
        .route(
            "/api/v1/admin/providers/:id/active",
            post(providers::set_provider_active),
        )
        .route(
            "/api/v1/admin/providers/preview-models",
            post(provider_models::preview_provider_models),
        )
        .route("/api/v1/admin/pricing", get(pricing::get_pricing))
        .route("/api/v1/admin/pricing", post(pricing::set_pricing))
        .route("/api/v1/admin/pricing/:id", delete(pricing::delete_pricing))
        .route("/api/v1/admin/analytics", get(analytics::get_analytics))
        .route("/api/v1/admin/analytics/by-tag", get(analytics::get_analytics_by_tag))
        .route("/api/v1/admin/middleware/plugins", get(middleware::get_middleware_plugins))
        .route("/api/v1/admin/middleware/configs", get(middleware::get_middleware_configs))
        .route("/api/v1/admin/middleware/configs", post(middleware::create_middleware_config))
        .route("/api/v1/admin/middleware/configs/:id", put(middleware::update_middleware_config))
        .route("/api/v1/admin/middleware/configs/:id", delete(middleware::delete_middleware_config))
        .route("/api/v1/admin/middleware/runs", get(middleware::get_middleware_runs))
}
