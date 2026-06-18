use async_trait::async_trait;

use crate::metadata::{InferenceMetadata, RequestMetadata, ResponseMetadata};

#[async_trait]
pub trait InspectorHook: Send + Sync {
    async fn capture(&self, req: RequestMetadata, resp: ResponseMetadata, inf: InferenceMetadata);

    async fn health_check(&self) -> bool {
        true
    }
}

pub struct NoopInspectorHook;

#[async_trait]
impl InspectorHook for NoopInspectorHook {
    async fn capture(
        &self,
        _req: RequestMetadata,
        _resp: ResponseMetadata,
        _inf: InferenceMetadata,
    ) {
    }

    async fn health_check(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CaptureLevel;
    use zorch_shared::{ModelId, ProviderId, RequestId};

    #[tokio::test]
    async fn test_noop_inspector_hook_capture_doesnt_panic() {
        let hook = NoopInspectorHook;
        let req = RequestMetadata {
            request_id: RequestId::new(),
            organization_id: None,
            api_key_id: None,
            provider_id: ProviderId::from("openai"),
            model: ModelId::from("gpt-4"),
            input_tokens: Some(100),
        };
        let resp = ResponseMetadata {
            status_code: 200,
            output_tokens: Some(50),
            error_message: None,
        };
        let inf = InferenceMetadata {
            latency_ms: 150,
            capture_level: CaptureLevel::MetadataOnly,
            middleware_metadata: None,
        };

        hook.capture(req, resp, inf).await;
    }
}
