//! Pricing endpoints — set / list / delete per-(provider, model) pricing rows.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use sqlx::Row;
use tracing::info;
use uuid::Uuid;
use zorch_shared::AppError;

use crate::AppState;

use super::types::PricingResponse;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPricingRequest {
    pub provider: String,
    pub model: String,
    pub input_cost_per_1m: f64,
    pub output_cost_per_1m: f64,
    pub markup_percent: f64,
    #[serde(default)]
    pub max_context_tokens: Option<u64>,
}

pub async fn get_pricing(
    State(state): State<AppState>,
) -> Result<Json<Vec<PricingResponse>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id, provider_id, provider, model,
            input_cost_per_1m, output_cost_per_1m,
            markup_percent, max_context_tokens,
            created_at, updated_at
        FROM provider_model_config
        ORDER BY provider, model
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch pricing: {}", e)))?;

    Ok(Json(rows.into_iter().map(row_to_response).collect()))
}

pub async fn set_pricing(
    State(state): State<AppState>,
    Json(req): Json<SetPricingRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    validate_set_pricing(&req)?;

    let provider_id = lookup_provider_id(&state.db_pool, &req.provider).await?;
    validate_target_model_exists(&state.db_pool, provider_id, &req.model).await?;

    let id: Uuid = sqlx::query(
        r#"
        INSERT INTO provider_model_config (
            provider_id, provider, model,
            input_cost_per_1m, output_cost_per_1m,
            markup_percent, max_context_tokens
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (provider_id, model)
        DO UPDATE SET
            input_cost_per_1m = EXCLUDED.input_cost_per_1m,
            output_cost_per_1m = EXCLUDED.output_cost_per_1m,
            markup_percent = EXCLUDED.markup_percent,
            max_context_tokens = EXCLUDED.max_context_tokens,
            provider = EXCLUDED.provider
        RETURNING id
        "#,
    )
    .bind(provider_id)
    .bind(&req.provider)
    .bind(&req.model)
    .bind(req.input_cost_per_1m)
    .bind(req.output_cost_per_1m)
    .bind(req.markup_percent)
    .bind(req.max_context_tokens.unwrap_or(0) as i64)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to set pricing: {}", e)))?
    .try_get("id")
    .map_err(|e| AppError::Internal(format!("Failed to get pricing id: {}", e)))?;

    reload_pricing(&state).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id.to_string(),
            "message": "Pricing set successfully."
        })),
    ))
}

async fn validate_target_model_exists(
    pool: &sqlx::PgPool,
    provider_id: Uuid,
    target_model: &str,
) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM provider_target_models WHERE provider_id = $1 AND target_model = $2)",
    )
    .bind(provider_id)
    .bind(target_model)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to validate target model: {}", e)))?;

    if !exists {
        return Err(AppError::BadRequest(format!(
            "Target model '{}' is not registered for provider '{}'. Add it on the provider's Target Models page first.",
            target_model,
            provider_id
        )));
    }
    Ok(())
}

async fn lookup_provider_id(pool: &sqlx::PgPool, provider_name: &str) -> Result<Uuid, AppError> {
    let row = sqlx::query("SELECT id FROM providers WHERE name = $1")
        .bind(provider_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to lookup provider id: {}", e)))?;

    match row {
        Some(r) => r
            .try_get::<Uuid, _>("id")
            .map_err(|e| AppError::Internal(format!("Failed to read provider id: {}", e))),
        None => Err(AppError::BadRequest(format!(
            "Provider '{}' does not exist. Add it on the Providers page before setting pricing.",
            provider_name
        ))),
    }
}

pub async fn delete_pricing(
    State(state): State<AppState>,
    Path(pricing_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::parse_str(&pricing_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let result = sqlx::query("DELETE FROM provider_model_config WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete pricing: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Pricing entry not found".to_string()));
    }

    reload_pricing(&state).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Hot-reload the in-memory `PricingEngine` from the `provider_model_config` table.
pub(crate) async fn reload_pricing(state: &AppState) -> Result<(), AppError> {
    let rows = sqlx::query(
        r#"
        SELECT provider, model,
               input_cost_per_1m, output_cost_per_1m,
               markup_percent, max_context_tokens
        FROM provider_model_config
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load pricing: {}", e)))?;

    let mut engine = zorch_gateway::PricingEngine::new();
    for row in &rows {
        engine.register(row_to_engine_pricing(row));
    }

    state.pricing.store(Arc::new(engine));
    info!("Hot-reloaded pricing from database");
    Ok(())
}

fn validate_set_pricing(req: &SetPricingRequest) -> Result<(), AppError> {
    if req.provider.is_empty() || req.model.is_empty() {
        return Err(AppError::BadRequest(
            "Provider and model are required".to_string(),
        ));
    }
    if req.input_cost_per_1m < 0.0 || req.output_cost_per_1m < 0.0 || req.markup_percent < 0.0 {
        return Err(AppError::BadRequest(
            "Costs and markup must be non-negative".to_string(),
        ));
    }
    Ok(())
}

fn row_to_response(row: sqlx::postgres::PgRow) -> PricingResponse {
    let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::nil());
    let provider_id: Uuid = row.try_get("provider_id").unwrap_or_else(|_| Uuid::nil());
    let provider: String = row.try_get("provider").unwrap_or_default();
    let model: String = row.try_get("model").unwrap_or_default();
    let input_cost: f64 = row.try_get("input_cost_per_1m").unwrap_or(0.0);
    let output_cost: f64 = row.try_get("output_cost_per_1m").unwrap_or(0.0);
    let markup: f64 = row.try_get("markup_percent").unwrap_or(0.0);
    let max_context_tokens: u64 = row.try_get::<i64, _>("max_context_tokens").unwrap_or(0) as u64;
    let created_at: chrono::DateTime<chrono::Utc> = row
        .try_get("created_at")
        .unwrap_or_else(|_| chrono::Utc::now());
    let updated_at: chrono::DateTime<chrono::Utc> = row
        .try_get("updated_at")
        .unwrap_or_else(|_| chrono::Utc::now());

    PricingResponse {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        provider,
        model,
        input_cost_per_1m: input_cost,
        output_cost_per_1m: output_cost,
        markup_percent: markup,
        max_context_tokens,
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    }
}

fn row_to_engine_pricing(row: &sqlx::postgres::PgRow) -> zorch_gateway::ModelPricing {
    let provider: String = row.try_get("provider").unwrap_or_default();
    let model: String = row.try_get("model").unwrap_or_default();
    let input_cost: f64 = row.try_get("input_cost_per_1m").unwrap_or(0.0);
    let output_cost: f64 = row.try_get("output_cost_per_1m").unwrap_or(0.0);
    let markup: f64 = row.try_get("markup_percent").unwrap_or(0.0);
    let max_context_tokens: u64 = row.try_get::<i64, _>("max_context_tokens").unwrap_or(0) as u64;

    zorch_gateway::ModelPricing {
        provider: zorch_shared::ProviderId::from(provider),
        model: zorch_shared::ModelId::from(model),
        input_cost_per_1m: input_cost,
        output_cost_per_1m: output_cost,
        markup_percent: markup,
        max_context_tokens,
    }
}
