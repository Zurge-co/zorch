//! Dashboard stats and recent activity endpoints.
//!
//! Stats are derived from the `requests_log` table. Trend percentages compare
//! the last hour against the prior hour so admins can see direction without
//! requiring a time-series store.

use axum::{extract::State, response::Json};
use sqlx::{PgPool, Row};
use zorch_shared::AppError;

use crate::AppState;

use super::types::{DashboardResponse, DashboardStats, RecentActivity};

pub async fn get_dashboard(
    State(state): State<AppState>,
) -> Result<Json<DashboardResponse>, AppError> {
    let pool = &state.db_pool;

    let stats = fetch_dashboard_stats(pool).await?;
    let recent_activity = fetch_recent_activity(pool).await?;

    Ok(Json(DashboardResponse {
        stats,
        recent_activity,
    }))
}

#[derive(sqlx::FromRow)]
struct WindowCounts {
    requests: i64,
    error_requests: i64,
    input_tokens: i64,
    output_tokens: i64,
}

async fn count_window(pool: &PgPool, seconds: i64) -> Result<WindowCounts, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*)::bigint AS requests,
            COUNT(*) FILTER (WHERE status_code >= 400)::bigint AS error_requests,
            COALESCE(SUM(input_tokens), 0)::bigint AS input_tokens,
            COALESCE(SUM(output_tokens), 0)::bigint AS output_tokens
        FROM requests_log
        WHERE created_at > NOW() - make_interval(secs => $1)
        "#,
    )
    .bind(seconds as f64)
    .fetch_one(pool)
    .await?;

    Ok(WindowCounts {
        requests: row.try_get("requests").unwrap_or(0),
        error_requests: row.try_get("error_requests").unwrap_or(0),
        input_tokens: row.try_get("input_tokens").unwrap_or(0),
        output_tokens: row.try_get("output_tokens").unwrap_or(0),
    })
}

fn format_count(value: i64) -> String {
    let abs = value.unsigned_abs();
    if abs >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if abs >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn format_percent(value: f64) -> String {
    format!("{:.2}%", value.clamp(0.0, 100.0))
}

fn trend_percent(current: i64, previous: i64) -> f64 {
    if previous <= 0 {
        return 0.0;
    }
    ((current as f64 - previous as f64) / previous as f64) * 100.0
}

async fn fetch_dashboard_stats(pool: &PgPool) -> Result<DashboardStats, sqlx::Error> {
    let last_hour = count_window(pool, 3600).await?;
    let prior_hour = count_window(pool, 3600).await?;

    let last_24h = count_window(pool, 86_400).await?;

    let now_requests = last_hour.requests;
    let now_tokens = last_hour.input_tokens + last_hour.output_tokens;

    let prev_requests = prior_hour.requests.saturating_sub(now_requests).max(0);
    let prev_tokens = prior_hour
        .input_tokens
        .saturating_sub(last_hour.input_tokens)
        + prior_hour
            .output_tokens
            .saturating_sub(last_hour.output_tokens);
    let prev_tokens = prev_tokens.max(0);

    let error_rate = if last_24h.requests > 0 {
        last_24h.error_requests as f64 / last_24h.requests as f64 * 100.0
    } else {
        0.0
    };

    let active_providers: i64 = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)::bigint
        FROM providers
        WHERE is_active = true
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    Ok(DashboardStats {
        requests_per_minute: format_count(now_requests / 60),
        tokens_per_minute: format_count(now_tokens / 60),
        error_rate: format_percent(error_rate),
        active_providers: active_providers as u32,
        requests_trend_percent: trend_percent(now_requests, prev_requests),
        tokens_trend_percent: trend_percent(now_tokens, prev_tokens),
        error_rate_trend_percent: 0.0,
        requests_last_24h: last_24h.requests,
        tokens_last_24h: last_24h.input_tokens + last_24h.output_tokens,
        error_requests_last_24h: last_24h.error_requests,
    })
}

async fn fetch_recent_activity(pool: &PgPool) -> Result<Vec<RecentActivity>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            api_key_id,
            provider,
            model,
            status_code,
            created_at,
            latency_ms
        FROM requests_log
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let id: uuid::Uuid = row.try_get("id").unwrap_or_else(|_| uuid::Uuid::nil());
            let api_key_id: uuid::Uuid = row
                .try_get("api_key_id")
                .unwrap_or_else(|_| uuid::Uuid::nil());
            let provider: String = row
                .try_get("provider")
                .unwrap_or_else(|_| "Unknown".to_string());
            let model: String = row
                .try_get("model")
                .unwrap_or_else(|_| "unknown".to_string());
            let status_code: i32 = row.try_get("status_code").unwrap_or(200);
            let created_at: chrono::DateTime<chrono::Utc> = row
                .try_get("created_at")
                .unwrap_or_else(|_| chrono::Utc::now());
            let latency_ms: Option<i32> = row.try_get("latency_ms").ok();

            RecentActivity {
                id: id.to_string(),
                key_id: format!(
                    "key_{}",
                    &api_key_id.to_string()[..8.min(api_key_id.to_string().len())]
                ),
                provider,
                model,
                status: if status_code >= 400 {
                    "error"
                } else {
                    "success"
                }
                .to_string(),
                timestamp: created_at.to_rfc3339(),
                latency: latency_ms
                    .map(|ms| format!("{}ms", ms))
                    .unwrap_or_else(|| "N/A".to_string()),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_count() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(123), "123");
        assert_eq!(format_count(1500), "1.5k");
        assert_eq!(format_count(1_500_000), "1.5M");
    }

    #[test]
    fn test_format_percent_clamps() {
        assert_eq!(format_percent(0.0), "0.00%");
        assert_eq!(format_percent(50.0), "50.00%");
        assert_eq!(format_percent(150.0), "100.00%");
        assert_eq!(format_percent(-5.0), "0.00%");
    }

    #[test]
    fn test_trend_percent_zero_baseline() {
        assert_eq!(trend_percent(100, 0), 0.0);
    }

    #[test]
    fn test_trend_percent_positive_and_negative() {
        assert!((trend_percent(150, 100) - 50.0).abs() < 0.01);
        assert!((trend_percent(50, 100) - -50.0).abs() < 0.01);
        assert_eq!(trend_percent(100, 100), 0.0);
    }
}
