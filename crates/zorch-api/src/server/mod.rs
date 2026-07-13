//! Server initialization and startup.

pub mod providers;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tower_http::classify::ServerErrorsFailureClass;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, info_span, Span};
use zorch_inspector::{ClickHouseInspector, NoopInspectorHook};
use zorch_providers::ProviderHttpClient;
use zorch_shared::{AppConfig, AppError, SecretVault};

use crate::{create_router, AppState};

pub async fn run(cfg: AppConfig) -> Result<(), AppError> {
    let db_pool = zorch_db::init_pool(&cfg.database_url)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to initialize database pool: {}", e)))?;

    let http_client = ProviderHttpClient::new(cfg.timeout_duration())
        .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))?;

    let vault = SecretVault::new(&cfg.encryption_key)
        .map_err(|e| AppError::Config(format!("Failed to initialize SecretVault: {}", e)))?;

    let redis_client = redis::Client::open(cfg.redis_url.as_str())
        .map_err(|e| AppError::Internal(format!("Failed to create Redis client: {}", e)))?;

    let inspector: Arc<dyn zorch_inspector::InspectorHook> = if !cfg.clickhouse_url.is_empty() {
        match ClickHouseInspector::new(&cfg.clickhouse_url, "inspector_requests") {
            Ok(inspector) => Arc::new(inspector),
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize ClickHouse inspector: {}. Using noop.",
                    e
                );
                Arc::new(NoopInspectorHook)
            }
        }
    } else {
        Arc::new(NoopInspectorHook)
    };

    let governance = Arc::new(zorch_gateway::GovernanceEngine::new(
        db_pool.clone(),
        redis_client.clone(),
    ));
    let billing = Arc::new(zorch_gateway::BillingEngine::new());
    let circuit_breaker = Arc::new(zorch_gateway::CircuitBreaker::new().with_config(
        5,
        std::time::Duration::from_secs(cfg.circuit_breaker_timeout_secs),
        3,
    ));

    let rate_limiter = match zorch_gateway::RateLimiter::new(redis_client.clone()) {
        Ok(limiter) => {
            info!("Rate limiter initialized with Redis");
            Arc::new(limiter)
        }
        Err(e) => {
            tracing::error!("Failed to initialize required rate limiter: {}", e);
            return Err(AppError::Internal(format!(
                "Failed to initialize rate limiter: {}",
                e
            )));
        }
    };

    let key_limits = match zorch_gateway::KeyLimits::new(redis_client.clone()) {
        Ok(limits) => {
            info!("Per-key rate limits initialized with Redis");
            Arc::new(limits)
        }
        Err(e) => {
            tracing::error!("Failed to initialize required key limits: {}", e);
            return Err(AppError::Internal(format!(
                "Failed to initialize key limits: {}",
                e
            )));
        }
    };

    let model_cache = match zorch_cache::ModelProviderCache::new(redis_client.clone(), 21_600) {
        Ok(cache) => {
            info!("Model-provider cache initialized with Redis (TTL: 6h)");
            Arc::new(cache)
        }
        Err(e) => {
            tracing::error!("Failed to initialize required model-provider cache: {}", e);
            return Err(AppError::Internal(format!(
                "Failed to initialize model-provider cache: {}",
                e
            )));
        }
    };

    let sticky_ttl = cfg.sticky_target_key_ttl_secs.unwrap_or(300);
    let sticky_target_key_cache =
        match zorch_cache::StickyTargetKeyCache::new(redis_client.clone(), sticky_ttl) {
            Ok(cache) => {
                info!(
                    "Sticky target key cache initialized with Redis (TTL: {}s)",
                    sticky_ttl
                );
                Arc::new(cache)
            }
            Err(e) => {
                tracing::error!("Failed to initialize sticky target key cache: {}", e);
                return Err(AppError::Internal(format!(
                    "Failed to initialize sticky target key cache: {}",
                    e
                )));
            }
        };

    let (proxy_providers, model_resolver) =
        providers::register_providers_and_models(&cfg, &db_pool, &http_client, &vault).await?;

    let middleware_engine = Arc::new(zorch_gateway::MiddlewareEngine::new(db_pool.clone()));

    let state = AppState {
        config: cfg.clone(),
        db_pool: db_pool.clone(),
        redis_client: redis_client.clone(),
        proxy_providers: proxy_providers.clone(),
        model_resolver: model_resolver.clone(),
        model_cache: model_cache.clone(),
        sticky_target_key_cache: sticky_target_key_cache.clone(),
        inspector: inspector.clone(),
        governance,
        billing,
        circuit_breaker,
        rate_limiter,
        key_limits,
        pricing: Arc::new(arc_swap::ArcSwap::new(Arc::new(
            zorch_gateway::PricingEngine::new(),
        ))),
        vault: vault.clone(),
        middleware: middleware_engine,
    };

    let inspector_layer = crate::middleware::inspector::InspectorLayer::new(
        state.inspector.clone(),
        zorch_inspector::CaptureLevel::parse(&cfg.inspector_capture_level),
    );

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &axum::http::Request<_>| {
            let request_id = request
                .extensions()
                .get::<zorch_shared::RequestId>()
                .map(|r| r.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            info_span!(
                "http_request",
                method = %request.method(),
                uri = %request.uri(),
                request_id = %request_id,
            )
        })
        .on_request(|request: &axum::http::Request<_>, _span: &Span| {
            let request_id = request
                .extensions()
                .get::<zorch_shared::RequestId>()
                .map(|r| r.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            tracing::info!(
                method = %request.method(),
                uri = %request.uri(),
                request_id = %request_id,
                "HTTP request started"
            );
        })
        .on_response(|response: &axum::response::Response, latency: Duration, span: &Span| {
            tracing::debug!(
                parent: span,
                status = response.status().as_u16(),
                latency_ms = latency.as_millis(),
                "HTTP request finished"
            );
        })
        .on_failure(
            |error: ServerErrorsFailureClass, latency: Duration, span: &Span| {
                tracing::error!(
                    parent: span,
                    error = %error,
                    latency_ms = latency.as_millis(),
                    "HTTP request failed"
                );
            },
        );

    // Layer ordering matters: request_id runs before trace so the trace span can
    // read the generated request ID from the request extensions.
    let app = create_router()
        .with_state(state.clone())
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::middleware,
        ))
        .layer(inspector_layer)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::timeout::middleware,
        ))
        .layer(trace_layer)
        .layer(axum::middleware::from_fn(
            crate::middleware::request_id::middleware,
        ))
        .layer(build_cors_layer(&cfg.cors_allowed_origins));

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.app_port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("Zorch server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| AppError::Internal(format!("Server error: {}", e)))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };

    let terminate = async {
        let mut sig = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            Ok(s) => s,
            Err(_) => return,
        };
        sig.recv().await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

fn build_cors_layer(allowed_origins: &[String]) -> CorsLayer {
    let base = CorsLayer::new().allow_methods([
        axum::http::Method::GET,
        axum::http::Method::POST,
        axum::http::Method::PUT,
        axum::http::Method::PATCH,
        axum::http::Method::DELETE,
        axum::http::Method::OPTIONS,
    ]);

    if allowed_origins.is_empty() {
        tracing::warn!(
            "ZORCH_CORS_ALLOWED_ORIGINS is unset; allowing ANY origin. \
             Set this to a comma-separated list of admin origins before production launch."
        );
        base.allow_origin(tower_http::cors::Any)
            .allow_headers([axum::http::header::CONTENT_TYPE])
    } else {
        let origins: Vec<axum::http::HeaderValue> = allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        base.allow_origin(origins).allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
    }
}
