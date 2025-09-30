//! MEV Arbitrage Strategy
//!
//! Provides MEV arbitrage detection and execution capabilities.
//! Supports two modes:
//! - `lite`: Minimal stub implementation (default)
//! - `full`: Complete implementation with all sub-packages

// Declare modules that are used in full mode
#[cfg(feature = "full")]
pub mod abstractions;
#[cfg(feature = "full")]
pub mod component_factory;
#[cfg(feature = "full")]
pub mod unified_arbitrage_manager;

// Core systems
#[cfg(feature = "full")]
pub mod event_system;
#[cfg(feature = "full")]
pub mod gas_strategy;
#[cfg(feature = "full")]
pub mod z3_optimizer;

// Active detection modules
#[cfg(feature = "full")]
pub mod enhanced_arbitrage_detector;
#[cfg(feature = "full")]
pub mod fast_arbitrage_detector;

// Legacy modules - preserved for feature migration
// These modules contain valuable functionality that will be gradually migrated to the new architecture
// Enable with: cargo build --features full,legacy
#[cfg(all(feature = "full", feature = "legacy"))]
pub mod legacy;

// Entry point modules
#[cfg(feature = "full")]
mod full;
#[cfg(feature = "full")]
pub use full::*;

#[cfg(not(feature = "full"))]
mod stub;
#[cfg(not(feature = "full"))]
pub use stub::*;