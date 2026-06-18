use axum::body::Bytes;
use zorch_shared::AppError;

pub fn extract_model(path: &str, body: &Bytes) -> Result<String, AppError> {
    if path == "/v1/models" || body.is_empty() {
        return Err(AppError::BadRequest(
            "Model lookup requires a model field in the request body".to_string(),
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| AppError::BadRequest("Invalid JSON in request body".to_string()))?;

    json.get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            AppError::BadRequest("Missing or empty 'model' field in request body".to_string())
        })
}
