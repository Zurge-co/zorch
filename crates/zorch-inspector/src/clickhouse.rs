use async_trait::async_trait;
use clickhouse::Client;
use serde::Serialize;
use time::OffsetDateTime;
use tracing;
use uuid::Uuid;

use zorch_shared::AppError;

use crate::config::CaptureLevel;
use crate::hook::InspectorHook;
use crate::metadata::{InferenceMetadata, RequestMetadata, ResponseMetadata};

#[derive(clickhouse::Row, Serialize)]
struct InspectorRow {
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    timestamp: OffsetDateTime,
    #[serde(with = "clickhouse::serde::uuid")]
    request_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid::option")]
    organization_id: Option<Uuid>,
    #[serde(with = "clickhouse::serde::uuid::option")]
    api_key_id: Option<Uuid>,
    provider_id: String,
    model: String,
    input_tokens: u32,
    output_tokens: u32,
    latency_ms: u32,
    status_code: u16,
    error_message: Option<String>,
    capture_level: String,
    /// JSON string describing middleware actions (e.g. body_modified).
    /// Requires ClickHouse schema:
    ///   ALTER TABLE inspector_requests
    ///   ADD COLUMN IF NOT EXISTS middleware_metadata Nullable(String);
    middleware_metadata: Option<String>,
}

pub struct ClickHouseInspector {
    client: Client,
    table: String,
}

impl ClickHouseInspector {
    pub fn new(url: &str, table: &str) -> Result<Self, AppError> {
        validate_table_name(table)?;
        let client = Client::default().with_url(url);
        Ok(Self {
            client,
            table: table.to_string(),
        })
    }

#[cfg(test)]
    pub fn build_insert_sql(&self, table: &str) -> String {
        format!(
            "INSERT INTO {} (timestamp, request_id, organization_id, api_key_id, provider_id, model, input_tokens, output_tokens, latency_ms, status_code, error_message, capture_level, middleware_metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            table
        )
    }
}

fn validate_table_name(table: &str) -> Result<(), AppError> {
    if table.is_empty() {
        return Err(AppError::Config(
            "ClickHouse table name cannot be empty".to_string(),
        ));
    }
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::Config(
            "ClickHouse table name must contain only alphanumeric characters and underscores"
                .to_string(),
        ));
    }
    Ok(())
}

#[async_trait]
impl InspectorHook for ClickHouseInspector {
    async fn capture(&self, req: RequestMetadata, resp: ResponseMetadata, inf: InferenceMetadata) {
        if inf.capture_level == CaptureLevel::None {
            return;
        }

        let row = InspectorRow {
            timestamp: OffsetDateTime::now_utc(),
            request_id: *req.request_id,
            organization_id: req.organization_id.map(|id| *id),
            api_key_id: req.api_key_id.map(|id| *id),
            provider_id: req.provider_id.to_string(),
            model: req.model.to_string(),
            input_tokens: req.input_tokens.unwrap_or(0),
            output_tokens: resp.output_tokens.unwrap_or(0),
            latency_ms: inf.latency_ms as u32,
            status_code: resp.status_code,
            error_message: resp.error_message,
            capture_level: format!("{:?}", inf.capture_level),
            middleware_metadata: inf.middleware_metadata.as_ref().map(|v| v.to_string()),
        };

        let mut insert = match self.client.insert::<InspectorRow>(&self.table) {
            Ok(insert) => insert,
            Err(e) => {
                tracing::error!("Failed to create ClickHouse insert: {}", e);
                return;
            }
        };

        if let Err(e) = insert.write(&row).await {
            tracing::error!("Failed to write inspector row: {}", e);
            return;
        }

        if let Err(e) = insert.end().await {
            tracing::error!("Failed to flush inspector log: {}", e);
        }
    }

    async fn health_check(&self) -> bool {
        match self.client.query("SELECT 1").fetch::<u8>() {
            Ok(_cursor) => true,
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clickhouse_inspector_build_insert_sql() {
        let inspector = ClickHouseInspector::new("http://localhost:8123", "test_table").unwrap();
        let sql = inspector.build_insert_sql("inspector_requests");

        assert!(sql.contains("INSERT INTO inspector_requests"));
        assert!(sql.contains("timestamp"));
        assert!(sql.contains("request_id"));
        assert!(sql.contains("organization_id"));
        assert!(sql.contains("api_key_id"));
        assert!(sql.contains("provider_id"));
        assert!(sql.contains("model"));
        assert!(sql.contains("input_tokens"));
        assert!(sql.contains("output_tokens"));
        assert!(sql.contains("latency_ms"));
        assert!(sql.contains("status_code"));
        assert!(sql.contains("error_message"));
        assert!(sql.contains("capture_level"));
        assert!(sql.contains("middleware_metadata"));
        assert!(sql.contains("VALUES"));
    }

    #[test]
    fn test_clickhouse_inspector_new() {
        let result = ClickHouseInspector::new("http://localhost:8123", "test_table");
        assert!(result.is_ok());

        let inspector = result.unwrap();
        assert_eq!(inspector.table, "test_table");
    }

    #[test]
    fn test_middleware_metadata_roundtrips_in_row() {
        // Verify that middleware_metadata JSON is correctly serialized into InspectorRow.
        use crate::config::CaptureLevel;
        use crate::metadata::{InferenceMetadata, RequestMetadata, ResponseMetadata};
        use zorch_shared::{ApiKeyId, ModelId, OrgId, ProviderId, RequestId};

        let inspector = ClickHouseInspector::new("http://localhost:8123", "test_table").unwrap();
        let sql = inspector.build_insert_sql("test_table");

        // The SQL must contain the middleware_metadata column
        assert!(sql.contains("middleware_metadata"));

        // Build a sample row with middleware metadata
        let middleware_meta = serde_json::json!({ "body_modified": true });
        let req = RequestMetadata {
            request_id: RequestId::new(),
            organization_id: Some(OrgId::new()),
            api_key_id: Some(ApiKeyId::new()),
            provider_id: ProviderId::from("openai"),
            model: ModelId::from("gpt-4"),
            input_tokens: Some(42),
        };
        let resp = ResponseMetadata {
            status_code: 200,
            output_tokens: Some(17),
            error_message: None,
        };
        let inf = InferenceMetadata {
            latency_ms: 1234,
            capture_level: CaptureLevel::MetadataOnly,
            middleware_metadata: Some(middleware_meta.clone()),
        };

        let row = InspectorRow {
            timestamp: time::OffsetDateTime::now_utc(),
            request_id: *req.request_id,
            organization_id: req.organization_id.map(|id| *id),
            api_key_id: req.api_key_id.map(|id| *id),
            provider_id: req.provider_id.to_string(),
            model: req.model.to_string(),
            input_tokens: req.input_tokens.unwrap_or(0),
            output_tokens: resp.output_tokens.unwrap_or(0),
            latency_ms: inf.latency_ms as u32,
            status_code: resp.status_code,
            error_message: resp.error_message,
            capture_level: format!("{:?}", inf.capture_level),
            middleware_metadata: inf.middleware_metadata.as_ref().map(|v| v.to_string()),
        };

        assert_eq!(row.middleware_metadata, Some(middleware_meta.to_string()));
    }
}
