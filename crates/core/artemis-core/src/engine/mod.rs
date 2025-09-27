//! High-Performance Event-Driven Engine
//!
//! This module contains the core engine implementation with enhanced performance,
//! advanced concurrency, and sophisticated event processing capabilities.
//!
//! ## Architecture Overview
//!
//! The engine module provides a sophisticated event-driven architecture designed for
//! high-frequency MEV operations with the following key components:
//!
//! ### Components
//!
//! - **[HighPerformanceEngine]**: Main orchestration engine with circuit breaker protection
//! - **[EventBus]**: Priority-based event processing with backpressure handling
//! - **[ExecutionCoordinator]**: Sophisticated execution planning with dependency resolution
//! - **[MetricsCollector]**: Comprehensive metrics collection and monitoring
//!
//! ## Usage Examples
//!
//! ### Basic Engine Setup
//!
//! ```rust,no_run
//! use artemis_core::engine::{HighPerformanceEngine, EventBus};
//! use artemis_core::types::{Event, Action};
//!
//! let mut engine = HighPerformanceEngine::new()
//!     .with_max_concurrent_strategies(10)
//!     .with_circuit_breaker_enabled(true)
//!     .with_adaptive_load_balancing(true)
//!     .build();
//!
//! // Start the engine
//! engine.start().await?;
//! ```
//!
//! ### Event Processing with Priority
//!
//! ```rust,no_run
//! use artemis_core::engine::{EventBus, EventPriority};
//!
//! let mut event_bus = EventBus::new(1000); // Buffer size: 1000
//!
//! // Send high-priority arbitrage opportunity
//! event_bus.send_with_priority(
//!     arbitrage_event,
//!     EventPriority::High
//! ).await?;
//! ```
//!
//! ## Performance Characteristics
//!
//! - **Latency**: Sub-millisecond event processing
//! - **Throughput**: 100,000+ events per second
//! - **Memory**: Zero-copy event handling where possible
//! - **Concurrency**: Work-stealing scheduler with adaptive scaling

pub mod high_performance_engine;
pub mod event_bus;
pub mod execution_coordinator;
pub mod metrics_collector;

pub use high_performance_engine::HighPerformanceEngine;
pub use event_bus::{EventBus, EventMessage, EventPriority};
pub use execution_coordinator::{ExecutionCoordinator, ExecutionPlan};
pub use metrics_collector::{MetricsCollector, PerformanceMetrics};

// Legacy engine module
pub mod legacy_engine;

// Re-export the legacy engine for compatibility
pub use legacy_engine::Engine;