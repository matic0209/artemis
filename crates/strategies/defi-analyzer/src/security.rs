//! Security Enhancement Module

use alloy_primitives::{Address, U256};
use anyhow::Result;
use tracing::{info, debug, warn, error};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{
    types::AnalysisEvent,
    error::{DeFiResult, DeFiAnalyzerError},
};

/// Security manager
pub struct SecurityManager {
    /// Rate limiting
    rate_limiter: RateLimiter,
    /// Input validator
    input_validator: InputValidator,
    /// Key manager
    key_manager: KeyManager,
    /// Security audit
    security_audit: SecurityAudit,
    /// Configuration
    config: SecurityConfig,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    /// Enable input validation
    pub enable_input_validation: bool,
    /// Enable key management
    pub enable_key_management: bool,
    /// Enable security audit
    pub enable_security_audit: bool,
    /// Rate limit per minute
    pub rate_limit_per_minute: u32,
    /// Max input size
    pub max_input_size: usize,
    /// Key rotation interval
    pub key_rotation_interval: Duration,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_rate_limiting: true,
            enable_input_validation: true,
            enable_key_management: true,
            enable_security_audit: true,
            rate_limit_per_minute: 100,
            max_input_size: 1024 * 1024, // 1MB
            key_rotation_interval: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Rate limiter
pub struct RateLimiter {
    /// Request counts per IP
    request_counts: HashMap<String, Vec<Instant>>,
    /// Rate limit per minute
    rate_limit: u32,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(rate_limit: u32) -> Self {
        Self {
            request_counts: HashMap::new(),
            rate_limit,
        }
    }

    /// Check if request is allowed
    pub fn is_allowed(&mut self, client_id: &str) -> bool {
        let now = Instant::now();
        let minute_ago = now - Duration::from_secs(60);
        
        // Get or create request history for this client
        let requests = self.request_counts.entry(client_id.to_string()).or_insert_with(Vec::new);
        
        // Remove old requests
        requests.retain(|&time| time > minute_ago);
        
        // Check if under rate limit
        if requests.len() < self.rate_limit as usize {
            requests.push(now);
            true
        } else {
            false
        }
    }

    /// Get current request count
    pub fn get_request_count(&self, client_id: &str) -> usize {
        self.request_counts.get(client_id).map(|v| v.len()).unwrap_or(0)
    }
}

/// Input validator
pub struct InputValidator {
    /// Max input size
    max_input_size: usize,
    /// Allowed patterns
    allowed_patterns: Vec<String>,
    /// Blocked patterns
    blocked_patterns: Vec<String>,
}

impl InputValidator {
    /// Create a new input validator
    pub fn new(max_size: usize) -> Self {
        Self {
            max_input_size: max_size,
            allowed_patterns: vec![
                r"^0x[0-9a-fA-F]+$".to_string(), // Hex strings
                r"^[0-9]+$".to_string(), // Numbers
                r"^[a-zA-Z0-9_]+$".to_string(), // Alphanumeric
            ],
            blocked_patterns: vec![
                r"<script".to_string(), // XSS
                r"javascript:".to_string(), // XSS
                r"eval\(".to_string(), // Code injection
                r"exec\(".to_string(), // Code injection
            ],
        }
    }

    /// Validate input
    pub fn validate(&self, input: &str) -> DeFiResult<()> {
        // Check size
        if input.len() > self.max_input_size {
            return Err(DeFiAnalyzerError::ConfigurationError {
                reason: format!("Input size {} exceeds maximum {}", input.len(), self.max_input_size),
            });
        }

        // Check blocked patterns
        for pattern in &self.blocked_patterns {
            if input.contains(pattern) {
                return Err(DeFiAnalyzerError::ConfigurationError {
                    reason: format!("Input contains blocked pattern: {}", pattern),
                });
            }
        }

        // Check allowed patterns (at least one must match)
        let mut matches_allowed = false;
        for pattern in &self.allowed_patterns {
            if regex::Regex::new(pattern).unwrap().is_match(input) {
                matches_allowed = true;
                break;
            }
        }

        if !matches_allowed {
            return Err(DeFiAnalyzerError::ConfigurationError {
                reason: "Input does not match any allowed pattern".to_string(),
            });
        }

        Ok(())
    }
}

/// Key manager
pub struct KeyManager {
    /// Current keys
    keys: HashMap<String, KeyInfo>,
    /// Key rotation interval
    rotation_interval: Duration,
    /// Last rotation time
    last_rotation: Instant,
}

/// Key information
#[derive(Debug, Clone)]
pub struct KeyInfo {
    pub key_id: String,
    pub key_type: KeyType,
    pub created_at: Instant,
    pub expires_at: Instant,
    pub permissions: Vec<String>,
}

/// Key types
#[derive(Debug, Clone)]
pub enum KeyType {
    /// API key
    ApiKey,
    /// Encryption key
    EncryptionKey,
    /// Signing key
    SigningKey,
}

impl KeyManager {
    /// Create a new key manager
    pub fn new(rotation_interval: Duration) -> Self {
        Self {
            keys: HashMap::new(),
            rotation_interval,
            last_rotation: Instant::now(),
        }
    }

    /// Generate a new key
    pub fn generate_key(&mut self, key_type: KeyType, permissions: Vec<String>) -> String {
        let key_id = format!("key_{}", uuid::Uuid::new_v4());
        let now = Instant::now();
        
        let key_info = KeyInfo {
            key_id: key_id.clone(),
            key_type,
            created_at: now,
            expires_at: now + Duration::from_secs(86400), // 24 hours
            permissions,
        };
        
        self.keys.insert(key_id.clone(), key_info);
        key_id
    }

    /// Validate key
    pub fn validate_key(&self, key_id: &str, required_permission: &str) -> bool {
        if let Some(key_info) = self.keys.get(key_id) {
            // Check if key is not expired
            if key_info.expires_at > Instant::now() {
                // Check permissions
                key_info.permissions.contains(&required_permission.to_string())
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Rotate keys if needed
    pub fn rotate_keys_if_needed(&mut self) {
        if self.last_rotation.elapsed() > self.rotation_interval {
            self.rotate_keys();
            self.last_rotation = Instant::now();
        }
    }

    /// Rotate all keys
    fn rotate_keys(&mut self) {
        info!("Rotating keys");
        self.keys.clear();
    }
}

/// Security audit
pub struct SecurityAudit {
    /// Audit logs
    audit_logs: Vec<AuditLog>,
    /// Security events
    security_events: Vec<SecurityEvent>,
    /// Configuration
    config: AuditConfig,
}

/// Audit configuration
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Enable audit logging
    pub enable_audit_logging: bool,
    /// Log retention days
    pub log_retention_days: u32,
    /// Alert on security events
    pub alert_on_security_events: bool,
}

/// Audit log entry
#[derive(Debug, Clone)]
pub struct AuditLog {
    pub timestamp: Instant,
    pub event_type: AuditEventType,
    pub user_id: String,
    pub action: String,
    pub result: AuditResult,
    pub details: String,
}

/// Audit event types
#[derive(Debug, Clone)]
pub enum AuditEventType {
    /// Authentication
    Authentication,
    /// Authorization
    Authorization,
    /// Data access
    DataAccess,
    /// Configuration change
    ConfigurationChange,
    /// Security event
    SecurityEvent,
}

/// Audit results
#[derive(Debug, Clone)]
pub enum AuditResult {
    /// Success
    Success,
    /// Failure
    Failure,
    /// Warning
    Warning,
}

/// Security event
#[derive(Debug, Clone)]
pub struct SecurityEvent {
    pub event_id: String,
    pub severity: SecuritySeverity,
    pub event_type: SecurityEventType,
    pub description: String,
    pub timestamp: Instant,
    pub source: String,
}

/// Security severity levels
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum SecuritySeverity {
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical severity
    Critical,
}

/// Security event types
#[derive(Debug, Clone)]
pub enum SecurityEventType {
    /// Unauthorized access
    UnauthorizedAccess,
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Invalid input
    InvalidInput,
    /// Key compromise
    KeyCompromise,
    /// System intrusion
    SystemIntrusion,
}

impl SecurityManager {
    /// Create a new security manager
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            rate_limiter: RateLimiter::new(config.rate_limit_per_minute),
            input_validator: InputValidator::new(config.max_input_size),
            key_manager: KeyManager::new(config.key_rotation_interval),
            security_audit: SecurityAudit {
                audit_logs: Vec::new(),
                security_events: Vec::new(),
                config: AuditConfig {
                    enable_audit_logging: true,
                    log_retention_days: 30,
                    alert_on_security_events: true,
                },
            },
            config,
        }
    }

    /// Validate request
    pub fn validate_request(&mut self, client_id: &str, input: &str) -> DeFiResult<()> {
        // Rate limiting
        if self.config.enable_rate_limiting {
            if !self.rate_limiter.is_allowed(client_id) {
                self.log_security_event(
                    SecurityEventType::RateLimitExceeded,
                    SecuritySeverity::Medium,
                    format!("Rate limit exceeded for client: {}", client_id),
                    client_id.to_string(),
                );
                return Err(DeFiAnalyzerError::ConfigurationError {
                    reason: "Rate limit exceeded".to_string(),
                });
            }
        }

        // Input validation
        if self.config.enable_input_validation {
            if let Err(e) = self.input_validator.validate(input) {
                self.log_security_event(
                    SecurityEventType::InvalidInput,
                    SecuritySeverity::High,
                    format!("Invalid input detected: {}", e),
                    client_id.to_string(),
                );
                return Err(e);
            }
        }

        // Key management
        if self.config.enable_key_management {
            self.key_manager.rotate_keys_if_needed();
        }

        Ok(())
    }

    /// Log security event
    pub fn log_security_event(&mut self, event_type: SecurityEventType, severity: SecuritySeverity, description: String, source: String) {
        let event = SecurityEvent {
            event_id: format!("sec_{}", uuid::Uuid::new_v4()),
            severity: severity.clone(),
            event_type: event_type.clone(),
            description: description.clone(),
            timestamp: Instant::now(),
            source,
        };

        self.security_audit.security_events.push(event.clone());

        // Alert on high severity events
        if self.security_audit.config.alert_on_security_events && severity >= SecuritySeverity::High {
            warn!("🚨 Security Alert: {:?} - {}", event_type, description);
        }
    }

    /// Get security statistics
    pub fn get_security_stats(&self) -> SecurityStats {
        let total_events = self.security_audit.security_events.len();
        let high_severity_events = self.security_audit.security_events
            .iter()
            .filter(|e| e.severity >= SecuritySeverity::High)
            .count();

        SecurityStats {
            total_events,
            high_severity_events,
            rate_limit_violations: self.count_events_by_type(SecurityEventType::RateLimitExceeded),
            invalid_input_events: self.count_events_by_type(SecurityEventType::InvalidInput),
            unauthorized_access_events: self.count_events_by_type(SecurityEventType::UnauthorizedAccess),
        }
    }

    /// Count events by type
    fn count_events_by_type(&self, event_type: SecurityEventType) -> usize {
        self.security_audit.security_events
            .iter()
            .filter(|e| std::mem::discriminant(&e.event_type) == std::mem::discriminant(&event_type))
            .count()
    }

    /// Generate security report
    pub fn generate_security_report(&self) -> String {
        let stats = self.get_security_stats();
        
        format!(
            "🔒 Security Report\n\
            ├─ Total Events: {}\n\
            ├─ High Severity: {}\n\
            ├─ Rate Limit Violations: {}\n\
            ├─ Invalid Input Events: {}\n\
            └─ Unauthorized Access: {}",
            stats.total_events,
            stats.high_severity_events,
            stats.rate_limit_violations,
            stats.invalid_input_events,
            stats.unauthorized_access_events
        )
    }
}

/// Security statistics
#[derive(Debug)]
pub struct SecurityStats {
    pub total_events: usize,
    pub high_severity_events: usize,
    pub rate_limit_violations: usize,
    pub invalid_input_events: usize,
    pub unauthorized_access_events: usize,
}
