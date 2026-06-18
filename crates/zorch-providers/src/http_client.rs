use std::time::Duration;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use http::HeaderMap;
use rand::{thread_rng, Rng};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::time::sleep;

use crate::errors::ProviderError;

/// Configuration for retry behavior with exponential backoff.
#[derive(Clone, Debug)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 3)
    pub max_retries: u32,
    /// Base delay in milliseconds for exponential backoff (default: 500)
    pub base_delay_ms: u64,
    /// Maximum delay cap in milliseconds (default: 30000)
    pub max_delay_ms: u64,
    /// HTTP status codes that trigger retry (default: [429, 500, 502, 503, 504])
    pub retryable_statuses: Vec<u16>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 500,
            max_delay_ms: 30000,
            retryable_statuses: vec![429, 500, 502, 503, 504],
        }
    }
}

impl RetryConfig {
    /// Create a new RetryConfig with custom values.
    pub fn new(
        max_retries: u32,
        base_delay_ms: u64,
        max_delay_ms: u64,
        retryable_statuses: Vec<u16>,
    ) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            max_delay_ms,
            retryable_statuses,
        }
    }
}

#[derive(Clone)]
pub struct ProviderHttpClient {
    inner: Client,
    retry_config: RetryConfig,
}

impl ProviderHttpClient {
    pub fn new(timeout: Duration) -> Result<Self, ProviderError> {
        Self::with_retry_config(timeout, RetryConfig::default())
    }

    pub fn with_retry_config(
        timeout: Duration,
        retry_config: RetryConfig,
    ) -> Result<Self, ProviderError> {
        let inner = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| ProviderError::Internal(format!("Failed to build HTTP client: {}", e)))?;
        Ok(Self {
            inner,
            retry_config,
        })
    }

    /// Check if a status code is retryable.
    fn is_retryable_status(&self, status: u16) -> bool {
        self.retry_config.retryable_statuses.contains(&status)
    }

    /// Calculate delay with exponential backoff and jitter.
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let base = self.retry_config.base_delay_ms;
        let max = self.retry_config.max_delay_ms;

        let exponential_delay = base.saturating_mul(1u64 << attempt);
        let capped_delay = exponential_delay.min(max);

        let jitter_range = (capped_delay as f64 * 0.2) as u64;
        let jitter = thread_rng().gen_range(0..=jitter_range);

        Duration::from_millis(capped_delay + jitter)
    }

    /// Execute a request with retry logic, returning the Response for further processing.
    async fn execute_with_retry(
        &self,
        request_builder: reqwest::RequestBuilder,
    ) -> Result<Response, ProviderError> {
        let mut last_error: Option<ProviderError> = None;

        for attempt in 0..=self.retry_config.max_retries {
            let mut rb = request_builder.try_clone().ok_or_else(|| {
                ProviderError::Internal("Failed to clone request body for retry".to_string())
            })?;
            if attempt > 0 {
                rb = rb.header("X-Retry-Count", attempt.to_string());
            }
            let result = rb.send().await;

            match result {
                Ok(response) => {
                    let status = response.status().as_u16();

                    if self.is_retryable_status(status) {
                        let body = response.text().await.unwrap_or_default();
                        last_error =
                            Some(ProviderError::Network(format!("HTTP {}: {}", status, body)));

                        if attempt < self.retry_config.max_retries {
                            let delay = self.calculate_delay(attempt);
                            sleep(delay).await;
                            continue;
                        }
                    } else {
                        return Ok(response);
                    }
                }
                Err(e) => {
                    let provider_error = ProviderError::from(e);

                    let is_retryable = matches!(
                        provider_error,
                        ProviderError::Network(_) | ProviderError::Timeout
                    );

                    if is_retryable && attempt < self.retry_config.max_retries {
                        last_error = Some(provider_error);
                        let delay = self.calculate_delay(attempt);
                        sleep(delay).await;
                        continue;
                    } else {
                        return Err(provider_error);
                    }
                }
            }

            break;
        }

        Err(last_error.unwrap_or_else(|| ProviderError::Internal("Retry logic failed".to_string())))
    }

    pub async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Response, ProviderError> {
        let request_builder = self.inner.request(method, url).headers(headers).body(body);
        self.execute_with_retry(request_builder).await
    }

    pub async fn post_json<T: Serialize, R: DeserializeOwned>(
        &self,
        url: &str,
        headers: HeaderMap,
        body: &T,
    ) -> Result<R, ProviderError> {
        let request_builder = self.inner.post(url).headers(headers).json(body);
        let response = self.execute_with_retry(request_builder).await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Network(format!("HTTP {}: {}", status, body)));
        }
        response
            .json::<R>()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))
    }

    pub async fn get_json<R: DeserializeOwned>(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<R, ProviderError> {
        let request_builder = self.inner.get(url).headers(headers);
        let response = self.execute_with_retry(request_builder).await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Network(format!("HTTP {}: {}", status, body)));
        }
        response
            .json::<R>()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))
    }

    pub async fn post_stream(
        &self,
        url: &str,
        headers: HeaderMap,
        body: &impl Serialize,
    ) -> Result<impl Stream<Item = Result<Bytes, ProviderError>>, ProviderError> {
        let request_builder = self.inner.post(url).headers(headers).json(body);
        let response = self.execute_with_retry(request_builder).await?;
        let stream = response
            .bytes_stream()
            .map(|result| result.map_err(|e| ProviderError::Network(e.to_string())));

        Ok(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestBody {
        message: String,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestResponse {
        result: String,
    }

    #[tokio::test]
    async fn test_successful_request_on_first_try() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(TestResponse {
                result: "success".to_string(),
            }))
            .mount(&server)
            .await;

        let client = ProviderHttpClient::new(Duration::from_secs(30)).unwrap();
        let body = TestBody {
            message: "hello".to_string(),
        };

        let result = client
            .post_json::<TestBody, TestResponse>(
                &format!("{}/test", server.uri()),
                HeaderMap::new(),
                &body,
            )
            .await
            .unwrap();

        assert_eq!(result.result, "success");
    }

    #[tokio::test]
    async fn test_retry_on_429_with_retry_after() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(429).set_body_string("Rate limited"))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(TestResponse {
                result: "success".to_string(),
            }))
            .mount(&server)
            .await;

        let mut config = RetryConfig::default();
        config.base_delay_ms = 10;
        config.max_delay_ms = 100;

        let client =
            ProviderHttpClient::with_retry_config(Duration::from_secs(30), config).unwrap();
        let body = TestBody {
            message: "hello".to_string(),
        };

        let result = client
            .post_json::<TestBody, TestResponse>(
                &format!("{}/test", server.uri()),
                HeaderMap::new(),
                &body,
            )
            .await
            .unwrap();

        assert_eq!(result.result, "success");
    }

    #[tokio::test]
    async fn test_retry_on_503_with_exponential_backoff() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(503).set_body_string("Service unavailable"))
            .up_to_n_times(2)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(TestResponse {
                result: "success".to_string(),
            }))
            .mount(&server)
            .await;

        let mut config = RetryConfig::default();
        config.base_delay_ms = 10;
        config.max_delay_ms = 100;

        let client =
            ProviderHttpClient::with_retry_config(Duration::from_secs(30), config).unwrap();

        let result = client
            .get_json::<TestResponse>(&format!("{}/test", server.uri()), HeaderMap::new())
            .await
            .unwrap();

        assert_eq!(result.result, "success");
    }

    #[tokio::test]
    async fn test_max_retries_exhausted_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal server error"))
            .expect(4)
            .mount(&server)
            .await;

        let mut config = RetryConfig::default();
        config.base_delay_ms = 10;
        config.max_delay_ms = 100;
        config.max_retries = 3;

        let client =
            ProviderHttpClient::with_retry_config(Duration::from_secs(30), config).unwrap();
        let body = TestBody {
            message: "hello".to_string(),
        };

        let result = client
            .post_json::<TestBody, TestResponse>(
                &format!("{}/test", server.uri()),
                HeaderMap::new(),
                &body,
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ProviderError::Network(_)));
        assert!(err.to_string().contains("HTTP 500"));
    }

    #[tokio::test]
    async fn test_retry_config_custom_values() {
        let config = RetryConfig::new(5, 1000, 60000, vec![429, 500, 502, 503, 504, 520]);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 60000);
        assert_eq!(
            config.retryable_statuses,
            vec![429, 500, 502, 503, 504, 520]
        );
    }

    #[tokio::test]
    async fn test_retry_header_on_retry_attempt() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .and(header("X-Retry-Count", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(TestResponse {
                result: "success".to_string(),
            }))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(503).set_body_string("Service unavailable"))
            .expect(1)
            .mount(&server)
            .await;

        let mut config = RetryConfig::default();
        config.base_delay_ms = 10;
        config.max_delay_ms = 100;

        let client =
            ProviderHttpClient::with_retry_config(Duration::from_secs(30), config).unwrap();
        let body = TestBody {
            message: "hello".to_string(),
        };

        let result = client
            .post_json::<TestBody, TestResponse>(
                &format!("{}/test", server.uri()),
                HeaderMap::new(),
                &body,
            )
            .await
            .unwrap();

        assert_eq!(result.result, "success");
    }
}
