//! DeFi Analyzer Error Handling

use thiserror::Error;

/// DeFi Analyzer specific errors
#[derive(Error, Debug)]
pub enum DeFiAnalyzerError {
    /// Analysis timeout
    #[error("Analysis timeout after {timeout_ms}ms")]
    AnalysisTimeout { timeout_ms: u64 },

    /// Symbolic execution failed
    #[error("Symbolic execution failed: {reason}")]
    SymbolicExecutionFailed { reason: String },

    /// Feature extraction failed
    #[error("Feature extraction failed: {reason}")]
    FeatureExtractionFailed { reason: String },

    /// Documentation comparison failed
    #[error("Documentation comparison failed: {reason}")]
    DocumentationComparisonFailed { reason: String },

    /// Risk assessment failed
    #[error("Risk assessment failed: {reason}")]
    RiskAssessmentFailed { reason: String },

    /// Cache operation failed
    #[error("Cache operation failed: {reason}")]
    CacheOperationFailed { reason: String },

    /// Configuration error
    #[error("Configuration error: {reason}")]
    ConfigurationError { reason: String },

    /// Network error
    #[error("Network error: {reason}")]
    NetworkError { reason: String },

    /// Insufficient resources
    #[error("Insufficient resources: {resource_type}")]
    InsufficientResources { resource_type: String },

    /// Analysis depth exceeded
    #[error("Analysis depth {depth} exceeded maximum {max_depth}")]
    AnalysisDepthExceeded { depth: u32, max_depth: u32 },

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Computation error
    #[error("Computation error: {0}")]
    ComputationError(String),

    /// ABI fetch error
    #[error("ABI fetch error: {0}")]
    ABIFetchError(String),

    /// Circuit breaker open
    #[error("Circuit breaker open: {0}")]
    CircuitBreakerOpen(String),

    /// Execution error
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// Z3 error
    #[error("Z3 error: {0}")]
    Z3Error(String),

    /// Contract call error
    #[error("Contract call error: {0}")]
    ContractCallError(String),
}

/// Result type for DeFi Analyzer operations
pub type DeFiResult<T> = Result<T, DeFiAnalyzerError>;

/// Error recovery strategies
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry with exponential backoff
    RetryWithBackoff { max_retries: u32, base_delay_ms: u64 },
    /// Use cached result
    UseCachedResult,
    /// Skip analysis
    SkipAnalysis,
    /// Reduce analysis depth
    ReduceAnalysisDepth,
    /// Use simplified analysis
    UseSimplifiedAnalysis,
}

impl DeFiAnalyzerError {
    /// Get recovery strategy for this error
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            DeFiAnalyzerError::AnalysisTimeout { .. } => {
                RecoveryStrategy::RetryWithBackoff { max_retries: 3, base_delay_ms: 100 }
            },
            DeFiAnalyzerError::SymbolicExecutionFailed { .. } => {
                RecoveryStrategy::UseSimplifiedAnalysis
            },
            DeFiAnalyzerError::FeatureExtractionFailed { .. } => {
                RecoveryStrategy::UseCachedResult
            },
            DeFiAnalyzerError::DocumentationComparisonFailed { .. } => {
                RecoveryStrategy::SkipAnalysis
            },
            DeFiAnalyzerError::RiskAssessmentFailed { .. } => {
                RecoveryStrategy::UseCachedResult
            },
            DeFiAnalyzerError::CacheOperationFailed { .. } => {
                RecoveryStrategy::SkipAnalysis
            },
            DeFiAnalyzerError::ConfigurationError { .. } => {
                RecoveryStrategy::SkipAnalysis
            },
            DeFiAnalyzerError::NetworkError { .. } => {
                RecoveryStrategy::RetryWithBackoff { max_retries: 5, base_delay_ms: 200 }
            },
            DeFiAnalyzerError::InsufficientResources { .. } => {
                RecoveryStrategy::ReduceAnalysisDepth
            },
            DeFiAnalyzerError::AnalysisDepthExceeded { .. } => {
                RecoveryStrategy::ReduceAnalysisDepth
            },
            DeFiAnalyzerError::InvalidInput(_) => RecoveryStrategy::SkipAnalysis,
            DeFiAnalyzerError::ComputationError(_) => RecoveryStrategy::UseSimplifiedAnalysis,
            DeFiAnalyzerError::ABIFetchError(_) => RecoveryStrategy::UseCachedResult,
            DeFiAnalyzerError::CircuitBreakerOpen(_) => RecoveryStrategy::SkipAnalysis,
            DeFiAnalyzerError::ExecutionError(_) => RecoveryStrategy::RetryWithBackoff { max_retries: 2, base_delay_ms: 250 },
            DeFiAnalyzerError::Z3Error(_) => RecoveryStrategy::UseSimplifiedAnalysis,
            DeFiAnalyzerError::ContractCallError(_) => RecoveryStrategy::RetryWithBackoff { max_retries: 3, base_delay_ms: 300 },
        }
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            DeFiAnalyzerError::AnalysisTimeout { .. } => true,
            DeFiAnalyzerError::SymbolicExecutionFailed { .. } => true,
            DeFiAnalyzerError::FeatureExtractionFailed { .. } => true,
            DeFiAnalyzerError::DocumentationComparisonFailed { .. } => true,
            DeFiAnalyzerError::RiskAssessmentFailed { .. } => true,
            DeFiAnalyzerError::CacheOperationFailed { .. } => true,
            DeFiAnalyzerError::NetworkError { .. } => true,
            DeFiAnalyzerError::InsufficientResources { .. } => true,
            DeFiAnalyzerError::AnalysisDepthExceeded { .. } => true,
            DeFiAnalyzerError::InvalidInput(_) => false,
            DeFiAnalyzerError::ComputationError(_) => true,
            DeFiAnalyzerError::ABIFetchError(_) => true,
            DeFiAnalyzerError::CircuitBreakerOpen(_) => true,
            DeFiAnalyzerError::ExecutionError(_) => true,
            DeFiAnalyzerError::Z3Error(_) => true,
            DeFiAnalyzerError::ContractCallError(_) => true,
            DeFiAnalyzerError::ConfigurationError { .. } => false,
        }
    }
}
