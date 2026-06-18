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
use futures_util::StreamExt;
use serde::Serialize;
use std::time::Instant;
use zorch_shared::{ApiKeyId, AppError, ModelId, OrgId, ProviderId};

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

#[derive(Clone)]
pub struct RequestContext {
    pub api_key_id: ApiKeyId,
    pub org_id: OrgId,
    pub provider_id: ProviderId,
    pub model_id: ModelId,
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

pub async fn proxy_handler(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, 10 * 1024 * 1024)
        .await
        .map_err(|_| AppError::BadRequest("Failed to read request body".to_string()))?;

    let model = extract_model(parts.uri.path(), &bytes)?;

    let provider = match state.model_cache.get(&model).await? {
        Some(provider_id) => state
            .proxy_providers
            .load_full()
            .get(&provider_id)
            .cloned()
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Cached provider for model '{}' is no longer available",
                    model
                ))
            })?,
        None => {
            let registry = state.proxy_providers.load_full();
            let provider = registry
                .find_by_model(&model)
                .ok_or_else(|| {
                    AppError::BadRequest(format!("No provider configured for model '{}'", model))
                })?
                .clone();
            state
                .model_cache
                .set(&model, &provider.provider_id())
                .await?;
            provider
        }
    };

    let provider_id = provider.provider_id();
    let model_id = ModelId::from(model.as_str());

    let api_key_id = parts
        .extensions
        .get::<ApiKeyId>()
        .ok_or_else(|| AppError::Auth("API key ID not found in request".to_string()))?
        .clone();

    let org_id = parts
        .extensions
        .get::<OrgId>()
        .ok_or_else(|| AppError::Auth("Organization ID not found in request".to_string()))?
        .clone();

    let request_id = parts
        .extensions
        .get::<zorch_shared::RequestId>()
        .map(|r| r.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let route = parts.uri.path().to_string();

    let ctx = RequestContext {
        api_key_id: api_key_id.clone(),
        org_id: org_id.clone(),
        provider_id: provider_id.clone(),
        model_id: model_id.clone(),
    };

    let body_json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {}", e)))?;

    let mw_ctx = zorch_gateway::MiddlewareContext {
        request_id: request_id.clone(),
        org_id: org_id.to_string(),
        api_key_id: api_key_id.to_string(),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        route: route.clone(),
    };

    let mw_input = zorch_gateway::MiddlewareInput::new(body_json.clone());

    let pre_governance_result = state
        .middleware
        .run_phase(zorch_gateway::MiddlewarePhase::RequestPreGovernance, &mw_ctx, mw_input)
        .await;

    let modified_input = match pre_governance_result {
        Ok(input) => input,
        Err(e) => {
            return Err(AppError::BadRequest(format!(
                "Middleware blocked request: {}",
                e.message
            )));
        }
    };

    let modified_body = if modified_input.body != body_json {
        serde_json::to_vec(&modified_input.body)
            .map_err(|e| AppError::Internal(format!("Failed to serialize modified body: {}", e)))?
    } else {
        bytes.to_vec()
    };

    run_governance_pipeline(&state, &ctx, &axum::body::Bytes::from(modified_body.clone())).await?;

    let pre_modified_body = modified_input.body.clone();
    let pre_upstream_input = zorch_gateway::MiddlewareInput::new(modified_input.body);
    let pre_upstream_result = state
        .middleware
        .run_phase(zorch_gateway::MiddlewarePhase::RequestPreUpstream, &mw_ctx, pre_upstream_input)
        .await;

    let final_input = match pre_upstream_result {
        Ok(input) => input,
        Err(e) => {
            return Err(AppError::BadRequest(format!(
                "Middleware blocked request: {}",
                e.message
            )));
        }
    };

    let final_body = if final_input.body != pre_modified_body {
        serde_json::to_vec(&final_input.body)
            .map_err(|e| AppError::Internal(format!("Failed to serialize modified body: {}", e)))?
    } else {
        modified_body
    };

    let path = normalize_upstream_path(parts.uri.path());
    let method_str = parts.method.as_str().to_string();

    let body_bytes = axum::body::Bytes::from(final_body);

    let upstream_response = if is_streaming_request(&body_bytes, &path) {
        let streaming_body = inject_stream_usage_options(&body_bytes);
        provider
            .proxy_request(
                parts.method,
                &path,
                filter_client_headers(parts.headers),
                streaming_body,
            )
            .await
            .map_err(|e| AppError::Provider(format!("Upstream request failed: {}", e)))?
    } else {
        provider
            .proxy_request(
                parts.method,
                &path,
                filter_client_headers(parts.headers),
                body_bytes.clone(),
            )
            .await
            .map_err(|e| AppError::Provider(format!("Upstream request failed: {}", e)))?
    };

    let status = upstream_response.status();
    let upstream_headers = upstream_response.headers().clone();

    let status_code = status.as_u16() as i32;
    let latency_ms = start.elapsed().as_millis() as i32;

    record_request_metrics(&method_str, status);

    if is_streaming_request(&body_bytes, &path) {
        let stream = upstream_response
            .bytes_stream()
            .map(move |result| result.map_err(|e| std::io::Error::other(e.to_string())));
        state.circuit_breaker.record_success(&ctx.provider_id).await;
        let wrapped =
            UsageCapturingStream::new(stream, state.clone(), ctx.clone(), status_code, latency_ms);
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
        Ok(response)
    } else {
        let response_bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| AppError::Provider(format!("Failed to read upstream response: {}", e)))?;

        state.circuit_breaker.record_success(&ctx.provider_id).await;
        record_usage(&state, &ctx, &response_bytes, status_code, latency_ms).await;

        let mut response = Response::builder()
            .status(status)
            .body(Body::from(response_bytes))
            .map_err(|e| AppError::Internal(format!("Failed to build response: {}", e)))?;
        *response.headers_mut() = filter_upstream_response_headers(upstream_headers);
        response.extensions_mut().insert(serde_json::json!({
            "middleware": {
                "body_modified": final_input.body != body_json,
            }
        }));
        Ok(response)
    }
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
    let registry = state.proxy_providers.load_full();
    let now = chrono::Utc::now().timestamp();

    let mut data = Vec::new();
    for provider in registry.list().iter().filter_map(|id| registry.get(id)) {
        let owned_by = provider.provider_id().as_str().to_string();
        for model in provider.models() {
            data.push(ModelEntry {
                id: model.clone(),
                object: "model".to_string(),
                created: now,
                owned_by: owned_by.clone(),
            });
        }
    }

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
    let registry = state.proxy_providers.load_full();

    let provider = registry
        .find_by_model(&model_id)
        .ok_or_else(|| AppError::NotFound(format!("Model '{}' not found", model_id)))?;

    Ok((
        StatusCode::OK,
        Json(ModelEntry {
            id: model_id,
            object: "model".to_string(),
            created: chrono::Utc::now().timestamp(),
            owned_by: provider.provider_id().as_str().to_string(),
        }),
    ))
}
