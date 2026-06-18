use axum::{
    body::Body, extract::State, http::StatusCode, response::Response, routing::get, Json, Router,
};
use serde_json::json;

use crate::AppState;
use zorch_shared::AppError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_handler))
        .route("/health/ready", get(readiness_handler))
        .route("/metrics", get(metrics_handler))
}

async fn health_handler() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

async fn readiness_handler(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false);

    let redis_ok = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map(|s| s == "PONG")
            .unwrap_or(false),
        Err(_) => false,
    };

    let clickhouse_ok = state.inspector.health_check().await;

    let all_ok = db_ok && redis_ok && clickhouse_ok;
    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = json!({
        "status": if all_ok { "ready" } else { "degraded" },
        "checks": {
            "database": db_ok,
            "redis": redis_ok,
            "clickhouse": clickhouse_ok,
        }
    });

    (status_code, Json(body))
}

async fn metrics_handler(State(_state): State<AppState>) -> Result<Response<Body>, AppError> {
    match zorch_telemetry::metrics_snapshot() {
        Some(body) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; version=0.0.4")
            .body(Body::from(body))
            .map_err(|e| AppError::Internal(format!("Failed to build metrics response: {}", e))),
        None => Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::from("Metrics not initialized"))
            .map_err(|e| AppError::Internal(format!("Failed to build metrics response: {}", e))),
    }
}
