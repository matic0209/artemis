//! Production Error Handling and Recovery
//! 
//! This module provides comprehensive error handling, recovery strategies,
//! and circuit breaker patterns for production deployment.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{error, warn, info, debug};
use tokio::sync::RwLock;
use std::sync::Arc;

use crate::error::{DeFiAnalyzerError, DeFiResult};

/// Production error handler with circuit breaker and retry logic
#[derive(Debug)]
pub struct ProductionErrorHandler {
    /// Circuit breakers for different error types
    circuit_breakers: Arc<RwLock<HashMap<ErrorCategory, CircuitBreaker>>>,
    /// Error statistics
    error_stats: Arc<RwLock<ErrorStatistics>>,
    /// Configuration
    config: ErrorHandlerConfig,
}

/// Circuit breaker for fault tolerance
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Current state
    state: CircuitBreakerState,
    /// Failure count
    failure_count: u32,
    /// Last failure time
    last_failure_time: Option<Instant>,
    /// Configuration
    config: CircuitBreakerConfig,
}

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing fast
    HalfOpen, // Testing recovery
}

/// Error categories for different handling strategies
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ErrorCategory {
    NetworkError,
    ContractCallError,
    ComputationError,
    ValidationError,
    ResourceExhaustion,
    SecurityError,
}

/// Error statistics tracking
#[derive(Debug, Default)]
pub struct ErrorStatistics {
    /// Total errors by category
    pub errors_by_category: HashMap<ErrorCategory, u64>,
    /// Recent errors (last hour)
    pub recent_errors: HashMap<ErrorCategory, u64>,
    /// Recovery success rates
    pub recovery_rates: HashMap<ErrorCategory, f64>,
    /// Last reset time
    pub last_reset: Instant,
}

/// Error handler configuration
#[derive(Debug, Clone)]
pub struct ErrorHandlerConfig {
    /// Circuit breaker configurations by category
    pub circuit_breaker_configs: HashMap<ErrorCategory, CircuitBreakerConfig>,
    /// Global retry configuration
    pub retry_config: RetryConfig,
    /// Enable error reporting
    pub enable_error_reporting: bool,
    /// Error reporting webhook
    pub error_reporting_webhook: Option<String>,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open circuit
    pub failure_threshold: u32,
    /// Recovery timeout duration
    pub recovery_timeout: Duration,
    /// Success threshold to close circuit
    pub success_threshold: u32,
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
}

/// Recovery strategy for different error types
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Immediate retry
    ImmediateRetry,
    /// Exponential backoff retry
    ExponentialBackoff { max_attempts: u32 },
    /// Fallback to simplified analysis
    FallbackAnalysis,
    /// Skip current operation
    Skip,
    /// Shutdown gracefully
    Shutdown,
}

impl Default for ErrorHandlerConfig {
    fn default() -> Self {
        let mut circuit_breaker_configs = HashMap::new();
        
        // Network errors - be more tolerant
        circuit_breaker_configs.insert(ErrorCategory::NetworkError, CircuitBreakerConfig {
            failure_threshold: 10,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 5,
        });
        
        // Contract call errors - moderate tolerance
        circuit_breaker_configs.insert(ErrorCategory::ContractCallError, CircuitBreakerConfig {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(10),
            success_threshold: 3,
        });
        
        // Computation errors - low tolerance
        circuit_breaker_configs.insert(ErrorCategory::ComputationError, CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(5),
            success_threshold: 2,
        });
        
        Self {
            circuit_breaker_configs,
            retry_config: RetryConfig {
                max_attempts: 3,
                base_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(10),
                backoff_multiplier: 2.0,
            },
            enable_error_reporting: true,
            error_reporting_webhook: None,
        }
    }
}

impl ProductionErrorHandler {
    /// Create new production error handler
    pub fn new(config: ErrorHandlerConfig) -> Self {
        let mut circuit_breakers = HashMap::new();
        
        for (category, cb_config) in &config.circuit_breaker_configs {
            circuit_breakers.insert(category.clone(), CircuitBreaker::new(cb_config.clone()));
        }
        
        Self {
            circuit_breakers: Arc::new(RwLock::new(circuit_breakers)),
            error_stats: Arc::new(RwLock::new(ErrorStatistics::default())),
            config,
        }
    }
    
    /// Handle error with appropriate recovery strategy
    pub async fn handle_error(&self, error: &DeFiAnalyzerError) -> RecoveryStrategy {
        let category = self.categorize_error(error);
        
        // Record error statistics
        self.record_error(&category).await;
        
        // Check circuit breaker
        let should_fail_fast = self.should_fail_fast(&category).await;
        if should_fail_fast {
            warn!("Circuit breaker open for {:?}, failing fast", category);
            return RecoveryStrategy::Skip;
        }
        
        // Determine recovery strategy based on error type
        let strategy = match category {
            ErrorCategory::NetworkError => RecoveryStrategy::ExponentialBackoff { max_attempts: 5 },
            ErrorCategory::ContractCallError => RecoveryStrategy::ExponentialBackoff { max_attempts: 3 },
            ErrorCategory::ComputationError => RecoveryStrategy::FallbackAnalysis,
            ErrorCategory::ValidationError => RecoveryStrategy::Skip,
            ErrorCategory::ResourceExhaustion => RecoveryStrategy::Skip,
            ErrorCategory::SecurityError => RecoveryStrategy::Shutdown,
        };
        
        // Report critical errors
        if matches!(category, ErrorCategory::SecurityError) {
            self.report_critical_error(error, &category).await;
        }
        
        strategy
    }
    
    /// Execute operation with error handling and retry logic
    pub async fn execute_with_retry<F, T>(&self, operation: F, category: ErrorCategory) -> DeFiResult<T>
    where
        F: Fn() -> DeFiResult<T> + Send + Sync,
        T: Send,
    {
        let mut attempts = 0;
        let mut delay = self.config.retry_config.base_delay;
        
        loop {
            attempts += 1;
            
            // Check circuit breaker
            if self.should_fail_fast(&category).await {
                return Err(DeFiAnalyzerError::CircuitBreakerOpen(format!("{:?}", category)));
            }
            
            match operation() {
                Ok(result) => {
                    // Record success for circuit breaker
                    self.record_success(&category).await;
                    return Ok(result);
                },
                Err(e) => {
                    error!("Operation failed (attempt {}): {}", attempts, e);
                    
                    // Record failure
                    self.record_error(&category).await;
                    
                    if attempts >= self.config.retry_config.max_attempts {
                        return Err(e);
                    }
                    
                    // Wait before retry with exponential backoff
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(
                        Duration::from_millis((delay.as_millis() as f64 * self.config.retry_config.backoff_multiplier) as u64),
                        self.config.retry_config.max_delay
                    );
                }
            }
        }
    }
    
    /// Categorize error for appropriate handling
    fn categorize_error(&self, error: &DeFiAnalyzerError) -> ErrorCategory {
        match error {
            DeFiAnalyzerError::NetworkError(_) => ErrorCategory::NetworkError,
            DeFiAnalyzerError::ContractCallError(_) => ErrorCategory::ContractCallError,
            DeFiAnalyzerError::ComputationError(_) => ErrorCategory::ComputationError,
            DeFiAnalyzerError::InvalidInput(_) => ErrorCategory::ValidationError,
            DeFiAnalyzerError::ABIFetchError(_) => ErrorCategory::NetworkError,
            DeFiAnalyzerError::Z3Error(_) => ErrorCategory::ComputationError,
            DeFiAnalyzerError::CircuitBreakerOpen(_) => ErrorCategory::ResourceExhaustion,
            _ => ErrorCategory::ComputationError,
        }
    }
    
    /// Check if circuit breaker should fail fast
    async fn should_fail_fast(&self, category: &ErrorCategory) -> bool {
        let circuit_breakers = self.circuit_breakers.read().await;
        if let Some(cb) = circuit_breakers.get(category) {
            cb.state == CircuitBreakerState::Open
        } else {
            false
        }
    }
    
    /// Record error occurrence
    async fn record_error(&self, category: &ErrorCategory) {
        let mut stats = self.error_stats.write().await;
        *stats.errors_by_category.entry(category.clone()).or_insert(0) += 1;
        *stats.recent_errors.entry(category.clone()).or_insert(0) += 1;
        
        // Update circuit breaker
        let mut circuit_breakers = self.circuit_breakers.write().await;
        if let Some(cb) = circuit_breakers.get_mut(category) {
            cb.record_failure();
        }
    }
    
    /// Record successful operation
    async fn record_success(&self, category: &ErrorCategory) {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        if let Some(cb) = circuit_breakers.get_mut(category) {
            cb.record_success();
        }
    }
    
    /// Report critical error
    async fn report_critical_error(&self, error: &DeFiAnalyzerError, category: &ErrorCategory) {
        error!("CRITICAL ERROR [{:?}]: {}", category, error);
        
        if let Some(webhook) = &self.config.error_reporting_webhook {
            // Send webhook notification (simplified)
            debug!("Would send error report to webhook: {}", webhook);
        }
    }
    
    /// Get error statistics
    pub async fn get_error_statistics(&self) -> ErrorStatistics {
        self.error_stats.read().await.clone()
    }
    
    /// Reset error statistics
    pub async fn reset_statistics(&self) {
        let mut stats = self.error_stats.write().await;
        stats.recent_errors.clear();
        stats.last_reset = Instant::now();
    }
}

impl CircuitBreaker {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            last_failure_time: None,
            config,
        }
    }
    
    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());
        
        if self.failure_count >= self.config.failure_threshold {
            self.state = CircuitBreakerState::Open;
            warn!("Circuit breaker opened after {} failures", self.failure_count);
        }
    }
    
    fn record_success(&mut self) {
        match self.state {
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                self.failure_count = 0;
            },
            CircuitBreakerState::HalfOpen => {
                // Close circuit after successful recovery
                self.state = CircuitBreakerState::Closed;
                self.failure_count = 0;
                info!("Circuit breaker closed after successful recovery");
            },
            CircuitBreakerState::Open => {
                // Check if recovery timeout has passed
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() >= self.config.recovery_timeout {
                        self.state = CircuitBreakerState::HalfOpen;
                        info!("Circuit breaker moved to half-open state");
                    }
                }
            }
        }
    }
}

impl Clone for ErrorStatistics {
    fn clone(&self) -> Self {
        Self {
            errors_by_category: self.errors_by_category.clone(),
            recent_errors: self.recent_errors.clone(),
            recovery_rates: self.recovery_rates.clone(),
            last_reset: self.last_reset,
        }
    }
}
