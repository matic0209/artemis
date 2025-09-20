//! Executors are responsible for taking actions produced by strategies and
//! executing them in different domains. For example, an executor might take a
//! `SubmitTx` action and submit it to the mempool.

/// This module contains shared types for executors.
pub mod mempool_types;

/// Alloy-based mempool executor.
pub mod mempool_alloy_executor;

/// Alloy-based Flashbots executor (using alloy-mev).
pub mod flashbots_alloy_executor;

/// Alloy-based MEV-share executor (using alloy-mev).
pub mod mev_share_alloy_executor;