//! Gateway configuration exposed to the admin UI.
//!
//! Currently read-only: values are loaded from environment variables at startup.

use axum::{extract::State, response::Json};
use serde::Serialize;
use zorch_shared::AppError;

use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfigResponse {
    pub app_port: u16,
    pub timeout_secs: u64,
    pub circuit_breaker_timeout_secs: u64,
    pub rust_log: String,
    pub inspector_capture_level: String,
}

pub async fn get_gateway_config(
    State(state): State<AppState>,
) -> Result<Json<GatewayConfigResponse>, AppError> {
    Ok(Json(GatewayConfigResponse {
        app_port: state.config.app_port,
        timeout_secs: state.config.timeout_secs,
        circuit_breaker_timeout_secs: state.config.circuit_breaker_timeout_secs,
        rust_log: state.config.rust_log.clone(),
        inspector_capture_level: state.config.inspector_capture_level.clone(),
    }))
}
