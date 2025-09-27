#![warn(unused_crate_dependencies)]
#![deny(unused_must_use, rust_2018_idioms)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]

//! Artemis: High-Performance MEV Bot Framework
//!
//! Artemis is a sophisticated, modular framework for building MEV (Maximal Extractable Value) bots
//! with enterprise-grade performance, reliability, and observability features.
//!
//! ## Core Architecture
//!
//! Artemis follows an advanced event-driven architecture with the following components:
//!
//! 1. **[Collectors](types::Collector)**: High-performance event ingestion from multiple sources
//!    - Real-time mempool monitoring with priority queuing
//!    - Multi-chain block event processing
//!    - DEX order book and AMM state tracking
//!
//! 2. **[Strategies](types::Strategy)**: Sophisticated opportunity detection and analysis
//!    - Advanced arbitrage detection with symbolic execution
//!    - MEV opportunity classification and risk assessment
//!    - Dynamic strategy adaptation based on market conditions
//!
//! 3. **[Executors](types::Executor)**: Efficient and reliable action execution
//!    - Multi-executor coordination with dependency resolution
//!    - Gas optimization and transaction batching
//!    - Flashbots integration and private mempool routing
//!
//! 4. **[High-Performance Engine](engine::HighPerformanceEngine)**: Advanced orchestration system
//!    - Priority-based event processing with backpressure handling
//!    - Circuit breaker protection and adaptive load balancing
//!    - Comprehensive metrics collection and alerting
//!
//! ## Performance Features
//!
//! - **Zero-copy processing** for minimal latency
//! - **Adaptive batching** to optimize throughput
//! - **Intelligent caching** with predictive preloading
//! - **Multi-threaded execution** with work-stealing queues
//! - **Resource pooling** for memory and connection optimization
//!
//! ## Enterprise Features
//!
//! - **Circuit breaker protection** for fault tolerance
//! - **Comprehensive monitoring** with Prometheus metrics
//! - **Hot-swappable strategies** for zero-downtime updates
//! - **Distributed execution** with coordination protocols
//! - **Advanced analytics** with time-series data storage
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use artemis_core::{
//!     engine::HighPerformanceEngine,
//!     types::{Event, Action},
//!     strategy_composer::StrategyComposer,
//! };
//!
//! // Create a high-performance engine with custom configuration
//! let engine = HighPerformanceEngine::new()
//!     .with_max_concurrent_strategies(10)
//!     .with_circuit_breaker_enabled(true)
//!     .build();
//!
//! // Compose multiple strategies for parallel execution
//! let composer = StrategyComposer::parallel()
//!     .add_strategy(arbitrage_strategy)
//!     .add_strategy(liquidation_strategy)
//!     .with_performance_target(99.0) // 99th percentile latency
//!     .build();
//! ```
//!
//! ## Performance Optimization
//!
//! Artemis provides several optimization modules for high-frequency trading:
//!
//! - **[Memory Optimization](memory_optimization)**: Custom allocators and zero-copy processing
//! - **[Concurrency Optimization](concurrency_optimization)**: Work-stealing schedulers and lock-free data structures
//! - **[SIMD Math](simd_math)**: Vectorized mathematical operations for price calculations
//! - **[Batch Processing](batch_processor)**: Intelligent batching for improved throughput

/// This module contains [collector](types::Collector) implementations.
pub mod collectors;
/// This module contains the [Engine](engine::Engine) struct, which is responsible
/// for orchestrating data flows between components
#[path = "engine/mod.rs"]
pub mod engine;
/// This module provides Alloy-based Ethereum SDK adapter layer.
pub mod eth;
/// This module contains [executor](types::Executor) implementations.
pub mod executors;
/// This module contains the core type definitions for Artemis.
pub mod types;
/// This module contains utilities for working with Artemis.
pub mod utilities;
/// Unified error handling system for Artemis.
pub mod error;
/// Intelligent state management with predictive caching.
pub mod state_manager;
/// High-performance connection pool for Ethereum providers.
pub mod connection_pool;
/// Adaptive performance tuner for automatic optimization.
pub mod adaptive_tuner;
/// Performance benchmark suite for optimization validation.
pub mod benchmarks;
/// Memory pool management for reducing allocation overhead.
pub mod memory_pool;
/// Batch processing for improved throughput.
pub mod batch_processor;
/// SIMD optimized mathematical calculations.
pub mod simd_math;
/// Zero-copy serialization for high-performance data processing.
pub mod zero_copy;
/// Comprehensive monitoring and alerting system.
pub mod monitoring;
/// Advanced strategy composition and orchestration system.
pub mod strategy_composer;
/// Memory optimization and zero-copy processing system.
pub mod memory_optimization;
/// Concurrency optimization with work-stealing schedulers and lock-free data structures.
pub mod concurrency_optimization;

// Suppress unused crate warnings
use alloy as _;
use alloy_mev as _;
use alloy_rpc_types as _;
use alloy_transport_http as _;
use alloy_transport_ws as _;
use serde_json as _;
use reqwest as _;
use jsonrpsee as _;
use thiserror as _;
use tower as _;
use alloy_network as _;
use alloy_signer as _;
use bytecheck as _;
use chrono as _;
use csv as _;
use hex as _;
use rayon as _;
