//! Coordination Module
//!
//! This module contains coordination and orchestration implementations:
//! - Event system for async event handling
//! - Arbitrage coordinator for managing arbitrage lifecycle
//! - Unified manager for high-level strategy coordination

pub mod event_system;
pub mod arbitrage_coordinator;
pub mod unified_manager;

// Re-export main coordination types
pub use event_system::{MEVEvent, EventRouter};
pub use unified_manager::UnifiedArbitrageManager;
