//! Router composition for admin API.

mod analytics;
mod api_keys;
mod config;
mod dashboard;
mod middleware;
mod model_targets;
mod models;
mod pricing;
mod provider_api_keys;
mod provider_models;
mod provider_target_models;
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
        .route("/api/v1/admin/config", get(config::get_gateway_config))
        .route("/api/v1/admin/dashboard", get(dashboard::get_dashboard))
        .route("/api/v1/admin/api-keys", get(api_keys::get_api_keys))
        .route("/api/v1/admin/api-keys", post(api_keys::create_api_key))
        .route(
            "/api/v1/admin/api-keys/:id",
            delete(api_keys::revoke_api_key),
        )
        .route("/api/v1/admin/api-keys/:id", put(api_keys::update_api_key))
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
            "/api/v1/admin/providers/:id/targets",
            get(model_targets::get_provider_targets),
        )
        .route(
            "/api/v1/admin/providers/:id/target-models",
            get(provider_target_models::get_provider_target_models),
        )
        .route(
            "/api/v1/admin/providers/:id/target-models",
            post(provider_target_models::create_provider_target_model),
        )
        .route(
            "/api/v1/admin/providers/:id/target-models/:tm_id",
            delete(provider_target_models::delete_provider_target_model),
        )
        .route(
            "/api/v1/admin/providers/:id/target-models/sync",
            post(provider_target_models::sync_provider_target_models),
        )
        .route(
            "/api/v1/admin/providers/:id/api-keys",
            get(provider_api_keys::get_provider_api_keys),
        )
        .route(
            "/api/v1/admin/providers/:id/api-keys",
            post(provider_api_keys::create_provider_api_key),
        )
        .route(
            "/api/v1/admin/providers/:id/api-keys/:key_id",
            delete(provider_api_keys::delete_provider_api_key),
        )
        .route(
            "/api/v1/admin/providers/:id/api-keys/:key_id/active",
            put(provider_api_keys::set_provider_api_key_active),
        )
        .route(
            "/api/v1/admin/providers/preview-models",
            post(provider_models::preview_provider_models),
        )
        .route("/api/v1/admin/models", get(models::get_models))
        .route("/api/v1/admin/models", post(models::create_model))
        .route("/api/v1/admin/models/:id", get(models::get_model))
        .route("/api/v1/admin/models/:id", put(models::update_model))
        .route("/api/v1/admin/models/:id", delete(models::delete_model))
        .route(
            "/api/v1/admin/models/:id/targets",
            get(model_targets::get_model_targets),
        )
        .route(
            "/api/v1/admin/models/:id/targets",
            post(model_targets::create_model_target),
        )
        .route(
            "/api/v1/admin/models/:id/targets/:target_id",
            put(model_targets::update_model_target),
        )
        .route(
            "/api/v1/admin/models/:id/targets/:target_id",
            delete(model_targets::delete_model_target),
        )
        .route("/api/v1/admin/pricing", get(pricing::get_pricing))
        .route("/api/v1/admin/pricing", post(pricing::set_pricing))
        .route("/api/v1/admin/pricing/:id", delete(pricing::delete_pricing))
        .route("/api/v1/admin/analytics", get(analytics::get_analytics))
        .route(
            "/api/v1/admin/analytics/by-tag",
            get(analytics::get_analytics_by_tag),
        )
        .route(
            "/api/v1/admin/middleware/configs",
            get(middleware::get_middleware_configs),
        )
        .route(
            "/api/v1/admin/middleware/configs",
            post(middleware::create_middleware_config),
        )
        .route(
            "/api/v1/admin/middleware/configs/:id",
            get(middleware::get_middleware_config),
        )
        .route(
            "/api/v1/admin/middleware/configs/:id",
            put(middleware::update_middleware_config),
        )
        .route(
            "/api/v1/admin/middleware/configs/:id",
            delete(middleware::delete_middleware_config),
        )
        .route(
            "/api/v1/admin/middleware/runs",
            get(middleware::get_middleware_runs),
        )
        .route(
            "/api/v1/admin/api-keys/:id/middleware-configs",
            get(api_keys::get_api_key_middleware_configs),
        )
        .route(
            "/api/v1/admin/api-keys/:id/middleware-configs/:config_id",
            post(api_keys::assign_api_key_middleware_config),
        )
        .route(
            "/api/v1/admin/api-keys/:id/middleware-configs/:config_id",
            delete(api_keys::unassign_api_key_middleware_config),
        )
        .route(
            "/api/v1/admin/middleware/validate",
            post(middleware::validate_middleware_script),
        )
        .route(
            "/api/v1/admin/middleware/run",
            post(middleware::run_middleware_script),
        )
}
