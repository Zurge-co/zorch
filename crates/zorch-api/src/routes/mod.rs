//! Route module hierarchy for the Zorch HTTP API.
//!
//! Composes health checks, public API v1 endpoints, OpenAPI docs, and admin routes.

mod admin;
mod docs;
mod health;
mod v1;

use axum::Router;

use crate::AppState;

pub fn create_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(v1::router())
        .merge(docs::router())
        .merge(admin::router())
}
