use sqlx::PgPool;
use zorch_shared::{ApiKeyId, AppError, ModelId, ProviderId, RequestId};

#[derive(Debug, Clone)]
pub struct BillingRecord {
    request_id: RequestId,
    api_key_id: ApiKeyId,
    organization_id: uuid::Uuid,
    provider: ProviderId,
    model: ModelId,
    input_tokens: u32,
    output_tokens: u32,
    provider_cost: f64,
    markup_percent: f64,
    status_code: i32,
    latency_ms: i32,
    tags: serde_json::Value,
    error_message: Option<String>,
}

impl BillingRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_id: RequestId,
        api_key_id: ApiKeyId,
        organization_id: uuid::Uuid,
        provider: ProviderId,
        model: ModelId,
        input_tokens: u32,
        output_tokens: u32,
        provider_cost: f64,
        markup_percent: f64,
        status_code: i32,
        latency_ms: i32,
        tags: serde_json::Value,
    ) -> Result<Self, AppError> {
        Self::with_error(
            request_id,
            api_key_id,
            organization_id,
            provider,
            model,
            input_tokens,
            output_tokens,
            provider_cost,
            markup_percent,
            status_code,
            latency_ms,
            tags,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_error(
        request_id: RequestId,
        api_key_id: ApiKeyId,
        organization_id: uuid::Uuid,
        provider: ProviderId,
        model: ModelId,
        input_tokens: u32,
        output_tokens: u32,
        provider_cost: f64,
        markup_percent: f64,
        status_code: i32,
        latency_ms: i32,
        tags: serde_json::Value,
        error_message: Option<String>,
    ) -> Result<Self, AppError> {
        if !provider_cost.is_finite() {
            return Err(AppError::Validation(
                "provider_cost must be finite".to_string(),
            ));
        }
        if !markup_percent.is_finite() {
            return Err(AppError::Validation(
                "markup_percent must be finite".to_string(),
            ));
        }
        if provider_cost < 0.0 {
            return Err(AppError::Validation(
                "provider_cost must be non-negative".to_string(),
            ));
        }
        if markup_percent < 0.0 {
            return Err(AppError::Validation(
                "markup_percent must be non-negative".to_string(),
            ));
        }
        if latency_ms < 0 {
            return Err(AppError::Validation(
                "latency_ms must be non-negative".to_string(),
            ));
        }

        Ok(Self {
            request_id,
            api_key_id,
            organization_id,
            provider,
            model,
            input_tokens,
            output_tokens,
            provider_cost,
            markup_percent,
            status_code,
            latency_ms,
            tags,
            error_message,
        })
    }

    pub fn request_id(&self) -> uuid::Uuid {
        *self.request_id
    }
    pub fn api_key_id(&self) -> uuid::Uuid {
        *self.api_key_id
    }
    pub fn organization_id(&self) -> uuid::Uuid {
        self.organization_id
    }
    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }
    pub fn model(&self) -> &ModelId {
        &self.model
    }
    pub fn input_tokens(&self) -> u32 {
        self.input_tokens
    }
    pub fn output_tokens(&self) -> u32 {
        self.output_tokens
    }
    pub fn provider_cost(&self) -> f64 {
        self.provider_cost
    }
    pub fn markup_percent(&self) -> f64 {
        self.markup_percent
    }
    pub fn status_code(&self) -> i32 {
        self.status_code
    }
    pub fn latency_ms(&self) -> i32 {
        self.latency_ms
    }

    pub fn tags(&self) -> &serde_json::Value {
        &self.tags
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn total_cost(&self) -> f64 {
        self.provider_cost * (1.0 + self.markup_percent / 100.0)
    }
}

pub struct BillingEngine;

impl BillingEngine {
    pub fn new() -> Self {
        Self
    }

    pub async fn record_request(
        &self,
        db_pool: &PgPool,
        record: BillingRecord,
    ) -> Result<(), AppError> {
        let total_cost = record.total_cost();

        sqlx::query(
            r#"
            INSERT INTO requests_log (
                request_id,
                organization_id,
                api_key_id,
                provider,
                model,
                status_code,
                latency_ms,
                input_tokens,
                output_tokens,
                provider_cost,
                markup_percent,
                total_cost,
                tags,
                error_message
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(record.request_id())
        .bind(record.organization_id())
        .bind(record.api_key_id())
        .bind(record.provider().to_string())
        .bind(record.model().to_string())
        .bind(record.status_code())
        .bind(record.latency_ms())
        .bind(record.input_tokens() as i32)
        .bind(record.output_tokens() as i32)
        .bind(record.provider_cost())
        .bind(record.markup_percent())
        .bind(total_cost)
        .bind(record.tags())
        .bind(record.error_message())
        .execute(db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to record request: {}", e)))?;

        Ok(())
    }

    pub fn calculate_total_cost(provider_cost: f64, markup_percent: f64) -> f64 {
        provider_cost * (1.0 + markup_percent / 100.0)
    }
}

impl Default for BillingEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_total_cost_no_markup() {
        let total = BillingEngine::calculate_total_cost(100.0, 0.0);
        assert!((total - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_total_cost_with_markup() {
        let total = BillingEngine::calculate_total_cost(100.0, 20.0);
        assert!((total - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_total_cost_zero_provider_cost() {
        let total = BillingEngine::calculate_total_cost(0.0, 25.0);
        assert!((total - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_total_cost_negative_markup() {
        let total = BillingEngine::calculate_total_cost(100.0, -10.0);
        assert!((total - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_billing_record_new_and_total_cost() {
        let record = BillingRecord::new(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("openai"),
            ModelId::from("gpt-4"),
            100,
            50,
            10.0,
            20.0,
            200,
            1234,
            serde_json::json!([]),
        )
        .unwrap();
        assert_eq!(record.input_tokens(), 100);
        assert_eq!(record.output_tokens(), 50);
        assert_eq!(record.status_code(), 200);
        assert_eq!(record.latency_ms(), 1234);
        assert!((record.total_cost() - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_billing_record_negative_cost_returns_error() {
        let result = BillingRecord::new(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("openai"),
            ModelId::from("gpt-4"),
            1,
            1,
            -1.0,
            0.0,
            200,
            0,
            serde_json::json!([]),
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn test_billing_record_negative_latency_returns_error() {
        let result = BillingRecord::new(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("openai"),
            ModelId::from("gpt-4"),
            1,
            1,
            0.0,
            0.0,
            200,
            -1,
            serde_json::json!([]),
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn test_billing_record_infinite_cost_returns_error() {
        let result = BillingRecord::new(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("openai"),
            ModelId::from("gpt-4"),
            1,
            1,
            f64::INFINITY,
            0.0,
            200,
            0,
            serde_json::json!([]),
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn test_billing_record_with_tags() {
        let tags = serde_json::json!([{"key": "project", "value": "marketing"}]);
        let record = BillingRecord::new(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("openai"),
            ModelId::from("gpt-4"),
            100,
            50,
            10.0,
            20.0,
            200,
            1234,
            tags.clone(),
        )
        .unwrap();
        assert_eq!(record.tags(), &tags);
        // Verify tags are preserved exactly as passed (snapshot contract for requests_log)
        assert_eq!(record.tags().as_array().unwrap().len(), 1);
        assert_eq!(record.tags()[0]["key"], "project");
        assert_eq!(record.tags()[0]["value"], "marketing");
    }

    #[test]
    fn test_billing_record_with_multiple_tags() {
        let tags = serde_json::json!([
            {"key": "project", "value": "marketing"},
            {"key": "team", "value": "ml"},
            {"key": "env", "value": "production"}
        ]);
        let record = BillingRecord::new(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("openai"),
            ModelId::from("gpt-4"),
            100,
            50,
            10.0,
            20.0,
            200,
            1234,
            tags.clone(),
        )
        .unwrap();
        assert_eq!(record.tags(), &tags);
        assert_eq!(record.tags().as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_billing_record_with_empty_tags() {
        let tags = serde_json::json!([]);
        let record = BillingRecord::new(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("openai"),
            ModelId::from("gpt-4"),
            100,
            50,
            10.0,
            20.0,
            200,
            1234,
            tags.clone(),
        )
        .unwrap();
        assert_eq!(record.tags(), &tags);
        assert!(record.tags().as_array().unwrap().is_empty());
    }

    #[test]
    fn test_billing_record_with_error_message() {
        let record = BillingRecord::with_error(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("openai"),
            ModelId::from("gpt-4"),
            0,
            0,
            0.0,
            0.0,
            403,
            0,
            serde_json::json!([]),
            Some("outside_allowed_hours".to_string()),
        )
        .unwrap();
        assert_eq!(record.status_code(), 403);
        assert_eq!(record.error_message(), Some("outside_allowed_hours"));
    }

    #[test]
    fn test_billing_record_access_window_denial() {
        // Verifies the exact constructor call pattern used in auth.rs
        // when an API key is rejected for being outside allowed hours.
        let tags = serde_json::json!([{"key": "project", "value": "marketing"}]);
        let record = BillingRecord::with_error(
            RequestId::new(),
            ApiKeyId::new(),
            uuid::Uuid::new_v4(),
            ProviderId::from("gateway"),
            ModelId::from("access-window"),
            0,
            0,
            0.0,
            0.0,
            403,
            0,
            tags.clone(),
            Some("outside_allowed_hours: window 09:00-18:00 UTC".to_string()),
        )
        .unwrap();
        assert_eq!(record.status_code(), 403);
        assert_eq!(record.provider().to_string(), "gateway");
        assert_eq!(record.model().to_string(), "access-window");
        assert_eq!(record.input_tokens(), 0);
        assert_eq!(record.output_tokens(), 0);
        assert_eq!(record.provider_cost(), 0.0);
        assert_eq!(record.total_cost(), 0.0);
        assert_eq!(record.error_message(), Some("outside_allowed_hours: window 09:00-18:00 UTC"));
        assert_eq!(record.tags(), &tags);
    }
}
