use axum::body::Bytes;
use zorch_shared::{AppError, ModelId};

/// A model reference extracted from the request body.
///
/// Only public model names are supported. Provider/model prefixes are no longer
/// used; routing is resolved through the model resolver.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRef {
    /// The raw model string exactly as sent by the client.
    pub raw: String,
    /// The public model identifier used for resolution and governance.
    pub model: ModelId,
}

impl ModelRef {
    /// Returns the public model id.
    pub fn public_id(&self) -> String {
        self.model.as_str().to_string()
    }
}

pub fn extract_model(path: &str, body: &Bytes) -> Result<ModelRef, AppError> {
    if path == "/v1/models" || body.is_empty() {
        return Err(AppError::BadRequest(
            "Model lookup requires a model field in the request body".to_string(),
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| AppError::BadRequest("Invalid JSON in request body".to_string()))?;

    let raw = json
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            AppError::BadRequest("Missing or empty 'model' field in request body".to_string())
        })?;

    if raw.is_empty() {
        return Err(AppError::BadRequest(
            "Missing or empty 'model' field in request body".to_string(),
        ));
    }

    Ok(ModelRef {
        raw: raw.clone(),
        model: ModelId::from(raw.as_str()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(s: &str) -> Bytes {
        Bytes::copy_from_slice(s.as_bytes())
    }

    #[test]
    fn test_extract_model_public_name() {
        let body = bytes(r#"{"model":"gpt5"}"#);
        let m = extract_model("/v1/chat/completions", &body).unwrap();
        assert_eq!(m.raw, "gpt5");
        assert_eq!(m.model, ModelId::from("gpt5"));
        assert_eq!(m.public_id(), "gpt5");
    }

    #[test]
    fn test_extract_model_missing_field() {
        let body = bytes(r#"{"prompt":"hello"}"#);
        assert!(extract_model("/v1/chat/completions", &body).is_err());
    }

    #[test]
    fn test_extract_model_empty() {
        let body = bytes(r#"{"model":""}"#);
        assert!(extract_model("/v1/chat/completions", &body).is_err());
    }
}
