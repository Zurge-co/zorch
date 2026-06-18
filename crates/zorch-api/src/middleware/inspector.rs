use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use axum::{body::Body, extract::Request, response::Response};
use tower::{Layer, Service};

use zorch_inspector::{
    CaptureLevel, InferenceMetadata, InspectorHook, RequestMetadata, ResponseMetadata,
};
use zorch_shared::{ApiKeyId, ModelId, OrgId, ProviderId};

/// Inspector middleware layer that wraps services to capture request/response metadata
#[derive(Clone)]
pub struct InspectorLayer {
    inspector: Arc<dyn InspectorHook>,
    capture_level: CaptureLevel,
}

impl InspectorLayer {
    pub fn new(inspector: Arc<dyn InspectorHook>, capture_level: CaptureLevel) -> Self {
        Self {
            inspector,
            capture_level,
        }
    }
}

impl<S> Layer<S> for InspectorLayer {
    type Service = InspectorMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InspectorMiddleware {
            inner,
            inspector: self.inspector.clone(),
            capture_level: self.capture_level,
        }
    }
}

/// Inspector middleware that wraps a service to capture metadata
#[derive(Clone)]
pub struct InspectorMiddleware<S> {
    inner: S,
    inspector: Arc<dyn InspectorHook>,
    capture_level: CaptureLevel,
}

impl<S> InspectorMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    #[allow(clippy::too_many_arguments)]
    fn capture_metadata(
        inspector: Arc<dyn InspectorHook>,
        capture_level: CaptureLevel,
        request_id: String,
        org_id: Option<OrgId>,
        api_key_id: Option<ApiKeyId>,
        provider_id: ProviderId,
        model: ModelId,
        status_code: u16,
        latency_ms: u64,
        error_message: Option<String>,
        middleware_metadata: Option<serde_json::Value>,
    ) {
        if capture_level == CaptureLevel::None {
            return;
        }

        tokio::spawn(async move {
            let req_meta = RequestMetadata {
                request_id: zorch_shared::RequestId::from_uuid(
                    uuid::Uuid::parse_str(&request_id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
                ),
                organization_id: org_id,
                api_key_id,
                provider_id,
                model,
                input_tokens: None,
            };

            let resp_meta = ResponseMetadata {
                status_code,
                output_tokens: None,
                error_message,
            };

            let inf_meta = InferenceMetadata {
                latency_ms,
                capture_level,
                middleware_metadata,
            };

            inspector.capture(req_meta, resp_meta, inf_meta).await;
        });
    }
}

impl<S> Service<Request<Body>> for InspectorMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        let inspector = self.inspector.clone();
        let capture_level = self.capture_level;

        let request_id = request
            .extensions()
            .get::<zorch_shared::RequestId>()
            .map(|id| id.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let org_id = request.extensions().get::<OrgId>().cloned();
        let api_key_id = request.extensions().get::<ApiKeyId>().cloned();

        let provider_id = request
            .extensions()
            .get::<ProviderId>()
            .cloned()
            .unwrap_or_else(|| ProviderId::from("unknown"));
        let model = request
            .extensions()
            .get::<ModelId>()
            .cloned()
            .unwrap_or_else(|| ModelId::from("unknown"));

        let start = Instant::now();

        Box::pin(async move {
            let response = inner.call(request).await?;
            let latency_ms = start.elapsed().as_millis() as u64;

            let status_code = response.status().as_u16();
            let error_message = if status_code >= 400 {
                Some(format!("HTTP {}", status_code))
            } else {
                None
            };

            let middleware_metadata = response.extensions().get::<serde_json::Value>().cloned();

            InspectorMiddleware::<S>::capture_metadata(
                inspector,
                capture_level,
                request_id,
                org_id,
                api_key_id,
                provider_id,
                model,
                status_code,
                latency_ms,
                error_message,
                middleware_metadata,
            );

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    struct MockInspector {
        captured: std::sync::Arc<std::sync::Mutex<bool>>,
    }

    impl MockInspector {
        fn new() -> (Self, std::sync::Arc<std::sync::Mutex<bool>>) {
            let captured = std::sync::Arc::new(std::sync::Mutex::new(false));
            (
                Self {
                    captured: captured.clone(),
                },
                captured,
            )
        }
    }

    #[async_trait::async_trait]
    impl InspectorHook for MockInspector {
        async fn capture(
            &self,
            _req: RequestMetadata,
            _resp: ResponseMetadata,
            _inf: InferenceMetadata,
        ) {
            *self.captured.lock().unwrap() = true;
        }
    }

    #[tokio::test]
    async fn test_inspector_layer_with_capture() {
        let (inspector, captured) = MockInspector::new();
        let layer = InspectorLayer::new(Arc::new(inspector), CaptureLevel::MetadataOnly);

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(layer);

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Give tokio::spawn time to complete
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(*captured.lock().unwrap());
    }

    #[tokio::test]
    async fn test_inspector_layer_noop_on_none_capture() {
        let (inspector, captured) = MockInspector::new();
        let layer = InspectorLayer::new(Arc::new(inspector), CaptureLevel::None);

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(layer);

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(!*captured.lock().unwrap());
    }
}
