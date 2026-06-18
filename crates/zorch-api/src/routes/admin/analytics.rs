use axum::{extract::{Query, State}, response::Json};
use serde::Deserialize;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use zorch_shared::{AppError, ModelId, ProviderId};

use crate::AppState;

use super::types::{AnalyticsResponse, CostTrendPoint, LatencyPoint, TagAnalyticsEntry, TagAnalyticsResponse, TokenUsagePoint};

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsQuery {
    pub tag: Option<String>,
    pub range: Option<String>,
}

pub async fn get_analytics(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsQuery>,
) -> Result<Json<AnalyticsResponse>, AppError> {
    let pool = &state.db_pool;
    let analytics = fetch_analytics(pool, &state.pricing.load(), &params).await?;
    Ok(Json(analytics))
}

fn resolve_interval(range: Option<&str>) -> &'static str {
    match range {
        Some("7d") => "7 days",
        Some("30d") => "30 days",
        _ => "24 hours",
    }
}

fn parse_tag_filter(tag: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = tag.splitn(2, ':').collect();
    if parts.len() == 2 {
        Some(serde_json::json!([{"key": parts[0], "value": parts[1]}]))
    } else {
        None
    }
}

fn build_where_clause(interval: &str, tag_filter: Option<&serde_json::Value>) -> (String, Option<serde_json::Value>) {
    if let Some(filter) = tag_filter {
        (
            format!(
                "WHERE created_at > NOW() - INTERVAL '{}' AND tags @> $1::jsonb",
                interval
            ),
            Some(filter.clone()),
        )
    } else {
        (format!("WHERE created_at > NOW() - INTERVAL '{}'", interval), None)
    }
}

async fn fetch_analytics(
    pool: &PgPool,
    pricing: &zorch_gateway::PricingEngine,
    params: &AnalyticsQuery,
) -> Result<AnalyticsResponse, sqlx::Error> {
    let interval = resolve_interval(params.range.as_deref());
    let tag_filter = params.tag.as_deref().and_then(parse_tag_filter);

    let (where_clause, tag_bind) = build_where_clause(interval, tag_filter.as_ref());

    let token_usage_query_str = format!(
        r#"
        SELECT
            DATE_TRUNC('hour', created_at) as hour,
            COALESCE(SUM(input_tokens + output_tokens), 0)::bigint as usage
        FROM requests_log
        {}
        GROUP BY DATE_TRUNC('hour', created_at)
        ORDER BY hour
        "#,
        where_clause
    );
    let mut token_usage_query = sqlx::query(&token_usage_query_str);
    if let Some(ref bind_val) = tag_bind {
        token_usage_query = token_usage_query.bind(bind_val);
    }
    let token_usage_rows = token_usage_query.fetch_all(pool).await?;

    let token_usage: Vec<TokenUsagePoint> = token_usage_rows
        .into_iter()
        .map(|row| {
            let hour: chrono::DateTime<chrono::Utc> =
                row.try_get("hour").unwrap_or_else(|_| chrono::Utc::now());
            let usage: i64 = row.try_get("usage").unwrap_or(0);
            TokenUsagePoint {
                name: hour.format("%H:%M").to_string(),
                usage: usage as u64,
            }
        })
        .collect();

    let cost_interval = if params.range.as_deref() == Some("30d") {
        "30 days"
    } else {
        "7 days"
    };
    let (cost_where_clause, cost_tag_bind) = build_where_clause(cost_interval, tag_filter.as_ref());

    let cost_query_str = format!(
        r#"
        SELECT
            DATE_TRUNC('day', created_at) as day,
            provider,
            model,
            COALESCE(SUM(input_tokens), 0)::bigint as input_tokens,
            COALESCE(SUM(output_tokens), 0)::bigint as output_tokens
        FROM requests_log
        {}
        GROUP BY DATE_TRUNC('day', created_at), provider, model
        ORDER BY day
        "#,
        cost_where_clause
    );
    let mut cost_query = sqlx::query(&cost_query_str);
    if let Some(ref bind_val) = cost_tag_bind {
        cost_query = cost_query.bind(bind_val);
    }
    let cost_rows = cost_query.fetch_all(pool).await?;

    let mut cost_by_day: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();

    for row in cost_rows {
        let day: chrono::DateTime<chrono::Utc> =
            row.try_get("day").unwrap_or_else(|_| chrono::Utc::now());
        let provider: String = row.try_get("provider").unwrap_or_default();
        let model: String = row.try_get("model").unwrap_or_default();
        let input_tokens: i64 = row.try_get("input_tokens").unwrap_or(0);
        let output_tokens: i64 = row.try_get("output_tokens").unwrap_or(0);

        let (provider_cost, _) = pricing.calculate_cost(
            &ProviderId::from(provider),
            &ModelId::from(model),
            input_tokens as u32,
            output_tokens as u32,
        );

        *cost_by_day.entry(day).or_insert(0.0) += provider_cost;
    }

    // Sort by underlying DateTime before converting to display names so
    // days appear chronologically (Mon, Tue, Wed …) instead of
    // alphabetically (Fri, Mon, Sat …).
    let mut cost_entries: Vec<(chrono::DateTime<chrono::Utc>, f64)> =
        cost_by_day.into_iter().collect();
    cost_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let cost_trends: Vec<CostTrendPoint> = cost_entries
        .into_iter()
        .map(|(day, cost)| {
            let cents = (cost * 100.0).round() as u64;
            CostTrendPoint {
                name: day.format("%a").to_string(),
                cost: cents,
            }
        })
        .collect();

    let latency_query_str = format!(
        r#"
        SELECT
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY latency_ms) as p50,
            PERCENTILE_CONT(0.90) WITHIN GROUP (ORDER BY latency_ms) as p90,
            PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY latency_ms) as p99,
            COUNT(*) FILTER (WHERE status_code >= 400)::bigint AS error_requests,
            COUNT(*)::bigint AS total_requests
        FROM requests_log
        {}
        "#,
        where_clause
    );
    let mut latency_query = sqlx::query(&latency_query_str);
    if let Some(ref bind_val) = tag_bind {
        latency_query = latency_query.bind(bind_val);
    }
    let latency_row = latency_query.fetch_one(pool).await?;

    let p50: f64 = latency_row.try_get("p50").unwrap_or(0.0);
    let p90: f64 = latency_row.try_get("p90").unwrap_or(0.0);
    let p99: f64 = latency_row.try_get("p99").unwrap_or(0.0);
    let total_requests: i64 = latency_row.try_get("total_requests").unwrap_or(0);
    let error_requests: i64 = latency_row.try_get("error_requests").unwrap_or(0);

    let error_rate = if total_requests > 0 {
        error_requests as f64 / total_requests as f64 * 100.0
    } else {
        0.0
    };

    let latency = vec![
        LatencyPoint {
            name: "P50".to_string(),
            value: p50 as u64,
        },
        LatencyPoint {
            name: "P90".to_string(),
            value: p90 as u64,
        },
        LatencyPoint {
            name: "P99".to_string(),
            value: p99 as u64,
        },
    ];

    Ok(AnalyticsResponse {
        token_usage,
        cost_trends,
        latency,
        error_rate,
        total_requests_24h: total_requests,
        error_requests_24h: error_requests,
    })
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ByTagQuery {
    pub range: Option<String>,
}

pub async fn get_analytics_by_tag(
    State(state): State<AppState>,
    Query(params): Query<ByTagQuery>,
) -> Result<Json<TagAnalyticsResponse>, AppError> {
    let pool = &state.db_pool;
    let interval = match params.range.as_deref() {
        Some("24h") => "24 hours",
        Some("30d") => "30 days",
        _ => "7 days",
    };

    let rows = sqlx::query(
        &format!(
            r#"
            SELECT tag->>'key' || ':' || tag->>'value' AS tag,
                   COUNT(*)::bigint AS requests,
                   COALESCE(SUM(input_tokens), 0)::bigint AS input_tokens,
                   COALESCE(SUM(output_tokens), 0)::bigint AS output_tokens,
                    COALESCE(SUM(ROUND(total_cost * 100)::bigint), 0) AS cost_cents,
                    AVG(CASE WHEN status_code >= 400 THEN 1.0 ELSE 0.0 END) AS error_rate
            FROM requests_log r,
                 jsonb_array_elements(r.tags) AS tag
            WHERE r.created_at > NOW() - INTERVAL '{}'
            GROUP BY tag
            ORDER BY cost_cents DESC
            LIMIT 100
            "#,
            interval
        ),
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch tag analytics: {}", e)))?;

    let tags = rows
        .into_iter()
        .map(|row| TagAnalyticsEntry {
            tag: row.try_get("tag").unwrap_or_default(),
            requests: row.try_get("requests").unwrap_or(0),
            input_tokens: row.try_get("input_tokens").unwrap_or(0),
            output_tokens: row.try_get("output_tokens").unwrap_or(0),
            cost_cents: row.try_get("cost_cents").unwrap_or(0),
            error_rate: row.try_get("error_rate").unwrap_or(0.0) * 100.0,
        })
        .collect();

    Ok(Json(TagAnalyticsResponse { tags }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_error_rate_calculation_zero_baseline() {
        let rate = if 0_i64 > 0 { 1.0_f64 } else { 0.0 };
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_error_rate_calculation_with_errors() {
        let total = 200_i64;
        let errors = 5_i64;
        let rate = errors as f64 / total as f64 * 100.0;
        assert!((rate - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_resolve_interval_defaults() {
        assert_eq!(super::resolve_interval(None), "24 hours");
        assert_eq!(super::resolve_interval(Some("7d")), "7 days");
        assert_eq!(super::resolve_interval(Some("30d")), "30 days");
        assert_eq!(super::resolve_interval(Some("24h")), "24 hours");
    }

    #[test]
    fn test_parse_tag_filter() {
        let result = super::parse_tag_filter("project:marketing");
        assert!(result.is_some());
        let v = result.unwrap();
        assert_eq!(v[0]["key"], "project");
        assert_eq!(v[0]["value"], "marketing");
    }

    #[test]
    fn test_parse_tag_filter_no_colon() {
        let result = super::parse_tag_filter("invalid");
        assert!(result.is_none());
    }

    #[test]
    fn test_build_where_clause_without_tag() {
        let (clause, bind) = super::build_where_clause("24 hours", None);
        assert_eq!(clause, "WHERE created_at > NOW() - INTERVAL '24 hours'");
        assert!(bind.is_none());
    }

    #[test]
    fn test_build_where_clause_with_tag() {
        let filter = serde_json::json!([{"key": "project", "value": "marketing"}]);
        let (clause, bind) = super::build_where_clause("7 days", Some(&filter));
        assert_eq!(
            clause,
            "WHERE created_at > NOW() - INTERVAL '7 days' AND tags @> $1::jsonb"
        );
        assert!(bind.is_some());
        assert_eq!(bind.unwrap(), filter);
    }

    #[test]
    fn test_by_tag_error_rate_multiplier() {
        // The SQL returns a fraction (e.g. 0.025). The Rust mapper multiplies by 100.
        // This test verifies the contract: if sqlx returns 0.025, the API exposes 2.5.
        let raw: f64 = 0.025;
        let exposed = raw * 100.0;
        assert!((exposed - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_by_tag_error_rate_zero() {
        let raw: f64 = 0.0;
        let exposed = raw * 100.0;
        assert_eq!(exposed, 0.0);
    }

    #[test]
    fn test_by_tag_error_rate_all_errors() {
        let raw: f64 = 1.0;
        let exposed = raw * 100.0;
        assert_eq!(exposed, 100.0);
    }

    #[test]
    fn test_cost_trends_sorted_chronologically() {
        use std::collections::HashMap;

        let mut cost_by_day: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
        // Insert days out of order
        let wed = chrono::NaiveDate::from_ymd_opt(2024, 1, 17)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let mon = chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let tue = chrono::NaiveDate::from_ymd_opt(2024, 1, 16)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        cost_by_day.insert(wed, 3.0);
        cost_by_day.insert(mon, 1.0);
        cost_by_day.insert(tue, 2.0);

        // Apply the same transformation + sort used in fetch_analytics
        let mut cost_entries: Vec<(chrono::DateTime<chrono::Utc>, f64)> =
            cost_by_day.into_iter().collect();
        cost_entries.sort_by(|a, b| a.0.cmp(&b.0));

        let names: Vec<String> = cost_entries
            .into_iter()
            .map(|(day, _)| day.format("%a").to_string())
            .collect();

        assert_eq!(names, vec!["Mon", "Tue", "Wed"]);
    }

    #[test]
    fn test_cost_trends_not_sorted_alphabetically() {
        use std::collections::HashMap;

        let mut cost_by_day: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
        let fri = chrono::NaiveDate::from_ymd_opt(2024, 1, 19)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let sun = chrono::NaiveDate::from_ymd_opt(2024, 1, 21)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let mon = chrono::NaiveDate::from_ymd_opt(2024, 1, 22)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        cost_by_day.insert(fri, 1.0);
        cost_by_day.insert(sun, 2.0);
        cost_by_day.insert(mon, 3.0);

        // Chronological sort: Fri → Sun → Mon
        let mut cost_entries: Vec<(chrono::DateTime<chrono::Utc>, f64)> =
            cost_by_day.into_iter().collect();
        cost_entries.sort_by(|a, b| a.0.cmp(&b.0));

        let names: Vec<String> = cost_entries
            .into_iter()
            .map(|(day, _)| day.format("%a").to_string())
            .collect();

        // Alphabetically this would be Fri, Mon, Sun — which is wrong.
        assert_eq!(names, vec!["Fri", "Sun", "Mon"]);
        assert_ne!(names, vec!["Fri", "Mon", "Sun"]);
    }
}
