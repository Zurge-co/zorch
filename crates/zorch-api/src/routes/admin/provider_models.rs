//! Live preview of an upstream provider's `/models` endpoint.
//!
//! Single responsibility: fetch a list of model identifiers from a provider
//! URL the admin is configuring, normalize, dedupe, cap, and return.
//! Different concern from per-provider CRUD endpoints in `providers.rs`.

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewModelsRequest {
    pub base_url: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default)]
    pub auth_header_name: Option<String>,
    #[serde(default)]
    pub auth_prefix: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

fn default_auth_type() -> String {
    zorch_providers::AuthType::default().to_string()
}

#[derive(Serialize)]
pub struct PreviewModelsResponse {
    pub models: Vec<String>,
}

#[derive(Serialize)]
pub struct PreviewModelsError {
    pub error: String,
}

const PREVIEW_MODELS_TIMEOUT_SECS: u64 = 15;
const PREVIEW_MODELS_MAX_RESULTS: usize = 1000;

/// Fetches the live model list from an upstream provider's `/models` endpoint
/// so the admin UI can pre-fill the Models field during provider configuration.
pub async fn preview_provider_models(
    State(_state): State<AppState>,
    Json(req): Json<PreviewModelsRequest>,
) -> Result<Json<PreviewModelsResponse>, (StatusCode, Json<PreviewModelsError>)> {
    let base_url = req.base_url.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PreviewModelsError {
                error: "base_url is required".to_string(),
            }),
        ));
    }

    let auth_type = zorch_providers::AuthType::from_config(
        &req.auth_type,
        req.auth_header_name.as_deref(),
        req.auth_prefix.as_deref(),
    )
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(PreviewModelsError {
                error: format!(
                    "invalid auth type '{}'; supported types: bearer, anthropic, custom: {}",
                    req.auth_type, e
                ),
            }),
        )
    })?;

    let url = format!("{}/models", base_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(PREVIEW_MODELS_TIMEOUT_SECS))
        .build()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PreviewModelsError {
                    error: format!("client init failed: {}", e),
                }),
            )
        })?;

    let mut rb = client.get(&url);
    if let Some(ref k) = req.api_key {
        let trimmed = k.trim();
        if !trimmed.is_empty() {
            let headers = zorch_providers::AuthHeaders::from_auth_type(trimmed, &auth_type)
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(PreviewModelsError {
                            error: format!("failed to build auth headers: {}", e),
                        }),
                    )
                })?
                .build();
            for (key, value) in headers.iter() {
                rb = rb.header(key.clone(), value.clone());
            }
        }
    }

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(base_url = %base_url, error = %e, "preview_provider_models: request failed");
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(PreviewModelsError {
                    error: format!("upstream request failed: {}", e),
                }),
            ));
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        tracing::warn!(base_url = %base_url, upstream_status = %status, "preview_provider_models: non-success upstream");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(PreviewModelsError {
                error: format!("upstream returned status {}", status.as_u16()),
            }),
        ));
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(PreviewModelsError {
                    error: format!("upstream returned non-JSON body: {}", e),
                }),
            ));
        }
    };

    let raw = extract_models(&body).ok_or_else(|| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(PreviewModelsError {
                error: "could not recognize models list shape (expected object with data[]/models[], or array)".to_string(),
            }),
        )
    })?;
    let (models, _truncated) = normalize_models(raw);
    Ok(Json(PreviewModelsResponse { models }))
}

/// Trim, dedupe (preserve first-occurrence order), and cap the model list.
fn normalize_models(raw: Vec<String>) -> (Vec<String>, usize) {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::with_capacity(raw.len().min(PREVIEW_MODELS_MAX_RESULTS));
    let mut truncated: usize = 0;
    for m in raw {
        let trimmed = m.trim().to_string();
        if trimmed.is_empty() || !seen.insert(trimmed.clone()) {
            continue;
        }
        if out.len() >= PREVIEW_MODELS_MAX_RESULTS {
            truncated += 1;
            continue;
        }
        out.push(trimmed);
    }
    if truncated > 0 {
        tracing::warn!(
            dropped = truncated,
            "preview_provider_models: truncating beyond {}",
            PREVIEW_MODELS_MAX_RESULTS
        );
    }
    (out, truncated)
}

fn extract_models(body: &serde_json::Value) -> Option<Vec<String>> {
    fn ids_from(arr: &serde_json::Value) -> Vec<String> {
        arr.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        v.get("id")
                            .or_else(|| v.get("name"))
                            .and_then(|i| i.as_str())
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn strings_from(arr: &serde_json::Value) -> Vec<String> {
        arr.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    // 1) OpenAI / OpenRouter: {"data": [...]} — `id` or `name` per entry
    if let Some(data) = body.get("data") {
        if data.is_array() {
            return Some(ids_from(data));
        }
    }

    // 2/3) Generic: {"models": [...]}
    if let Some(models) = body.get("models") {
        if models.is_array() {
            let ids = ids_from(models);
            if !ids.is_empty() {
                return Some(ids);
            }
            let strs = strings_from(models);
            if !strs.is_empty() {
                return Some(strs);
            }
        }
    }

    // 4/5) Bare array
    let ids = ids_from(body);
    if !ids.is_empty() {
        return Some(ids);
    }
    let strs = strings_from(body);
    if !strs.is_empty() {
        return Some(strs);
    }

    None
}
