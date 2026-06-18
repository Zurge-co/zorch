use serde_json::Value;
use sqlx::PgPool;

pub struct MiddlewareAudit;

pub struct MiddlewareRunRecord<'a> {
    pub request_id: &'a str,
    pub plugin_key: &'a str,
    pub phase: &'a str,
    pub status: &'a str,
    pub action: &'a str,
    pub duration_ms: i32,
    pub body_changed: bool,
    pub metadata: Value,
    pub error: Option<String>,
}

impl MiddlewareAudit {
    pub fn new() -> Self {
        Self
    }

    pub async fn record_run(
        &self,
        pool: &PgPool,
        record: MiddlewareRunRecord<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO middleware_runs
                (request_id, plugin_key, phase, status, action, duration_ms, body_changed, metadata, error)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(record.request_id)
        .bind(record.plugin_key)
        .bind(record.phase)
        .bind(record.status)
        .bind(record.action)
        .bind(record.duration_ms)
        .bind(record.body_changed)
        .bind(record.metadata)
        .bind(record.error)
        .execute(pool)
        .await?;

        Ok(())
    }
}

impl Default for MiddlewareAudit {
    fn default() -> Self {
        Self::new()
    }
}
