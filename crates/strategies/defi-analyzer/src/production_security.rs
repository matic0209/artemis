//! Production Security and Access Control
//! 
//! This module provides security validations, access controls,
//! and protection mechanisms for production deployment.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use std::net::IpAddr;
use alloy_primitives::{Address, U256};
use tracing::{info, warn, error, debug};
use sha3::{Digest, Keccak256};

use crate::{
    types::{AnalysisEvent, AnalysisAction},
    error::{DeFiResult, DeFiAnalyzerError},
};

/// Production security manager
#[derive(Debug)]
pub struct ProductionSecurity {
    /// Configuration
    config: SecurityConfig,
    /// Rate limiter
    rate_limiter: RateLimiter,
    /// Access validator
    access_validator: AccessValidator,
    /// Transaction validator
    transaction_validator: TransactionValidator,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable authentication
    pub enable_auth: bool,
    /// API keys
    pub api_keys: HashSet<String>,
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    /// Rate limits by endpoint
    pub rate_limits: HashMap<String, u32>,
    /// Whitelisted IPs
    pub whitelisted_ips: HashSet<IpAddr>,
    /// Maximum transaction value (ETH)
    pub max_transaction_value: U256,
    /// Maximum gas limit
    pub max_gas_limit: u64,
    /// Blacklisted contracts
    pub blacklisted_contracts: HashSet<Address>,
    /// Enable transaction validation
    pub enable_tx_validation: bool,
}

/// Rate limiter implementation
#[derive(Debug)]
struct RateLimiter {
    /// Request counts by client
    request_counts: HashMap<String, RequestCount>,
    /// Configuration
    config: RateLimitConfig,
}

/// Request count tracking
#[derive(Debug)]
struct RequestCount {
    /// Number of requests
    count: u32,
    /// Window start time
    window_start: Instant,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
struct RateLimitConfig {
    /// Window duration
    window_duration: Duration,
    /// Default limit per window
    default_limit: u32,
    /// Per-endpoint limits
    endpoint_limits: HashMap<String, u32>,
}

/// Access validator
#[derive(Debug)]
struct AccessValidator {
    /// Valid API keys
    api_keys: HashSet<String>,
    /// Whitelisted IPs
    whitelisted_ips: HashSet<IpAddr>,
    /// Enable IP whitelisting
    enable_ip_whitelist: bool,
}

/// Transaction validator
#[derive(Debug)]
struct TransactionValidator {
    /// Maximum allowed transaction value
    max_value: U256,
    /// Maximum allowed gas limit
    max_gas_limit: u64,
    /// Blacklisted contracts
    blacklisted_contracts: HashSet<Address>,
    /// Enable validation
    enabled: bool,
}

/// Security validation result
#[derive(Debug, PartialEq)]
pub enum ValidationResult {
    Valid,
    Invalid(String),
    Blocked(String),
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_auth: true,
            api_keys: HashSet::new(),
            enable_rate_limiting: true,
            rate_limits: HashMap::new(),
            whitelisted_ips: HashSet::new(),
            max_transaction_value: U256::from(100) * U256::from(10u64.pow(18)), // 100 ETH
            max_gas_limit: 10_000_000,
            blacklisted_contracts: HashSet::new(),
            enable_tx_validation: true,
        }
    }
}

impl ProductionSecurity {
    /// Create new production security manager
    pub fn new(config: SecurityConfig) -> Self {
        let rate_limiter = RateLimiter::new(RateLimitConfig {
            window_duration: Duration::from_secs(60),
            default_limit: 1000,
            endpoint_limits: config.rate_limits.clone(),
        });
        
        let access_validator = AccessValidator::new(
            config.api_keys.clone(),
            config.whitelisted_ips.clone(),
            !config.whitelisted_ips.is_empty()
        );
        
        let transaction_validator = TransactionValidator::new(
            config.max_transaction_value,
            config.max_gas_limit,
            config.blacklisted_contracts.clone(),
            config.enable_tx_validation
        );
        
        Self {
            config,
            rate_limiter,
            access_validator,
            transaction_validator,
        }
    }
    
    /// Validate API request
    pub fn validate_request(&mut self, client_id: &str, endpoint: &str, api_key: Option<&str>, client_ip: Option<IpAddr>) -> ValidationResult {
        // Check authentication
        if self.config.enable_auth {
            match self.access_validator.validate_api_key(api_key) {
                ValidationResult::Invalid(msg) => return ValidationResult::Invalid(msg),
                ValidationResult::Blocked(msg) => return ValidationResult::Blocked(msg),
                _ => {}
            }
        }
        
        // Check IP whitelist
        if let Some(ip) = client_ip {
            match self.access_validator.validate_ip(ip) {
                ValidationResult::Invalid(msg) => return ValidationResult::Invalid(msg),
                ValidationResult::Blocked(msg) => return ValidationResult::Blocked(msg),
                _ => {}
            }
        }
        
        // Check rate limiting
        if self.config.enable_rate_limiting {
            match self.rate_limiter.check_rate_limit(client_id, endpoint) {
                ValidationResult::Blocked(msg) => return ValidationResult::Blocked(msg),
                _ => {}
            }
        }
        
        ValidationResult::Valid
    }
    
    /// Validate analysis event
    pub fn validate_analysis_event(&self, event: &AnalysisEvent) -> ValidationResult {
        // Check contract address against blacklist
        let contract_addr = Address::from(event.contract_address);
        if self.config.blacklisted_contracts.contains(&contract_addr) {
            return ValidationResult::Blocked("Contract address is blacklisted".to_string());
        }
        
        // Check transaction data size
        if event.transaction_data.len() > 100_000 {
            return ValidationResult::Invalid("Transaction data too large".to_string());
        }
        
        // Check block number validity
        if event.block_number == 0 {
            return ValidationResult::Invalid("Invalid block number".to_string());
        }
        
        ValidationResult::Valid
    }
    
    /// Validate analysis action before execution
    pub fn validate_analysis_action(&self, action: &AnalysisAction) -> ValidationResult {
        if !self.config.enable_tx_validation {
            return ValidationResult::Valid;
        }
        
        match self.transaction_validator.validate_transaction(action) {
            ValidationResult::Invalid(msg) => ValidationResult::Invalid(msg),
            ValidationResult::Blocked(msg) => ValidationResult::Blocked(msg),
            _ => ValidationResult::Valid
        }
    }
    
    /// Generate security audit log
    pub fn audit_log(&self, action: &str, client_id: &str, success: bool, details: &str) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        info!(
            "AUDIT: {} | Client: {} | Success: {} | Details: {} | Timestamp: {}",
            action, client_id, success, details, timestamp
        );
    }
}

impl RateLimiter {
    fn new(config: RateLimitConfig) -> Self {
        Self {
            request_counts: HashMap::new(),
            config,
        }
    }
    
    fn check_rate_limit(&mut self, client_id: &str, endpoint: &str) -> ValidationResult {
        let now = Instant::now();
        let limit = self.config.endpoint_limits.get(endpoint)
            .copied()
            .unwrap_or(self.config.default_limit);
        
        let key = format!("{}:{}", client_id, endpoint);
        
        // Clean up old entries
        self.request_counts.retain(|_, count| {
            now.duration_since(count.window_start) < self.config.window_duration
        });
        
        // Check current request count
        let count = self.request_counts.entry(key).or_insert(RequestCount {
            count: 0,
            window_start: now,
        });
        
        if now.duration_since(count.window_start) >= self.config.window_duration {
            // Reset window
            count.count = 0;
            count.window_start = now;
        }
        
        if count.count >= limit {
            ValidationResult::Blocked(format!("Rate limit exceeded: {} requests per minute", limit))
        } else {
            count.count += 1;
            ValidationResult::Valid
        }
    }
}

impl AccessValidator {
    fn new(api_keys: HashSet<String>, whitelisted_ips: HashSet<IpAddr>, enable_ip_whitelist: bool) -> Self {
        Self {
            api_keys,
            whitelisted_ips,
            enable_ip_whitelist,
        }
    }
    
    fn validate_api_key(&self, api_key: Option<&str>) -> ValidationResult {
        match api_key {
            Some(key) => {
                if self.api_keys.contains(key) {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid("Invalid API key".to_string())
                }
            },
            None => ValidationResult::Invalid("API key required".to_string()),
        }
    }
    
    fn validate_ip(&self, ip: IpAddr) -> ValidationResult {
        if !self.enable_ip_whitelist {
            return ValidationResult::Valid;
        }
        
        if self.whitelisted_ips.contains(&ip) {
            ValidationResult::Valid
        } else {
            ValidationResult::Blocked("IP address not whitelisted".to_string())
        }
    }
}

impl TransactionValidator {
    fn new(max_value: U256, max_gas_limit: u64, blacklisted_contracts: HashSet<Address>, enabled: bool) -> Self {
        Self {
            max_value,
            max_gas_limit,
            blacklisted_contracts,
            enabled,
        }
    }
    
    fn validate_transaction(&self, action: &AnalysisAction) -> ValidationResult {
        if !self.enabled {
            return ValidationResult::Valid;
        }
        
        // Check transaction value
        if action.value > self.max_value {
            return ValidationResult::Blocked(format!("Transaction value {} exceeds maximum {}", action.value, self.max_value));
        }
        
        // Check gas limit
        if action.gas_limit > self.max_gas_limit {
            return ValidationResult::Blocked(format!("Gas limit {} exceeds maximum {}", action.gas_limit, self.max_gas_limit));
        }
        
        // Check contract blacklist
        let target_addr = Address::from(action.target_address);
        if self.blacklisted_contracts.contains(&target_addr) {
            return ValidationResult::Blocked("Target contract is blacklisted".to_string());
        }
        
        // Check for suspicious patterns
        if self.is_suspicious_transaction(action) {
            return ValidationResult::Blocked("Transaction flagged as suspicious".to_string());
        }
        
        ValidationResult::Valid
    }
    
    fn is_suspicious_transaction(&self, action: &AnalysisAction) -> bool {
        // Check for MEV sandwich attack patterns
        if action.calldata.len() >= 4 {
            let selector = &action.calldata[0..4];
            // Check for suspicious function selectors
            matches!(selector, [0xab, 0xcd, 0xef, 0x00] | [0x12, 0x34, 0x56, 0x78])
        } else {
            false
        }
    }
}
