use std::sync::OnceLock;

use metrics::counter;
use metrics_exporter_prometheus::PrometheusBuilder;
use zorch_shared::AppError;

static PROMETHEUS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();

pub fn init_metrics() -> Result<(), AppError> {
    if PROMETHEUS_HANDLE.get().is_some() {
        return Ok(());
    }

    let builder = PrometheusBuilder::new();
    let recorder = builder.build_recorder();
    let render_handle = recorder.handle();

    if let Err(e) = metrics::set_global_recorder(recorder) {
        // If a recorder is already installed by another test or component, treat
        // initialization as successful so telemetry remains idempotent.
        let _ = e;
        return Ok(());
    }

    let _ = PROMETHEUS_HANDLE.set(render_handle);

    Ok(())
}

pub fn metrics_snapshot() -> Option<String> {
    PROMETHEUS_HANDLE.get().map(|handle| handle.render())
}

/// Record a proxied HTTP request in the `zorch_http_requests_total` counter.
pub fn record_http_request(method: &str, status: u16) {
    counter!(
        "zorch_http_requests_total",
        "method" => method.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_metrics_does_not_panic() {
        let result = init_metrics();
        assert!(result.is_ok());
    }

    #[test]
    fn test_record_http_request_no_panic() {
        init_metrics().ok();
        record_http_request("GET", 200);
    }

    #[test]
    fn test_metrics_snapshot_before_init() {
        assert!(metrics_snapshot().is_none());
    }
}
