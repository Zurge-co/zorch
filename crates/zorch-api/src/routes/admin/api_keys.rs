use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::{PgPool, Row};
use zorch_shared::{validate_tags, ApiKeyTag, AppError};

use crate::AppState;

use super::types::{ApiKeyResponse, ApiKeysResponse};

fn generate_api_key() -> (String, String) {
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let plaintext = format!("sk-zorch-{}", suffix);
    let mut hasher = sha2::Sha256::new();
    hasher.update(plaintext.as_bytes());
    let hash = hex::encode(hasher.finalize());
    (plaintext, hash)
}

pub async fn get_api_keys(
    State(state): State<AppState>,
) -> Result<Json<ApiKeysResponse>, AppError> {
    let pool = &state.db_pool;
    let keys = fetch_api_keys(pool).await?;
    Ok(Json(ApiKeysResponse { keys }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub expires_in_days: Option<i64>,
    #[serde(default)]
    pub tags: Vec<ApiKeyTag>,
    pub allowed_hours_start: Option<u8>,
    pub allowed_hours_end: Option<u8>,
    pub window_timezone: Option<String>,
    pub requests_per_minute: Option<i32>,
    pub requests_per_day: Option<i32>,
    pub max_spend_usd: Option<f64>,
    #[serde(default)]
    pub allowed_models: Option<Vec<String>>,
}

pub async fn create_api_key(
    State(state): State<AppState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let pool = &state.db_pool;

    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }

    validate_tags(&req.tags).map_err(AppError::Validation)?;
    validate_governance(&req)?;

    validate_access_window(
        req.allowed_hours_start,
        req.allowed_hours_end,
        req.window_timezone.as_deref(),
    )?;

    let (plaintext, key_hash) = generate_api_key();
    let scopes = if req.scopes.is_empty() {
        vec!["default".to_string()]
    } else {
        req.scopes
    };
    let expires_at = req
        .expires_in_days
        .map(|days| chrono::Utc::now() + chrono::Duration::days(days));

    let tags_json = serde_json::to_value(&req.tags).unwrap_or(serde_json::json!([]));

    let org_id = uuid::Uuid::new_v4();
    let org_name = format!("Org for {}", name);

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Database(format!("Failed to begin transaction: {}", e)))?;

    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
        .bind(org_id)
        .bind(&org_name)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(format!("Failed to create organization: {}", e)))?;

    let api_key_id: uuid::Uuid = sqlx::query(
        "INSERT INTO api_keys (organization_id, name, key_hash, scopes, expires_at, is_active, tags, allowed_hours_start, allowed_hours_end, window_timezone, requests_per_minute, requests_per_day, max_spend_usd, allowed_models)
         VALUES ($1, $2, $3, $4, $5, true, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id",
    )
    .bind(org_id)
    .bind(name)
    .bind(&key_hash)
    .bind(&scopes)
    .bind(expires_at)
    .bind(&tags_json)
    .bind(req.allowed_hours_start.map(|h| h as i16))
    .bind(req.allowed_hours_end.map(|h| h as i16))
    .bind(&req.window_timezone)
    .bind(req.requests_per_minute)
    .bind(req.requests_per_day)
    .bind(req.max_spend_usd)
    .bind(req.allowed_models.as_ref())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::Database(format!("Failed to create API key: {}", e)))?
    .try_get("id")
    .map_err(|e| AppError::Internal(format!("Failed to get created API key id: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Database(format!("Failed to commit API key transaction: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "key": plaintext,
            "hash": key_hash,
            "id": api_key_id.to_string(),
            "name": name,
            "message": "Store this key securely. It will not be shown again."
        })),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiKeyRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<ApiKeyTag>>,
    pub allowed_hours_start: Option<Option<i16>>,
    pub allowed_hours_end: Option<Option<i16>>,
    pub window_timezone: Option<Option<String>>,
    pub requests_per_minute: Option<Option<i32>>,
    pub requests_per_day: Option<Option<i32>>,
    pub max_spend_usd: Option<Option<f64>>,
    pub allowed_models: Option<Option<Vec<String>>>,
}

fn build_update_sql(req: &UpdateApiKeyRequest) -> Result<(String, u8), AppError> {
    let mut sets: Vec<String> = Vec::new();
    let mut param_idx: u8 = 1;

    if req.name.is_some() {
        sets.push(format!("name = ${}", param_idx));
        param_idx += 1;
    }
    if req.scopes.is_some() {
        sets.push(format!("scopes = ${}", param_idx));
        param_idx += 1;
    }
    if req.tags.is_some() {
        sets.push(format!("tags = ${}", param_idx));
        param_idx += 1;
    }
    if req.allowed_hours_start.is_some() {
        sets.push(format!("allowed_hours_start = ${}", param_idx));
        param_idx += 1;
    }
    if req.allowed_hours_end.is_some() {
        sets.push(format!("allowed_hours_end = ${}", param_idx));
        param_idx += 1;
    }
    if req.window_timezone.is_some() {
        sets.push(format!("window_timezone = ${}", param_idx));
        param_idx += 1;
    }
    if req.requests_per_minute.is_some() {
        sets.push(format!("requests_per_minute = ${}", param_idx));
        param_idx += 1;
    }
    if req.requests_per_day.is_some() {
        sets.push(format!("requests_per_day = ${}", param_idx));
        param_idx += 1;
    }
    if req.max_spend_usd.is_some() {
        sets.push(format!("max_spend_usd = ${}", param_idx));
        param_idx += 1;
    }
    if req.allowed_models.is_some() {
        sets.push(format!("allowed_models = ${}", param_idx));
        param_idx += 1;
    }

    if sets.is_empty() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let sql = format!(
        "UPDATE api_keys SET {} WHERE id = ${} RETURNING id",
        sets.join(", "),
        param_idx
    );
    Ok((sql, param_idx))
}

pub async fn update_api_key(
    State(state): State<AppState>,
    Path(key_id): Path<String>,
    Json(req): Json<UpdateApiKeyRequest>,
) -> Result<Json<ApiKeyResponse>, AppError> {
    let pool = &state.db_pool;
    let id = uuid::Uuid::parse_str(&key_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    if let Some(ref tags) = req.tags {
        validate_tags(tags).map_err(AppError::Validation)?;
    }

    if let (Some(start), Some(end)) = (req.allowed_hours_start, req.allowed_hours_end) {
        let start_u8 = start.map(|h| h as u8);
        let end_u8 = end.map(|h| h as u8);
        let tz = req.window_timezone.as_ref().and_then(|opt| opt.as_deref());
        validate_access_window(start_u8, end_u8, tz)?;
    }

    if let Some(Some(rpm)) = req.requests_per_minute {
        if !(1..=1_000_000).contains(&rpm) {
            return Err(AppError::Validation(
                "requests_per_minute must be between 1 and 1,000,000".to_string(),
            ));
        }
    }
    if let Some(Some(rpd)) = req.requests_per_day {
        if !(1..=10_000_000).contains(&rpd) {
            return Err(AppError::Validation(
                "requests_per_day must be between 1 and 10,000,000".to_string(),
            ));
        }
    }
    if let Some(Some(budget)) = req.max_spend_usd {
        if !(0.0..=1_000_000.0).contains(&budget) {
            return Err(AppError::Validation(
                "max_spend_usd must be between 0 and 1,000,000".to_string(),
            ));
        }
    }
    if let Some(Some(ref models)) = req.allowed_models {
        if models.len() > 128 {
            return Err(AppError::Validation(
                "allowed_models must not exceed 128 entries".to_string(),
            ));
        }
        for m in models {
            if m.is_empty() || m.len() > 128 {
                return Err(AppError::Validation(
                    "Each allowed model must be 1-128 characters".to_string(),
                ));
            }
        }
    }

    let (sql, _) = build_update_sql(&req)?;

    let tags_json = req
        .tags
        .as_ref()
        .map(|t| serde_json::to_value(t).unwrap_or(serde_json::json!([])));

    let mut query = sqlx::query(&sql);
    if let Some(name) = req.name.as_deref() {
        query = query.bind(name);
    }
    if let Some(scopes) = req.scopes.as_ref() {
        query = query.bind(scopes);
    }
    if let Some(ref tags) = tags_json {
        query = query.bind(tags);
    }
    if let Some(start) = req.allowed_hours_start {
        query = query.bind(start);
    }
    if let Some(end) = req.allowed_hours_end {
        query = query.bind(end);
    }
    if let Some(tz) = req.window_timezone {
        query = query.bind(tz);
    }
    if let Some(rpm) = req.requests_per_minute {
        query = query.bind(rpm);
    }
    if let Some(rpd) = req.requests_per_day {
        query = query.bind(rpd);
    }
    if let Some(budget) = req.max_spend_usd {
        query = query.bind(budget);
    }
    if let Some(models) = req.allowed_models {
        query = query.bind(models);
    }
    query = query.bind(id);

    let result = query
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update API key: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    let keys = fetch_api_keys_with_filter(pool, &format!("WHERE id = '{}'", id)).await?;
    Ok(Json(keys.into_iter().next().ok_or_else(|| {
        AppError::NotFound("API key not found after update".to_string())
    })?))
}

pub async fn replace_api_key_tags(
    State(state): State<AppState>,
    Path(key_id): Path<String>,
    Json(tags): Json<Vec<ApiKeyTag>>,
) -> Result<Json<ApiKeyResponse>, AppError> {
    let pool = &state.db_pool;
    let id = uuid::Uuid::parse_str(&key_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    validate_tags(&tags).map_err(AppError::Validation)?;

    let tags_json = serde_json::to_value(&tags).unwrap_or(serde_json::json!([]));

    let result = sqlx::query("UPDATE api_keys SET tags = $1 WHERE id = $2")
        .bind(&tags_json)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update tags: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    let keys = fetch_api_keys_with_filter(pool, &format!("WHERE id = '{}'", id)).await?;
    Ok(Json(keys.into_iter().next().ok_or_else(|| {
        AppError::NotFound("API key not found after update".to_string())
    })?))
}

pub async fn revoke_api_key(
    State(state): State<AppState>,
    Path(key_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let pool = &state.db_pool;

    let id = uuid::Uuid::parse_str(&key_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let result = sqlx::query("UPDATE api_keys SET is_active = false WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to revoke API key: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

fn validate_access_window(
    start: Option<u8>,
    end: Option<u8>,
    tz: Option<&str>,
) -> Result<(), AppError> {
    match (start, end) {
        (Some(s), Some(e)) => {
            if s > 23 || e > 23 {
                return Err(AppError::Validation(
                    "Access window hours must be 0-23".to_string(),
                ));
            }
            if let Some(tz_str) = tz {
                tz_str
                    .parse::<chrono_tz::Tz>()
                    .map_err(|_| AppError::Validation(format!("Invalid timezone: {}", tz_str)))?;
            }
            Ok(())
        }
        (Some(_), None) | (None, Some(_)) => Err(AppError::Validation(
            "Both allowed_hours_start and allowed_hours_end must be set together".to_string(),
        )),
        (None, None) => {
            if tz.is_some() {
                return Err(AppError::Validation(
                    "window_timezone requires both allowed_hours_start and allowed_hours_end"
                        .to_string(),
                ));
            }
            Ok(())
        }
    }
}

fn validate_governance(req: &CreateApiKeyRequest) -> Result<(), AppError> {
    if let Some(rpm) = req.requests_per_minute {
        if !(1..=1_000_000).contains(&rpm) {
            return Err(AppError::Validation(
                "requests_per_minute must be between 1 and 1,000,000".to_string(),
            ));
        }
    }
    if let Some(rpd) = req.requests_per_day {
        if !(1..=10_000_000).contains(&rpd) {
            return Err(AppError::Validation(
                "requests_per_day must be between 1 and 10,000,000".to_string(),
            ));
        }
    }
    if let Some(budget) = req.max_spend_usd {
        if !(0.0..=1_000_000.0).contains(&budget) {
            return Err(AppError::Validation(
                "max_spend_usd must be between 0 and 1,000,000".to_string(),
            ));
        }
    }
    if let Some(ref models) = req.allowed_models {
        if models.len() > 128 {
            return Err(AppError::Validation(
                "allowed_models must not exceed 128 entries".to_string(),
            ));
        }
        for m in models {
            if m.is_empty() || m.len() > 128 {
                return Err(AppError::Validation(
                    "Each allowed model must be 1-128 characters".to_string(),
                ));
            }
        }
    }
    Ok(())
}

async fn fetch_api_keys(pool: &PgPool) -> Result<Vec<ApiKeyResponse>, sqlx::Error> {
    fetch_api_keys_with_filter(pool, "ORDER BY created_at DESC").await
}

async fn fetch_api_keys_with_filter(
    pool: &PgPool,
    filter: &str,
) -> Result<Vec<ApiKeyResponse>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        r#"
            SELECT
                id,
                organization_id,
                name,
                key_hash,
                scopes,
                expires_at,
                is_active,
                created_at,
                tags,
                allowed_hours_start,
                allowed_hours_end,
                window_timezone,
                requests_per_minute,
                requests_per_day,
                max_spend_usd,
                allowed_models
            FROM api_keys
            {}"#,
        filter
    ))
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let id: uuid::Uuid = row.try_get("id").unwrap_or_else(|_| uuid::Uuid::nil());
            let key_hash: String = row
                .try_get("key_hash")
                .unwrap_or_else(|_| "unknown".to_string());
            let name: String = row
                .try_get("name")
                .unwrap_or_else(|_| "Unnamed".to_string());
            let is_active: bool = row.try_get("is_active").unwrap_or(false);
            let created_at: chrono::DateTime<chrono::Utc> = row
                .try_get("created_at")
                .unwrap_or_else(|_| chrono::Utc::now());

            let tags_json: serde_json::Value = row.try_get("tags").unwrap_or(serde_json::json!([]));
            let tags: Vec<ApiKeyTag> = serde_json::from_value(tags_json).unwrap_or_default();

            let allowed_hours_start: Option<i16> = row.try_get("allowed_hours_start").ok();
            let allowed_hours_end: Option<i16> = row.try_get("allowed_hours_end").ok();
            let window_timezone: Option<String> = row.try_get("window_timezone").ok();

            let requests_per_minute: Option<i32> = row.try_get("requests_per_minute").ok();
            let requests_per_day: Option<i32> = row.try_get("requests_per_day").ok();
            let max_spend_usd: Option<f64> = row.try_get("max_spend_usd").ok();
            let allowed_models: Option<Vec<String>> = row.try_get("allowed_models").ok();

            ApiKeyResponse {
                id: id.to_string(),
                name,
                key: format!("sk-zorch-{}...", &key_hash[..8.min(key_hash.len())]),
                status: if is_active { "active" } else { "revoked" }.to_string(),
                created_at: created_at.format("%Y-%m-%d").to_string(),
                usage: "0 tokens".to_string(),
                tags,
                allowed_hours_start: allowed_hours_start.map(|h| h as u8),
                allowed_hours_end: allowed_hours_end.map(|h| h as u8),
                window_timezone,
                requests_per_minute,
                requests_per_day,
                max_spend_usd,
                allowed_models,
            }
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyMiddlewareConfigResponse {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub priority: i32,
}

pub async fn get_api_key_middleware_configs(
    State(state): State<AppState>,
    Path(key_id): Path<String>,
) -> Result<Json<Vec<ApiKeyMiddlewareConfigResponse>>, AppError> {
    let id = uuid::Uuid::parse_str(&key_id)
        .map_err(|_| AppError::BadRequest("Invalid UUID format".to_string()))?;

    let rows = sqlx::query(
        r#"
        SELECT
            mc.id::text,
            mc.name,
            mc.phase,
            mc.priority
        FROM middleware_configs mc
        JOIN api_key_middleware_configs akmc ON akmc.middleware_config_id = mc.id
        WHERE akmc.api_key_id = $1
          AND mc.enabled = true
        ORDER BY mc.priority ASC, mc.name ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to list API key middleware configs: {}", e)))?;

    let configs = rows
        .into_iter()
        .map(|row| ApiKeyMiddlewareConfigResponse {
            id: row.try_get("id").unwrap_or_default(),
            name: row.try_get("name").unwrap_or_default(),
            phase: row.try_get("phase").unwrap_or_default(),
            priority: row.try_get("priority").unwrap_or(100),
        })
        .collect();

    Ok(Json(configs))
}

pub async fn assign_api_key_middleware_config(
    State(state): State<AppState>,
    Path((key_id, config_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let api_key_id = uuid::Uuid::parse_str(&key_id)
        .map_err(|_| AppError::BadRequest("Invalid API key UUID format".to_string()))?;
    let middleware_config_id = uuid::Uuid::parse_str(&config_id)
        .map_err(|_| AppError::BadRequest("Invalid middleware config UUID format".to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO api_key_middleware_configs (api_key_id, middleware_config_id)
        VALUES ($1, $2)
        ON CONFLICT (api_key_id, middleware_config_id) DO NOTHING
        "#,
    )
    .bind(api_key_id)
    .bind(middleware_config_id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to assign middleware config: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn unassign_api_key_middleware_config(
    State(state): State<AppState>,
    Path((key_id, config_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let api_key_id = uuid::Uuid::parse_str(&key_id)
        .map_err(|_| AppError::BadRequest("Invalid API key UUID format".to_string()))?;
    let middleware_config_id = uuid::Uuid::parse_str(&config_id)
        .map_err(|_| AppError::BadRequest("Invalid middleware config UUID format".to_string()))?;

    let result = sqlx::query(
        "DELETE FROM api_key_middleware_configs WHERE api_key_id = $1 AND middleware_config_id = $2"
    )
    .bind(api_key_id)
    .bind(middleware_config_id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to unassign middleware config: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Middleware config assignment not found".to_string(),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::UpdateApiKeyRequest;
    use zorch_shared::{validate_tags, ApiKeyTag};

    #[test]
    fn valid_tag_passes() {
        let tags = vec![ApiKeyTag {
            key: "project".to_string(),
            value: "marketing".to_string(),
        }];
        assert!(validate_tags(&tags).is_ok());
    }

    #[test]
    fn uppercase_key_rejected() {
        let tags = vec![ApiKeyTag {
            key: "Project".to_string(),
            value: "marketing".to_string(),
        }];
        assert!(validate_tags(&tags).is_err());
    }

    #[test]
    fn key_too_long_rejected() {
        let tags = vec![ApiKeyTag {
            key: "a".repeat(33),
            value: "val".to_string(),
        }];
        assert!(validate_tags(&tags).is_err());
    }

    #[test]
    fn empty_key_rejected() {
        let tags = vec![ApiKeyTag {
            key: "".to_string(),
            value: "val".to_string(),
        }];
        assert!(validate_tags(&tags).is_err());
    }

    #[test]
    fn value_too_long_rejected() {
        let tags = vec![ApiKeyTag {
            key: "k".to_string(),
            value: "v".repeat(129),
        }];
        assert!(validate_tags(&tags).is_err());
    }

    #[test]
    fn empty_value_rejected() {
        let tags = vec![ApiKeyTag {
            key: "k".to_string(),
            value: "".to_string(),
        }];
        assert!(validate_tags(&tags).is_err());
    }

    #[test]
    fn too_many_tags_rejected() {
        let tags: Vec<ApiKeyTag> = (0..17)
            .map(|i| ApiKeyTag {
                key: format!("k{}", i),
                value: "v".to_string(),
            })
            .collect();
        assert!(validate_tags(&tags).is_err());
    }

    #[test]
    fn duplicate_key_rejected() {
        let tags = vec![
            ApiKeyTag {
                key: "project".to_string(),
                value: "a".to_string(),
            },
            ApiKeyTag {
                key: "project".to_string(),
                value: "b".to_string(),
            },
        ];
        assert!(validate_tags(&tags).is_err());
    }

    #[test]
    fn max_16_tags_accepted() {
        let tags: Vec<ApiKeyTag> = (0..16)
            .map(|i| ApiKeyTag {
                key: format!("k{}", i),
                value: "v".to_string(),
            })
            .collect();
        assert!(validate_tags(&tags).is_ok());
    }

    #[test]
    fn build_update_sql_single_field() {
        let req = UpdateApiKeyRequest {
            name: Some("test".to_string()),
            scopes: None,
            tags: None,
            allowed_hours_start: None,
            allowed_hours_end: None,
            window_timezone: None,
            requests_per_minute: None,
            requests_per_day: None,
            max_spend_usd: None,
            allowed_models: None,
        };
        let (sql, param_count) = super::build_update_sql(&req).unwrap();
        assert!(sql.contains("name = $1"));
        assert!(sql.contains("WHERE id = $2"));
        assert_eq!(param_count, 2);
    }

    #[test]
    fn build_update_sql_multiple_fields_sequential_params() {
        let req = UpdateApiKeyRequest {
            name: Some("test".to_string()),
            scopes: None,
            tags: Some(vec![ApiKeyTag {
                key: "project".to_string(),
                value: "marketing".to_string(),
            }]),
            allowed_hours_start: None,
            allowed_hours_end: None,
            window_timezone: Some(Some("UTC".to_string())),
            requests_per_minute: None,
            requests_per_day: None,
            max_spend_usd: None,
            allowed_models: None,
        };
        let (sql, param_count) = super::build_update_sql(&req).unwrap();
        assert!(sql.contains("name = $1"));
        assert!(sql.contains("tags = $2"));
        assert!(sql.contains("window_timezone = $3"));
        assert!(sql.contains("WHERE id = $4"));
        assert!(!sql.contains("scopes ="));
        assert_eq!(param_count, 4);
    }

    #[test]
    fn build_update_sql_empty_fields_rejected() {
        let req = UpdateApiKeyRequest {
            name: None,
            scopes: None,
            tags: None,
            allowed_hours_start: None,
            allowed_hours_end: None,
            window_timezone: None,
            requests_per_minute: None,
            requests_per_day: None,
            max_spend_usd: None,
            allowed_models: None,
        };
        let result = super::build_update_sql(&req);
        assert!(result.is_err());
    }
}
