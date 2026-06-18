use axum::body::Bytes;
use axum::http::HeaderMap;

pub fn is_streaming_request(body: &Bytes, path: &str) -> bool {
    if path == "/v1/chat/completions/stream" {
        return true;
    }
    if path != "/v1/chat/completions" {
        return false;
    }
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("stream").and_then(|s| s.as_bool()))
        .unwrap_or(false)
}

pub fn inject_stream_usage_options(body: &Bytes) -> Bytes {
    let mut json: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return body.clone(),
    };

    let is_stream = json
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !is_stream {
        return body.clone();
    }

    if let Some(obj) = json.as_object_mut() {
        let opts = obj
            .entry("stream_options")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let Some(opts_obj) = opts.as_object_mut() {
            opts_obj.insert("include_usage".to_string(), serde_json::Value::Bool(true));
        }
    }

    serde_json::to_vec(&json)
        .map(Bytes::from)
        .unwrap_or_else(|_| body.clone())
}

pub fn normalize_upstream_path(path: &str) -> String {
    // The base_url of each provider already includes the /v1 prefix
    // (e.g. "https://openrouter.ai/api/v1", "https://api.openai.com/v1"),
    // so strip the incoming /v1 prefix to avoid doubling it.
    if let Some(stripped) = path.strip_prefix("/v1") {
        if stripped.is_empty() {
            "/".to_string()
        } else {
            stripped.to_string()
        }
    } else {
        path.to_string()
    }
}

pub fn filter_upstream_response_headers(headers: HeaderMap) -> HeaderMap {
    let mut filtered = HeaderMap::new();
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if key_str == "transfer-encoding"
            || key_str == "connection"
            || key_str == "keep-alive"
            || key_str == "upgrade"
        {
            continue;
        }
        filtered.insert(key.clone(), value.clone());
    }
    filtered
}

pub fn filter_client_headers(headers: HeaderMap) -> HeaderMap {
    let mut filtered = HeaderMap::new();
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if key_str == "host"
            || key_str == "connection"
            || key_str == "keep-alive"
            || key_str == "transfer-encoding"
            || key_str == "expect"
            || key_str == "authorization"
        {
            continue;
        }
        filtered.insert(key.clone(), value.clone());
    }
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_upstream_path_strips_v1_prefix_for_chat_completions() {
        assert_eq!(
            normalize_upstream_path("/v1/chat/completions"),
            "/chat/completions"
        );
    }

    #[test]
    fn normalize_upstream_path_strips_v1_prefix_for_anthropic_messages() {
        assert_eq!(normalize_upstream_path("/v1/messages"), "/messages");
    }

    #[test]
    fn normalize_upstream_path_strips_v1_prefix_for_models() {
        assert_eq!(normalize_upstream_path("/v1/models"), "/models");
    }

    #[test]
    fn normalize_upstream_path_keeps_non_v1_paths() {
        assert_eq!(normalize_upstream_path("/health"), "/health");
    }
}
