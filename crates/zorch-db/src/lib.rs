//! Database layer: connection pooling and SQLx models.
//!
//! [`init_pool`](pool::init_pool) returns a [`sqlx::PgPool`] configured from
//! the shared [`AppConfig`](zorch_shared::AppConfig).  Model modules expose
//! strictly-typed structs that mirror the PostgreSQL schema.

pub mod models;
pub mod pool;

pub use models::*;
pub use pool::init_pool;
