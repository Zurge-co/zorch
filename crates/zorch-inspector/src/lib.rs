//! Request metadata capture and inspection.
//!
//! Provides hooks that observe inference requests and responses without
//! interfering with the hot path.  Captured metadata is written to ClickHouse
//! for downstream analytics.  The [`InspectorHook`](hook::InspectorHook)
//! trait allows swapping the backend (e.g. for testing) without changing
//! call sites.

pub mod clickhouse;
pub mod config;
pub mod hook;
pub mod metadata;

pub use clickhouse::ClickHouseInspector;
pub use config::CaptureLevel;
pub use hook::{InspectorHook, NoopInspectorHook};
pub use metadata::{InferenceMetadata, RequestMetadata, ResponseMetadata};
