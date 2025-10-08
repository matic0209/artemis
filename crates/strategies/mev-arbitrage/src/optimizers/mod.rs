//! Optimizers Module
//!
//! This module contains optimization implementations:
//! - Z3 optimizer for constraint-based optimization

pub mod z3;

// Re-export main optimizer types
pub use z3::{Z3ArbitrageOptimizer, Z3OptimizerConfig, Z3OptimizerStats};
