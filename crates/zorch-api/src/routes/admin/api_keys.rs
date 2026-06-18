use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;
use sha2::Digest;
use sqlx::{PgPool, Row};
use zorch_shared::{AppError, ApiKeyTag, validate_tags};

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

    sqlx::query(
        "INSERT INTO api_keys (organization_id, name, key_hash, scopes, expires_at, is_active, tags, allowed_hours_start, allowed_hours_end, window_timezone)
         VALUES ($1, $2, $3, $4, $5, true, $6, $7, $8, $9)",
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
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::Database(format!("Failed to create API key: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Database(format!("Failed to commit API key transaction: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "key": plaintext,
            "hash": key_hash,
            "id": org_id.to_string(),
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

    let (sql, _) = build_update_sql(&req)?;

    let tags_json = req.tags.as_ref().map(|t| serde_json::to_value(t).unwrap_or(serde_json::json!([])));

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
    if let Some(ref tz) = req.window_timezone {
        query = query.bind(tz);
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
    Ok(Json(keys.into_iter().next().ok_or_else(|| AppError::NotFound("API key not found after update".to_string()))?))
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

    let result = sqlx::query(
        "UPDATE api_keys SET tags = $1 WHERE id = $2"
    )
    .bind(&tags_json)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to update tags: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    let keys = fetch_api_keys_with_filter(pool, &format!("WHERE id = '{}'", id)).await?;
    Ok(Json(keys.into_iter().next().ok_or_else(|| AppError::NotFound("API key not found after update".to_string()))?))
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

async fn fetch_api_keys(pool: &PgPool) -> Result<Vec<ApiKeyResponse>, sqlx::Error> {
    fetch_api_keys_with_filter(pool, "ORDER BY created_at DESC").await
}

async fn fetch_api_keys_with_filter(
    pool: &PgPool,
    filter: &str,
) -> Result<Vec<ApiKeyResponse>, sqlx::Error> {
    let rows = sqlx::query(
        &format!(
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
                window_timezone
            FROM api_keys
            {}"#,
            filter
        ),
    )
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

            let tags_json: serde_json::Value =
                row.try_get("tags").unwrap_or(serde_json::json!([]));
            let tags: Vec<ApiKeyTag> =
                serde_json::from_value(tags_json).unwrap_or_default();

            let allowed_hours_start: Option<i16> = row.try_get("allowed_hours_start").ok();
            let allowed_hours_end: Option<i16> = row.try_get("allowed_hours_end").ok();
            let window_timezone: Option<String> = row.try_get("window_timezone").ok();

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
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use zorch_shared::{ApiKeyTag, validate_tags};
    use super::UpdateApiKeyRequest;

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
        };
        let result = super::build_update_sql(&req);
        assert!(result.is_err());
    }
}
