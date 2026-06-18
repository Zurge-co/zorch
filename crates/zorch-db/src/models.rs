use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// ApiKey model with per-key rate limiting and spend budget fields.
///
/// Expected database schema (api_keys table):
/// - id: UUID (primary key)
/// - organization_id: UUID (foreign key to organizations)
/// - key_hash: TEXT (hashed API key for authentication)
/// - scopes: TEXT[] (array of permission scopes)
/// - expires_at: TIMESTAMPTZ (optional expiration timestamp)
/// - is_active: BOOLEAN (whether key is currently active)
/// - created_at: TIMESTAMPTZ (creation timestamp)
/// - name: TEXT (friendly name)
/// - requests_per_minute: INTEGER NULL (per-key RPM limit, NULL = use global default)
/// - requests_per_day: INTEGER NULL (per-key RPD limit, NULL = use global default)
/// - max_spend_usd: DOUBLE PRECISION NULL (spend budget in USD, NULL = unlimited)
/// - allowed_models: TEXT[] NULL (allowed model IDs, NULL = all models allowed)
/// - tags: JSONB NOT NULL DEFAULT '[]' (key:value tags for cost attribution)
/// - allowed_hours_start: SMALLINT NULL (0-23, time-of-day window start)
/// - allowed_hours_end: SMALLINT NULL (0-23, time-of-day window end)
/// - window_timezone: TEXT NULL (IANA timezone name for the access window)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub requests_per_minute: Option<i32>,
    pub requests_per_day: Option<i32>,
    pub max_spend_usd: Option<f64>,
    pub allowed_models: Option<Vec<String>>,
    pub tags: serde_json::Value,
    pub allowed_hours_start: Option<i16>,
    pub allowed_hours_end: Option<i16>,
    pub window_timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProviderConfig {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub config: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}
