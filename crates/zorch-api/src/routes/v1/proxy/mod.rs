mod governance;
mod model;
mod request;
mod usage;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use bytes::Bytes;
use futures_util::StreamExt;
use serde::Serialize;
use std::time::Instant;
use zorch_db::ApiKey;
use zorch_providers::{BackendSelector, Target};
use zorch_shared::{ApiKeyId, AppError, BackendId, ModelId, OrgId, ProviderApiKeyId, ProviderId};

use crate::AppState;

use governance::run_governance_pipeline;
use model::extract_model;
use request::{
    filter_client_headers, filter_upstream_response_headers, inject_stream_usage_options,
    is_streaming_request, normalize_upstream_path,
};
use usage::{record_usage, UsageCapturingStream};

fn record_request_metrics(method: &str, status: axum::http::StatusCode) {
    zorch_telemetry::record_http_request(method, status.as_u16());
}

/// Merge middleware headers into a HeaderMap.
fn merge_middleware_headers(
    headers: &mut axum::http::HeaderMap,
    middleware_headers: &std::collections::HashMap<String, String>,
) {
    for (key, value) in middleware_headers {
        let Ok(name) = axum::http::HeaderName::from_bytes(key.as_bytes()) else {
            continue;
        };
        let Ok(val) = axum::http::HeaderValue::from_str(value) else {
            continue;
        };
        headers.insert(name, val);
    }
}

/// Build a response for a middleware block, preserving the script's requested status code.
fn middleware_block_response(err: &zorch_gateway::MiddlewareError) -> Response {
    let status = err
        .status_code
        .filter(|code| (100..=599).contains(code))
        .and_then(|code| axum::http::StatusCode::from_u16(code).ok())
        .unwrap_or(axum::http::StatusCode::BAD_REQUEST);
    let body = Json(serde_json::json!({
        "error": err.message,
    }));
    (status, body).into_response()
}

#[derive(Clone)]
pub struct RequestContext {
    pub api_key_id: ApiKeyId,
    pub org_id: OrgId,
    pub provider_id: ProviderId,
    pub provider_api_key_id: Option<ProviderApiKeyId>,
    pub backend_id: BackendId,
    pub public_model_id: ModelId,
    pub target_model_id: ModelId,
    pub api_key: ApiKey,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chat/completions", post(proxy_handler))
        .route("/chat/completions/stream", post(proxy_handler))
        .route("/messages", post(proxy_handler))
        .route("/embeddings", post(proxy_handler))
        .route("/models", get(list_models_handler))
        .route("/models/:model_id", get(get_model_handler))
}

/// Returns true for HTTP status codes that should trigger a backend failover.
fn is_retryable_status(status: axum::http::StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

/// Rewrite the `model` field in a JSON body to the upstream target model.
fn rewrite_body_model(body: &Bytes, target_model: &str) -> Result<Bytes, AppError> {
    if body.is_empty() {
        return Ok(body.clone());
    }
    let mut json: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {}", e)))?;
    if let Some(obj) = json.as_object_mut() {
        obj.insert(
            "model".to_string(),
            serde_json::Value::String(target_model.to_string()),
        );
    }
    serde_json::to_vec(&json)
        .map(Bytes::from)
        .map_err(|e| AppError::Internal(format!("Failed to serialize rewritten body: {}", e)))
}

/// Resolve a sticky target API key for the given client key and provider.
///
/// 1. Check the Redis cache for an existing mapping.
/// 2. If the mapped key is still active, use it.
/// 3. Otherwise pick the next key in round-robin order, store it, and return it.
async fn resolve_sticky_target_key(
    state: &AppState,
    api_key_id: &ApiKeyId,
    provider: &zorch_providers::ProxyProvider,
) -> ProviderApiKeyId {
    let provider_id = provider.provider_id();

    // Check the sticky cache first.
    if let Ok(Some(cached_id)) = state
        .sticky_target_key_cache
        .get(api_key_id, &provider_id)
        .await
    {
        if provider.find_key_index(&cached_id).is_some() {
            return cached_id;
        }
        // Cached key no longer exists on this provider; clear it.
        let _ = state
            .sticky_target_key_cache
            .invalidate(api_key_id, &provider_id)
            .await;
    }

    // Round-robin over the active keys on this provider using a Redis counter.
    let key_count = provider.api_keys().len();
    let index = state
        .sticky_target_key_cache
        .next_key_index(api_key_id, &provider_id, key_count)
        .await
        .unwrap_or(0);
    let next_id = provider
        .api_keys()
        .get(index)
        .map(|k| k.id)
        .unwrap_or_else(ProviderApiKeyId::new);

    let _ = state
        .sticky_target_key_cache
        .set(api_key_id, &provider_id, &next_id)
        .await;
    next_id
}

/// Try the selected provider backend for a single target.
///
/// Uses sticky target API key resolution, falls over to the next key in the
/// provider when the chosen key fails, and updates the sticky mapping to the
/// successful key.
async fn try_target_pool(
    state: &AppState,
    api_key_id: &ApiKeyId,
    target: &Target,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    path: &str,
    body: Bytes,
) -> Result<(BackendId, ProviderApiKeyId, reqwest::Response, u128), AppError> {
    let registry = state.proxy_providers.load_full();
    let candidates = registry.find_backends(&target.provider_id, &target.target_model);
    if candidates.is_empty() {
        return Err(AppError::Provider(format!(
            "No backends available for {}/{}",
            target.provider_id, target.target_model
        )));
    }

    let selector = BackendSelector::new();
    let max_attempts = candidates.len().max(1);
    let mut attempts = 0;

    loop {
        attempts += 1;

        let healthy: Vec<&zorch_providers::ProxyProvider> =
            futures_util::future::join_all(candidates.iter().map(|p| async {
                let healthy = state
                    .circuit_breaker
                    .is_backend_healthy(&p.backend_id())
                    .await
                    .unwrap_or(false);
                if healthy {
                    Some(*p)
                } else {
                    None
                }
            }))
            .await
            .into_iter()
            .flatten()
            .collect();

        let pool = if healthy.is_empty() {
            candidates.as_slice()
        } else {
            healthy.as_slice()
        };

        let provider = selector.select(pool).ok_or_else(|| {
            AppError::Provider(format!(
                "All backends unavailable for {}/{}",
                target.provider_id, target.target_model
            ))
        })?;

        let backend_id = provider.backend_id();
        let preferred_key_id = resolve_sticky_target_key(state, api_key_id, provider).await;

        let provider_start = Instant::now();
        match provider
            .proxy_request(
                Some(preferred_key_id),
                method.clone(),
                path,
                filter_client_headers(headers.clone()),
                body.clone(),
            )
            .await
        {
            Ok(result) => {
                let provider_latency_ms = provider_start.elapsed().as_millis();
                let status = result.response.status();
                if is_retryable_status(status) {
                    tracing::warn!(
                        backend_id = %backend_id,
                        provider_id = %target.provider_id,
                        target_model = %target.target_model,
                        status = %status,
                        provider_latency_ms = %provider_latency_ms,
                        "Upstream returned retryable status after exhausting target API keys"
                    );
                    state.circuit_breaker.record_failure(&backend_id).await;
                    if attempts < max_attempts {
                        continue;
                    }
                    return Err(AppError::Provider(format!(
                        "Upstream returned retryable status {}",
                        status
                    )));
                }
                // Remember the key that produced the successful response.
                let _ = state
                    .sticky_target_key_cache
                    .set(api_key_id, &provider.provider_id(), &result.api_key_id)
                    .await;
                return Ok((backend_id, result.api_key_id, result.response, provider_latency_ms));
            }
            Err(e) => {
                tracing::warn!(
                    backend_id = %backend_id,
                    provider_id = %target.provider_id,
                    target_model = %target.target_model,
                    error = %e,
                    "Upstream request failed, failing over to another backend"
                );
                state.circuit_breaker.record_failure(&backend_id).await;
                if attempts < max_attempts {
                    continue;
                }
                return Err(AppError::Provider(format!(
                    "Upstream request failed: {}",
                    e
                )));
            }
        }
    }
}

pub async fn proxy_handler(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, 10 * 1024 * 1024)
        .await
        .map_err(|_| AppError::BadRequest("Failed to read request body".to_string()))?;

    let api_key_id = *parts
        .extensions
        .get::<ApiKeyId>()
        .ok_or_else(|| AppError::Auth("API key ID not found in request".to_string()))?;
    let api_key_id_for_sticky = api_key_id;

    let org_id = *parts
        .extensions
        .get::<OrgId>()
        .ok_or_else(|| AppError::Auth("Organization ID not found in request".to_string()))?;

    let api_key = parts
        .extensions
        .get::<ApiKey>()
        .ok_or_else(|| AppError::Auth("API key metadata not found in request".to_string()))?
        .clone();

    let request_id = parts
        .extensions
        .get::<zorch_shared::RequestId>()
        .map(|r| r.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let route = parts.uri.path().to_string();

    let body_json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {}", e)))?;

    // Run pre-governance middleware before model/target resolution so a script can
    // influence which provider backend the request is routed to.
    let pre_governance_mw_ctx = zorch_gateway::MiddlewareContext {
        request_id: request_id.clone(),
        org_id: org_id.to_string(),
        api_key_id: api_key_id.to_string(),
        provider_id: String::new(),
        model_id: String::new(),
        route: route.clone(),
    };
    let pre_governance_input = zorch_gateway::MiddlewareInput::new(body_json.clone());

    let pre_governance_result = state
        .middleware
        .run_phase(
            &api_key_id.to_string(),
            zorch_gateway::MiddlewarePhase::RequestPreGovernance,
            &pre_governance_mw_ctx,
            pre_governance_input,
        )
        .await;

    let modified_input = match pre_governance_result {
        Ok(input) => input,
        Err(e) => return Ok(middleware_block_response(&e)),
    };

    let modified_body_bytes = serde_json::to_vec(&modified_input.body)
        .map_err(|e| AppError::Internal(format!("Failed to serialize modified body: {}", e)))?;
    let model_ref = extract_model(parts.uri.path(), &axum::body::Bytes::from(modified_body_bytes))?;

    let resolver = state.model_resolver.load_full();
    let targets = resolver.resolve(&model_ref.model);
    if targets.is_empty() {
        return Err(AppError::BadRequest(format!(
            "No provider configured for model '{}'",
            model_ref.public_id()
        )));
    }

    let first_target = targets[0];
    let provider_id = first_target.provider_id.clone();

    // Apply headers modified by pre-governance middleware.
    let mut current_headers = parts.headers.clone();
    merge_middleware_headers(&mut current_headers, &modified_input.headers);

    // Governance is run once per client request against the public model.
    let gov_ctx = RequestContext {
        api_key_id,
        org_id,
        provider_id: provider_id.clone(),
        provider_api_key_id: None,
        backend_id: BackendId::new(),
        public_model_id: model_ref.model.clone(),
        target_model_id: first_target.target_model.clone(),
        api_key: api_key.clone(),
    };
    run_governance_pipeline(
        &state,
        &gov_ctx,
        &axum::body::Bytes::from(
            serde_json::to_vec(&modified_input.body)
                .map_err(|e| AppError::Internal(format!("Failed to serialize body: {}", e)))?,
        ),
    )
    .await?;

    // Run pre-upstream middleware with the resolved provider/model context.
    let pre_upstream_mw_ctx = zorch_gateway::MiddlewareContext {
        request_id: request_id.clone(),
        org_id: org_id.to_string(),
        api_key_id: api_key_id.to_string(),
        provider_id: provider_id.to_string(),
        model_id: model_ref.model.to_string(),
        route: route.clone(),
    };
    let pre_upstream_input = zorch_gateway::MiddlewareInput::new(modified_input.body.clone());
    let pre_upstream_result = state
        .middleware
        .run_phase(
            &api_key_id.to_string(),
            zorch_gateway::MiddlewarePhase::RequestPreUpstream,
            &pre_upstream_mw_ctx,
            pre_upstream_input,
        )
        .await;

    let final_input = match pre_upstream_result {
        Ok(input) => input,
        Err(e) => return Ok(middleware_block_response(&e)),
    };

    // Apply headers modified by pre-upstream middleware.
    merge_middleware_headers(&mut current_headers, &final_input.headers);

    let final_body = serde_json::to_vec(&final_input.body)
        .map_err(|e| AppError::Internal(format!("Failed to serialize final body: {}", e)))?;

    let path = normalize_upstream_path(parts.uri.path());
    let method_str = parts.method.as_str().to_string();
    let base_body = axum::body::Bytes::from(final_body);

    tracing::debug!(
        model = %model_ref.public_id(),
        provider_id = %provider_id,
        body_bytes = base_body.len(),
        "proxy_handler: forwarding request upstream"
    );

    let mut last_error: Option<AppError> = None;

    for target in &targets {
        let rewritten = rewrite_body_model(&base_body, target.target_model.as_str())?;
        let upstream_body = if is_streaming_request(&rewritten, &path) {
            inject_stream_usage_options(&rewritten)
        } else {
            rewritten.clone()
        };

        match try_target_pool(
            &state,
            &api_key_id_for_sticky,
            target,
            parts.method.clone(),
            current_headers.clone(),
            &path,
            upstream_body,
        )
        .await
        {
            Ok((backend_id, provider_api_key_id, upstream_response, provider_latency_ms)) => {
                let status = upstream_response.status();
                let upstream_headers = upstream_response.headers().clone();
                let status_code = status.as_u16() as i32;
                let total_latency_ms = start.elapsed().as_millis() as i32;
                let provider_latency_ms = provider_latency_ms as i32;
                let gateway_latency_ms = total_latency_ms.saturating_sub(provider_latency_ms);

                record_request_metrics(&method_str, status);

                let ctx = RequestContext {
                    api_key_id,
                    org_id,
                    provider_id: target.provider_id.clone(),
                    provider_api_key_id: Some(provider_api_key_id),
                    backend_id,
                    public_model_id: model_ref.model.clone(),
                    target_model_id: target.target_model.clone(),
                    api_key: api_key.clone(),
                };

                if is_streaming_request(&rewritten, &path) {
                    let stream = upstream_response.bytes_stream().map(move |result| {
                        result.map_err(|e| std::io::Error::other(e.to_string()))
                    });
                    state.circuit_breaker.record_success(&ctx.backend_id).await;
                    let wrapped = UsageCapturingStream::new(
                        stream,
                        state.clone(),
                        ctx.clone(),
                        status_code,
                        total_latency_ms,
                        provider_latency_ms,
                        gateway_latency_ms,
                    );
                    let mut response = Response::builder()
                        .status(status)
                        .body(Body::from_stream(wrapped))
                        .map_err(|e| {
                            AppError::Internal(format!("Failed to build streaming response: {}", e))
                        })?;
                    response.extensions_mut().insert(serde_json::json!({
                        "middleware": {
                            "body_modified": final_input.body != body_json,
                        }
                    }));
                    return Ok(response);
                } else {
                    let response_bytes = upstream_response.bytes().await.map_err(|e| {
                        AppError::Provider(format!("Failed to read upstream response: {}", e))
                    })?;

                    state.circuit_breaker.record_success(&ctx.backend_id).await;
                    record_usage(
                        &state,
                        &ctx,
                        &response_bytes,
                        status_code,
                        total_latency_ms,
                        provider_latency_ms,
                        gateway_latency_ms,
                    )
                    .await;

                    let mut response = Response::builder()
                        .status(status)
                        .body(Body::from(response_bytes))
                        .map_err(|e| {
                            AppError::Internal(format!("Failed to build response: {}", e))
                        })?;
                    *response.headers_mut() = filter_upstream_response_headers(upstream_headers);
                    response.extensions_mut().insert(serde_json::json!({
                        "middleware": {
                            "body_modified": final_input.body != body_json,
                        }
                    }));
                    return Ok(response);
                }
            }
            Err(e) => {
                tracing::warn!(
                    provider_id = %target.provider_id,
                    target_model = %target.target_model,
                    error = %e,
                    "Target pool failed, moving to next target"
                );
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        AppError::Provider(format!(
            "All targets unavailable for model '{}'",
            model_ref.public_id()
        ))
    }))
}

#[derive(Serialize)]
struct ModelEntry {
    id: String,
    object: String,
    created: i64,
    owned_by: String,
}

#[derive(Serialize)]
struct ModelsListResponse {
    object: String,
    data: Vec<ModelEntry>,
}

pub async fn list_models_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let resolver = state.model_resolver.load_full();
    let now = chrono::Utc::now().timestamp();

    let data = resolver
        .public_models()
        .into_iter()
        .map(|m| ModelEntry {
            id: m.as_str().to_string(),
            object: "model".to_string(),
            created: now,
            owned_by: "zorch".to_string(),
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(ModelsListResponse {
            object: "list".to_string(),
            data,
        }),
    ))
}

pub async fn get_model_handler(
    State(state): State<AppState>,
    axum::extract::Path(model_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let resolver = state.model_resolver.load_full();
    let targets = resolver.resolve(&ModelId::from(model_id.as_str()));

    if targets.is_empty() {
        return Err(AppError::NotFound(format!(
            "Model '{}' not found",
            model_id
        )));
    }

    Ok((
        StatusCode::OK,
        Json(ModelEntry {
            id: model_id,
            object: "model".to_string(),
            created: chrono::Utc::now().timestamp(),
            owned_by: targets[0].provider_id.as_str().to_string(),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn merge_middleware_headers_adds_and_overrides_headers() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );

        let mut mw_headers = std::collections::HashMap::new();
        mw_headers.insert("X-Custom".to_string(), "value".to_string());
        mw_headers.insert("Content-Type".to_string(), "text/plain".to_string());

        merge_middleware_headers(&mut headers, &mw_headers);

        assert_eq!(
            headers.get("X-Custom").unwrap(),
            "value"
        );
        assert_eq!(
            headers.get(axum::http::header::CONTENT_TYPE).unwrap(),
            "text/plain"
        );
    }

    #[test]
    fn merge_middleware_headers_skips_invalid_entries() {
        let mut headers = axum::http::HeaderMap::new();
        let mut mw_headers = std::collections::HashMap::new();
        mw_headers.insert(String::new(), "empty-key".to_string());
        mw_headers.insert("X-Valid".to_string(), "ok".to_string());
        mw_headers.insert("X-Bad-Value".to_string(), "\0".to_string());

        merge_middleware_headers(&mut headers, &mw_headers);

        assert_eq!(headers.get("X-Valid").unwrap(), "ok");
        assert!(headers.get("X-Bad-Value").is_none());
        assert!(headers.get("").is_none());
    }

    #[test]
    fn middleware_block_response_uses_custom_status_code() {
        let err = zorch_gateway::MiddlewareError {
            plugin_key: "rhai".to_string(),
            message: "Forbidden".to_string(),
            status_code: Some(403),
        };
        let response = middleware_block_response(&err);
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn middleware_block_response_defaults_to_bad_request() {
        let err = zorch_gateway::MiddlewareError {
            plugin_key: "rhai".to_string(),
            message: "Bad".to_string(),
            status_code: None,
        };
        let response = middleware_block_response(&err);
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn middleware_block_response_defaults_for_invalid_status_code() {
        let err = zorch_gateway::MiddlewareError {
            plugin_key: "rhai".to_string(),
            message: "Bad".to_string(),
            status_code: Some(999),
        };
        let response = middleware_block_response(&err);
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn rewrite_body_model_updates_model_field() {
        let body = Bytes::from_static(br#"{"model":"public","temperature":0.5}"#);
        let rewritten = rewrite_body_model(&body, "target-model").unwrap();
        let json: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();
        assert_eq!(json["model"], "target-model");
        assert_eq!(json["temperature"], 0.5);
    }

    #[test]
    fn rewrite_body_model_preserves_empty_body() {
        let body = Bytes::new();
        let rewritten = rewrite_body_model(&body, "target-model").unwrap();
        assert!(rewritten.is_empty());
    }
}
