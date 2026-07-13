use axum::body::Body;
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use sha2::Digest;
use zorch_db::ApiKey;
use zorch_shared::{ApiKeyId, AppError, OrgId, RequestId};

pub async fn middleware(
    State(state): State<crate::AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let path = req.uri().path();

    if path == "/health" || path == "/health/ready" || path == "/api-docs" {
        return Ok(next.run(req).await);
    }

    if path.starts_with("/admin/") || path.starts_with("/api/v1/admin/") {
        let admin_secret = req
            .headers()
            .get("X-Admin-Secret")
            .and_then(|v| v.to_str().ok());

        let cfg_secret = state.config.admin_secret.as_deref().unwrap_or("");
        if !cfg_secret.is_empty() {
            if let Some(provided) = admin_secret {
                if constant_time_eq(provided, cfg_secret) {
                    return Ok(next.run(req).await);
                }
            }
        }

        let auth_header = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        if let Some(auth_header) = auth_header {
            let token = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header);
            let key_hash = hash_token(token);

            if let Some(api_key) = sqlx::query_as::<_, ApiKey>(
                "SELECT id, organization_id, name, key_hash, scopes, expires_at, is_active, created_at,
                        requests_per_minute, requests_per_day, max_spend_usd, allowed_models, tags,
                        allowed_hours_start, allowed_hours_end, window_timezone
                 FROM api_keys
                 WHERE key_hash = $1",
            )
            .bind(&key_hash)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            {
                if api_key.is_active {
                    if let Some(expires_at) = api_key.expires_at {
                        if chrono::Utc::now() > expires_at {
                            return Err(AppError::Auth("API key has expired".to_string()));
                        }
                    }
                    if api_key.scopes.iter().any(|s| s == "admin") {
                        req.extensions_mut().insert(ApiKeyId::from(api_key.id));
                        req.extensions_mut().insert(OrgId::from(api_key.organization_id));
                        return Ok(next.run(req).await);
                    }
                }
            }
        }

        return Err(AppError::Auth("Admin access denied".to_string()));
    }

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Auth("Missing Authorization header".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Auth("Invalid Authorization format".to_string()))?;

    let key_hash = hash_token(token);

    let api_key = sqlx::query_as::<_, ApiKey>(
        "SELECT id, organization_id, name, key_hash, scopes, expires_at, is_active, created_at,
                allowed_models, max_spend_usd, requests_per_minute, requests_per_day, tags,
                allowed_hours_start, allowed_hours_end, window_timezone
         FROM api_keys
         WHERE key_hash = $1",
    )
    .bind(&key_hash)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::Auth("Invalid or expired API key".to_string()))?;

    if !api_key.is_active {
        return Err(AppError::Auth("API key is inactive".to_string()));
    }

    if let Some(expires_at) = api_key.expires_at {
        if chrono::Utc::now() > expires_at {
            return Err(AppError::Auth("API key has expired".to_string()));
        }
    }

    match access_window_check(&api_key, chrono::Utc::now()) {
        Ok(()) => {}
        Err(AppError::AccessWindow {
            start,
            end,
            timezone: tz,
        }) => {
            let request_id = RequestId::new();
            let error_msg = format!(
                "outside_allowed_hours: window {}{}-{}{} {}",
                start, ":00", end, ":00", tz
            );
            let record = zorch_gateway::BillingRecord::with_error(
                request_id,
                ApiKeyId::from(api_key.id),
                *OrgId::from(api_key.organization_id),
                zorch_shared::ProviderId::from("gateway"),
                None,
                zorch_shared::ModelId::from("access-window"),
                zorch_shared::ModelId::from("access-window"),
                0,
                0,
                0.0,
                0.0,
                403,
                0,
                0,
                0,
                api_key.tags.clone(),
                Some(error_msg),
            );
            if let Ok(record) = record {
                let _ = state.billing.record_request(&state.db_pool, record).await;
            }
            return Err(AppError::AccessWindow {
                start,
                end,
                timezone: tz,
            });
        }
        Err(e) => return Err(e),
    }

    req.extensions_mut().insert(ApiKeyId::from(api_key.id));
    req.extensions_mut()
        .insert(OrgId::from(api_key.organization_id));
    req.extensions_mut().insert(api_key);

    Ok(next.run(req).await)
}

/// Pure access-window decision logic for a resolved API key.
/// Returns `Ok(())` if the key may proceed, or an `AppError::AccessWindow`
/// describing the restriction when it should be blocked.
pub fn access_window_check(
    api_key: &ApiKey,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), AppError> {
    if api_key.scopes.iter().any(|s| s == "admin") {
        return Ok(());
    }
    let window = zorch_gateway::AccessWindow {
        allowed_hours_start: api_key.allowed_hours_start.map(|h| h as u8),
        allowed_hours_end: api_key.allowed_hours_end.map(|h| h as u8),
        window_timezone: api_key.window_timezone.clone(),
    };
    if !window.is_within_window_at(now)? {
        let start = window.allowed_hours_start.unwrap_or(0);
        let end = window.allowed_hours_end.unwrap_or(0);
        let tz = window.window_timezone.unwrap_or_else(|| "UTC".to_string());
        return Err(AppError::AccessWindow {
            start,
            end,
            timezone: tz,
        });
    }
    Ok(())
}

fn hash_token(token: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::access_window_check;
    use uuid::Uuid;
    use zorch_db::ApiKey;
    use zorch_shared::AppError;

    fn api_key_with_window(
        start: Option<i16>,
        end: Option<i16>,
        tz: Option<&str>,
        scopes: &[&str],
    ) -> ApiKey {
        ApiKey {
            id: Uuid::new_v4(),
            organization_id: Uuid::new_v4(),
            name: "test".to_string(),
            key_hash: "abc123".to_string(),
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
            expires_at: None,
            is_active: true,
            created_at: chrono::Utc::now(),
            requests_per_minute: None,
            requests_per_day: None,
            max_spend_usd: None,
            allowed_models: None,
            tags: serde_json::json!([]),
            allowed_hours_start: start,
            allowed_hours_end: end,
            window_timezone: tz.map(String::from),
        }
    }

    fn hour_utc(h: u32) -> chrono::DateTime<chrono::Utc> {
        chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(h, 0, 0)
            .unwrap()
            .and_utc()
    }

    #[test]
    fn no_window_always_allows() {
        let key = api_key_with_window(None, None, None, &["default"]);
        assert!(access_window_check(&key, hour_utc(12)).is_ok());
    }

    #[test]
    fn inside_window_allows() {
        let key = api_key_with_window(Some(9), Some(18), Some("UTC"), &["default"]);
        assert!(access_window_check(&key, hour_utc(12)).is_ok());
    }

    #[test]
    fn outside_window_blocks() {
        let key = api_key_with_window(Some(9), Some(18), Some("UTC"), &["default"]);
        let result = access_window_check(&key, hour_utc(6));
        if let Err(AppError::AccessWindow {
            start,
            end,
            timezone,
        }) = result
        {
            assert_eq!(start, 9);
            assert_eq!(end, 18);
            assert_eq!(timezone, "UTC");
        } else {
            panic!("Expected AccessWindow error for hour 06 outside 9-18 UTC window");
        }
    }

    #[test]
    fn admin_bypasses_window() {
        let key = api_key_with_window(Some(9), Some(18), Some("UTC"), &["admin"]);
        assert!(access_window_check(&key, hour_utc(6)).is_ok());
    }

    #[test]
    fn wraparound_inside_window() {
        let key = api_key_with_window(Some(22), Some(6), Some("UTC"), &["default"]);
        assert!(access_window_check(&key, hour_utc(23)).is_ok());
        assert!(access_window_check(&key, hour_utc(3)).is_ok());
    }

    #[test]
    fn wraparound_outside_window() {
        let key = api_key_with_window(Some(22), Some(6), Some("UTC"), &["default"]);
        let result = access_window_check(&key, hour_utc(12));
        if let Err(AppError::AccessWindow {
            start,
            end,
            timezone,
        }) = result
        {
            assert_eq!(start, 22);
            assert_eq!(end, 6);
            assert_eq!(timezone, "UTC");
        } else {
            panic!("Expected AccessWindow error for hour 12 outside 22-6 UTC wraparound window");
        }
    }

    #[test]
    fn timezone_conversion_affects_result() {
        // 03:00 UTC = 12:00 Asia/Bangkok, inside 9-18 Bangkok window
        let key = api_key_with_window(Some(9), Some(18), Some("Asia/Bangkok"), &["default"]);
        assert!(access_window_check(&key, hour_utc(3)).is_ok());
    }
}
