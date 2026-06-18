use zorch_shared::{ApiKeyId, ModelId, OrgId, ProviderId, RequestId};

#[derive(Debug, Clone)]
pub struct RequestMetadata {
    pub request_id: RequestId,
    pub organization_id: Option<OrgId>,
    pub api_key_id: Option<ApiKeyId>,
    pub provider_id: ProviderId,
    pub model: ModelId,
    pub input_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ResponseMetadata {
    pub status_code: u16,
    pub output_tokens: Option<u32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InferenceMetadata {
    pub latency_ms: u64,
    pub capture_level: crate::config::CaptureLevel,
    pub middleware_metadata: Option<serde_json::Value>,
}
