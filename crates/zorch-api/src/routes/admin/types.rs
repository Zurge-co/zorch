//! Shared response DTOs for admin routes.
//!
//! All DTOs serialize to camelCase so the Next.js admin app can consume them
//! without manual field-name translation.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub requests_per_minute: String,
    pub tokens_per_minute: String,
    pub error_rate: String,
    pub active_providers: u32,
    pub requests_trend_percent: f64,
    pub tokens_trend_percent: f64,
    pub error_rate_trend_percent: f64,
    pub requests_last_24h: i64,
    pub tokens_last_24h: i64,
    pub error_requests_last_24h: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentActivity {
    pub id: String,
    pub key_id: String,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub timestamp: String,
    pub latency: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardResponse {
    pub stats: DashboardStats,
    pub recent_activity: Vec<RecentActivity>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key: String,
    pub status: String,
    pub created_at: String,
    pub usage: String,
    pub tags: Vec<zorch_shared::ApiKeyTag>,
    pub allowed_hours_start: Option<u8>,
    pub allowed_hours_end: Option<u8>,
    pub window_timezone: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeysResponse {
    pub keys: Vec<ApiKeyResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderResponse {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub status: String,
    pub models: Vec<String>,
    pub latency: String,
    pub cost_per_1m: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingResponse {
    pub id: String,
    pub provider_id: String,
    pub provider: String,
    pub model: String,
    pub input_cost_per_1m: f64,
    pub output_cost_per_1m: f64,
    pub markup_percent: f64,
    pub max_context_tokens: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsagePoint {
    pub name: String,
    pub usage: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostTrendPoint {
    pub name: String,
    pub cost: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyPoint {
    pub name: String,
    pub value: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsResponse {
    pub token_usage: Vec<TokenUsagePoint>,
    pub cost_trends: Vec<CostTrendPoint>,
    pub latency: Vec<LatencyPoint>,
    pub error_rate: f64,
    pub total_requests_24h: i64,
    pub error_requests_24h: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagAnalyticsEntry {
    pub tag: String,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_cents: i64,
    pub error_rate: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagAnalyticsResponse {
    pub tags: Vec<TagAnalyticsEntry>,
}
