//! Unified error handling system for Artemis
//!
//! This module provides a comprehensive error handling framework that replaces
//! all unwrap() and expect() calls with proper error propagation.

use thiserror::Error;

/// Main error type for Artemis operations
#[derive(Error, Debug, Clone)]
pub enum ArtemisError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("JSON-RPC error: {0}")]
    JsonRpc(String),
    
    #[error("Provider error: {0}")]
    Provider(String),
    
    #[error("Signing error: {0}")]
    Signing(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Strategy error: {0}")]
    Strategy(String),
    
    #[error("Collector error: {0}")]
    Collector(String),
    
    #[error("Executor error: {0}")]
    Executor(String),
    
    #[error("State management error: {0}")]
    StateManagement(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("IO error: {0}")]
    Io(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("CSV parsing error: {0}")]
    Csv(String),
    
    #[error("Alloy provider error: {0}")]
    AlloyProvider(String),
    
    #[error("MEV error: {0}")]
    Mev(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for Artemis operations
pub type Result<T> = std::result::Result<T, ArtemisError>;

impl ArtemisError {
    /// Create a configuration error with context
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
    
    /// Create a provider error with context
    pub fn provider(msg: impl Into<String>) -> Self {
        Self::Provider(msg.into())
    }
    
    /// Create a signing error with context
    pub fn signing(msg: impl Into<String>) -> Self {
        Self::Signing(msg.into())
    }
    
    /// Create a transaction error with context
    pub fn transaction(msg: impl Into<String>) -> Self {
        Self::Transaction(msg.into())
    }
    
    /// Create a strategy error with context
    pub fn strategy(msg: impl Into<String>) -> Self {
        Self::Strategy(msg.into())
    }
    
    /// Create a collector error with context
    pub fn collector(msg: impl Into<String>) -> Self {
        Self::Collector(msg.into())
    }
    
    /// Create an executor error with context
    pub fn executor(msg: impl Into<String>) -> Self {
        Self::Executor(msg.into())
    }
    
    /// Create a parse error with context
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }
    
    /// Create a validation error with context
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }
    
    /// Create an MEV error with context
    pub fn mev(msg: impl Into<String>) -> Self {
        Self::Mev(msg.into())
    }
    
    /// Create an internal error with context
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Extension trait for Result types to add context
pub trait ResultExt<T, E> {
    /// Add context to the error
    fn with_context<F>(self, f: F) -> std::result::Result<T, anyhow::Error>
    where
        F: FnOnce() -> String;
    
    /// Add context with a closure that takes the error
    fn with_context_from<F>(self, f: F) -> std::result::Result<T, anyhow::Error>
    where
        F: FnOnce(&E) -> String;
}

impl<T, E> ResultExt<T, E> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_context<F>(self, f: F) -> std::result::Result<T, anyhow::Error>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| anyhow::Error::new(e).context(f()))
    }
    
    fn with_context_from<F>(self, f: F) -> std::result::Result<T, anyhow::Error>
    where
        F: FnOnce(&E) -> String,
    {
        self.map_err(|e| {
            let context = f(&e);
            anyhow::Error::new(e).context(context)
        })
    }
}

/// Helper macros for common error handling patterns
#[macro_export]
macro_rules! ensure {
    ($condition:expr, $($arg:tt)*) => {
        if !($condition) {
            return Err($crate::error::ArtemisError::validation(format!($($arg)*)));
        }
    };
}

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::error::ArtemisError::internal(format!($($arg)*)));
    };
}

/// Safe parsing utilities that return ArtemisError instead of panicking
pub mod safe_parse {
    use super::*;
    use crate::eth::Address;
    
    /// Safely parse an address from a string
    pub fn address(s: &str) -> Result<Address> {
        s.parse::<Address>()
            .map_err(|e| ArtemisError::parse(format!("Invalid address '{}': {}", s, e)))
    }
    
    /// Safely parse a private key
    pub fn private_key(s: &str) -> Result<crate::eth::LocalWallet> {
        use crate::eth::LocalWallet;
        s.parse::<LocalWallet>()
            .map_err(|e| ArtemisError::parse(format!("Invalid private key: {}", e)))
    }
    
    /// Safely parse a U256 from a string
    pub fn u256(s: &str) -> Result<crate::eth::U256> {
        s.parse::<crate::eth::U256>()
            .map_err(|e| ArtemisError::parse(format!("Invalid U256 '{}': {}", s, e)))
    }
}

/// Error handling utilities for common operations
pub mod utils {
    use super::*;
    use std::fmt::Display;
    
    /// Convert anyhow::Error to ArtemisError
    pub fn from_anyhow(err: anyhow::Error) -> ArtemisError {
        // Try to downcast to our error types first
        if let Some(artemis_err) = err.downcast_ref::<ArtemisError>() {
            return artemis_err.clone();
        }
        
        // Otherwise wrap as internal error
        ArtemisError::internal(err.to_string())
    }
    
    /// Wrap an error with additional context
    pub fn wrap<T, E>(result: std::result::Result<T, E>, context: impl Display) -> Result<T>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        result.map_err(|e| {
            ArtemisError::internal(format!("{}: {}", context, e))
        })
    }
    
    /// Handle optional values with meaningful error messages
    pub fn require<T>(value: Option<T>, field_name: &str) -> Result<T> {
        value.ok_or_else(|| ArtemisError::validation(format!("Required field '{}' is missing", field_name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let err = ArtemisError::config("test config error");
        assert!(matches!(err, ArtemisError::Config(_)));
        
        let err = ArtemisError::provider("test provider error");
        assert!(matches!(err, ArtemisError::Provider(_)));
    }
    
    #[test]
    fn test_safe_parse() {
        // Valid address
        let addr = safe_parse::address("0x0000000000000000000000000000000000000000").unwrap();
        assert_eq!(addr.to_string(), "0x0000000000000000000000000000000000000000");
        
        // Invalid address
        let err = safe_parse::address("invalid").unwrap_err();
        assert!(matches!(err, ArtemisError::Parse(_)));
    }
}
