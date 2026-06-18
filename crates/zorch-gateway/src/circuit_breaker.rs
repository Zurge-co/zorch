use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use zorch_shared::{AppError, ProviderId};

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
struct ProviderCircuit {
    state: CircuitState,
    failures: u32,
    success_count: u32,
    last_failure: Option<Instant>,
    threshold: u32,
    timeout: Duration,
    half_open_max_calls: u32,
}

impl ProviderCircuit {
    fn new(threshold: u32, timeout: Duration, half_open_max_calls: u32) -> Self {
        Self {
            state: CircuitState::Closed,
            failures: 0,
            success_count: 0,
            last_failure: None,
            threshold,
            timeout,
            half_open_max_calls,
        }
    }

    fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failures = 0;
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.half_open_max_calls {
                    self.state = CircuitState::Closed;
                    self.failures = 0;
                    self.success_count = 0;
                }
            }
            CircuitState::Open => {
                // Should not happen; Open state blocks calls.
            }
        }
    }

    fn record_failure(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failures += 1;
                self.last_failure = Some(Instant::now());
                if self.failures >= self.threshold {
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.last_failure = Some(Instant::now());
                self.success_count = 0;
            }
            CircuitState::Open => {
                self.last_failure = Some(Instant::now());
            }
        }
    }

    fn is_healthy(&self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has elapsed — transition to HalfOpen
                if let Some(last) = self.last_failure {
                    last.elapsed() >= self.timeout
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    fn transition_if_needed(&mut self) {
        if self.state == CircuitState::Open {
            if let Some(last) = self.last_failure {
                if last.elapsed() >= self.timeout {
                    self.state = CircuitState::HalfOpen;
                    self.success_count = 0;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct CircuitBreaker {
    circuits: Arc<Mutex<HashMap<ProviderId, ProviderCircuit>>>,
    default_threshold: u32,
    default_timeout: Duration,
    default_half_open_max_calls: u32,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            circuits: Arc::new(Mutex::new(HashMap::new())),
            default_threshold: 5,
            default_timeout: Duration::from_secs(30),
            default_half_open_max_calls: 3,
        }
    }

    pub fn with_config(
        mut self,
        threshold: u32,
        timeout: Duration,
        half_open_max_calls: u32,
    ) -> Self {
        self.default_threshold = threshold;
        self.default_timeout = timeout;
        self.default_half_open_max_calls = half_open_max_calls;
        self
    }

    /// Record a successful request for the given provider.
    pub async fn record_success(&self, provider_id: &ProviderId) {
        let mut circuits = self.circuits.lock().await;
        let circuit = circuits
            .entry(provider_id.clone())
            .or_insert_with(|| self.make_circuit());
        circuit.record_success();
    }

    /// Record a failed request for the given provider.
    pub async fn record_failure(&self, provider_id: &ProviderId) {
        let mut circuits = self.circuits.lock().await;
        let circuit = circuits
            .entry(provider_id.clone())
            .or_insert_with(|| self.make_circuit());
        circuit.record_failure();
    }

    /// Returns true if provider is considered healthy (Closed or HalfOpen after timeout).
    pub async fn is_provider_healthy(&self, provider_id: &ProviderId) -> Result<bool, AppError> {
        let mut circuits = self.circuits.lock().await;
        let circuit = circuits
            .entry(provider_id.clone())
            .or_insert_with(|| self.make_circuit());
        circuit.transition_if_needed();
        Ok(circuit.is_healthy())
    }

    /// Get current circuit state for a provider (for observability).
    pub async fn get_state(&self, provider_id: &ProviderId) -> Result<CircuitState, AppError> {
        let mut circuits = self.circuits.lock().await;
        let circuit = circuits
            .entry(provider_id.clone())
            .or_insert_with(|| self.make_circuit());
        circuit.transition_if_needed();
        Ok(circuit.state.clone())
    }

    fn make_circuit(&self) -> ProviderCircuit {
        ProviderCircuit::new(
            self.default_threshold,
            self.default_timeout,
            self.default_half_open_max_calls,
        )
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_starts_closed() {
        let cb = CircuitBreaker::new();
        let provider = ProviderId::from("openai");
        assert!(cb.is_provider_healthy(&provider).await.unwrap());
        assert_eq!(cb.get_state(&provider).await.unwrap(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_opens_after_threshold() {
        let cb = CircuitBreaker::new().with_config(3, Duration::from_secs(60), 2);
        let provider = ProviderId::from("openai");

        cb.record_failure(&provider).await;
        cb.record_failure(&provider).await;
        assert!(cb.is_provider_healthy(&provider).await.unwrap());

        cb.record_failure(&provider).await;
        assert!(!cb.is_provider_healthy(&provider).await.unwrap());
        assert_eq!(cb.get_state(&provider).await.unwrap(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_half_open_then_closes() {
        let cb = CircuitBreaker::new().with_config(2, Duration::from_millis(1), 2);
        let provider = ProviderId::from("openai");

        cb.record_failure(&provider).await;
        cb.record_failure(&provider).await;
        assert_eq!(cb.get_state(&provider).await.unwrap(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(2));

        assert!(cb.is_provider_healthy(&provider).await.unwrap());
        assert_eq!(
            cb.get_state(&provider).await.unwrap(),
            CircuitState::HalfOpen
        );

        cb.record_success(&provider).await;
        cb.record_success(&provider).await;
        assert_eq!(cb.get_state(&provider).await.unwrap(), CircuitState::Closed);
        assert!(cb.is_provider_healthy(&provider).await.unwrap());
    }

    #[tokio::test]
    async fn test_circuit_half_open_then_reopens() {
        let cb = CircuitBreaker::new().with_config(2, Duration::from_millis(1), 2);
        let provider = ProviderId::from("openai");

        cb.record_failure(&provider).await;
        cb.record_failure(&provider).await;
        assert_eq!(cb.get_state(&provider).await.unwrap(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(2));

        assert!(cb.is_provider_healthy(&provider).await.unwrap());
        assert_eq!(
            cb.get_state(&provider).await.unwrap(),
            CircuitState::HalfOpen
        );

        cb.record_failure(&provider).await;
        assert!(!cb.is_provider_healthy(&provider).await.unwrap());
        assert_eq!(cb.get_state(&provider).await.unwrap(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_success_resets_failures_in_closed() {
        let cb = CircuitBreaker::new().with_config(3, Duration::from_secs(60), 2);
        let provider = ProviderId::from("openai");

        cb.record_failure(&provider).await;
        cb.record_failure(&provider).await;
        cb.record_success(&provider).await;
        // failures should be reset, need 3 more to open
        cb.record_failure(&provider).await;
        cb.record_failure(&provider).await;
        assert!(cb.is_provider_healthy(&provider).await.unwrap());

        cb.record_failure(&provider).await;
        assert!(!cb.is_provider_healthy(&provider).await.unwrap());
    }

    #[tokio::test]
    async fn test_default_circuit_breaker() {
        let cb = CircuitBreaker::default();
        let provider = ProviderId::from("anthropic");
        assert!(cb.is_provider_healthy(&provider).await.unwrap());
    }
}
