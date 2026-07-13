use axum::body::Bytes;
use futures_util::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use zorch_shared::RequestId;

use super::RequestContext;
use crate::AppState;

pub async fn record_usage(
    state: &AppState,
    ctx: &RequestContext,
    response_bytes: &Bytes,
    status_code: i32,
    latency_ms: i32,
    provider_latency_ms: i32,
    gateway_latency_ms: i32,
) {
    let usage = parse_usage_from_response(response_bytes);
    let (prompt_tokens, completion_tokens) = usage.unwrap_or((0, 0));
    record_usage_async(
        state,
        ctx,
        prompt_tokens,
        completion_tokens,
        status_code,
        latency_ms,
        provider_latency_ms,
        gateway_latency_ms,
    )
    .await;
}

async fn fetch_api_key_tags(pool: &sqlx::PgPool, api_key_id: &uuid::Uuid) -> serde_json::Value {
    sqlx::query_scalar::<_, serde_json::Value>("SELECT tags FROM api_keys WHERE id = $1")
        .bind(*api_key_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(serde_json::json!([]))
}

#[allow(clippy::too_many_arguments)]
async fn record_usage_async(
    state: &AppState,
    ctx: &RequestContext,
    prompt_tokens: u32,
    completion_tokens: u32,
    status_code: i32,
    latency_ms: i32,
    provider_latency_ms: i32,
    gateway_latency_ms: i32,
) {
    let pricing = state.pricing.load();
    let (provider_cost, _total_cost) = pricing.calculate_cost(
        &ctx.provider_id,
        &ctx.target_model_id,
        prompt_tokens,
        completion_tokens,
    );

    let markup_percent = pricing
        .get_pricing(&ctx.provider_id, &ctx.target_model_id)
        .map(|p| p.markup_percent)
        .unwrap_or(0.0);

    let tags = fetch_api_key_tags(&state.db_pool, &ctx.api_key_id).await;

    let request_id = RequestId::new();
    let billing_record = zorch_gateway::BillingRecord::new(
        request_id,
        ctx.api_key_id,
        *ctx.org_id,
        ctx.provider_id.clone(),
        ctx.provider_api_key_id,
        ctx.target_model_id.clone(),
        ctx.public_model_id.clone(),
        prompt_tokens,
        completion_tokens,
        provider_cost,
        markup_percent,
        status_code,
        latency_ms,
        provider_latency_ms,
        gateway_latency_ms,
        tags,
    );

    if let Ok(record) = billing_record {
        let _ = state.billing.record_request(&state.db_pool, record).await;
    }

    let _ = state
        .key_limits
        .record_spend(&ctx.api_key_id.to_string(), provider_cost)
        .await;
}

fn parse_usage_from_response(bytes: &Bytes) -> Option<(u32, u32)> {
    let json: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let usage = json.get("usage")?;
    let prompt = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)?;
    let completion = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)?;
    Some((prompt, completion))
}

pub struct UsageCapturingStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>,
    usage_tx: Option<mpsc::Sender<(u32, u32)>>,
    captured_usage: Option<(u32, u32)>,
}

impl UsageCapturingStream {
    pub fn new(
        stream: impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
        state: AppState,
        ctx: RequestContext,
        status_code: i32,
        latency_ms: i32,
        provider_latency_ms: i32,
        gateway_latency_ms: i32,
    ) -> Self {
        let (usage_tx, mut usage_rx) = mpsc::channel::<(u32, u32)>(1);

        tokio::spawn(async move {
            let usage = tokio::time::timeout(std::time::Duration::from_secs(10), usage_rx.recv())
                .await
                .ok()
                .flatten();

            let (prompt_tokens, completion_tokens) = usage.unwrap_or((0, 0));
            record_usage_async(
                &state,
                &ctx,
                prompt_tokens,
                completion_tokens,
                status_code,
                latency_ms,
                provider_latency_ms,
                gateway_latency_ms,
            )
            .await;
        });

        Self {
            inner: Box::pin(stream),
            usage_tx: Some(usage_tx),
            captured_usage: None,
        }
    }
}

impl Stream for UsageCapturingStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                if self.captured_usage.is_none() {
                    if let Some(usage) = parse_sse_usage_chunk(&chunk) {
                        self.captured_usage = Some(usage);
                        if let Some(tx) = self.usage_tx.as_ref() {
                            let _ = tx.try_send(usage);
                        }
                    }
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                if let Some(usage) = self.captured_usage {
                    if let Some(tx) = self.usage_tx.take() {
                        let _ = tx.try_send(usage);
                    }
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

fn parse_sse_usage_chunk(chunk: &Bytes) -> Option<(u32, u32)> {
    let text = std::str::from_utf8(chunk).ok()?;
    let data_line = text
        .lines()
        .find(|line| line.starts_with("data: "))?
        .strip_prefix("data: ")?;

    if data_line.trim() == "[DONE]" {
        return None;
    }

    let json: serde_json::Value = serde_json::from_str(data_line).ok()?;
    let usage = json.get("usage")?;
    let prompt = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)?;
    let completion = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)?;
    Some((prompt, completion))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_usage_from_response_valid() {
        let body = serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "usage": {
                "prompt_tokens": 42,
                "completion_tokens": 17
            }
        });
        let bytes = Bytes::from(serde_json::to_vec(&body).unwrap());
        let result = super::parse_usage_from_response(&bytes);
        assert_eq!(result, Some((42, 17)));
    }

    #[test]
    fn test_parse_usage_from_response_missing_usage() {
        let body = serde_json::json!({ "id": "chatcmpl-test" });
        let bytes = Bytes::from(serde_json::to_vec(&body).unwrap());
        let result = super::parse_usage_from_response(&bytes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_usage_from_response_invalid_json() {
        let bytes = Bytes::from_static(b"not json");
        let result = super::parse_usage_from_response(&bytes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_sse_usage_chunk_valid() {
        let sse = b"data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5}}\n\n";
        let chunk = Bytes::from_static(sse);
        let result = super::parse_sse_usage_chunk(&chunk);
        assert_eq!(result, Some((10, 5)));
    }

    #[test]
    fn test_parse_sse_usage_chunk_done() {
        let sse = b"data: [DONE]\n\n";
        let chunk = Bytes::from_static(sse);
        let result = super::parse_sse_usage_chunk(&chunk);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_sse_usage_chunk_no_usage() {
        let sse = b"data: {\"id\":\"test\"}\n\n";
        let chunk = Bytes::from_static(sse);
        let result = super::parse_sse_usage_chunk(&chunk);
        assert_eq!(result, None);
    }
}
