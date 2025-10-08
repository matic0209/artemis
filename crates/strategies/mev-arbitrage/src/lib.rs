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

// Organized modules (by functionality)
#[cfg(feature = "full")]
pub mod detectors;
#[cfg(feature = "full")]
pub mod validators;
#[cfg(feature = "full")]
pub mod optimizers;
#[cfg(feature = "full")]
pub mod strategies;
#[cfg(feature = "full")]
pub mod execution;
#[cfg(feature = "full")]
pub mod coordination;

// Multi-strategy MEV modules (0x0e49-inspired)
#[cfg(feature = "full")]
pub mod simulators;
#[cfg(feature = "full")]
pub mod composers;

// Artemis Strategy integration
#[cfg(feature = "full")]
pub mod strategy;

// Common utilities (always available)
pub mod utils;

// Legacy modules have been archived to /archived/legacy/
// They are no longer part of the active codebase

// Entry point modules
#[cfg(feature = "full")]
mod full;
#[cfg(feature = "full")]
pub use full::*;

#[cfg(not(feature = "full"))]
mod stub;
#[cfg(not(feature = "full"))]
pub use stub::*;