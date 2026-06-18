use crate::AppState;
use axum::extract::State;
use axum::response::IntoResponse;

pub async fn middleware(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<impl IntoResponse, zorch_shared::AppError> {
    let future = next.run(req);
    let response = tokio::time::timeout(state.config.timeout_duration(), future)
        .await
        .map_err(|_| zorch_shared::AppError::Timeout)?;
    Ok(response)
}
