//! Executors are responsible for taking actions produced by strategies and
//! executing them in different domains. For example, an executor might take a
//! `SubmitTx` action and submit it to the mempool.

/// This executor submits transactions to the flashbots relay.
#[cfg(feature = "sdk-ethers")]
pub mod flashbots_executor;

/// Alloy-based Flashbots executor (feature-gated)
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub mod flashbots_alloy_executor;

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub mod mempool_alloy_executor;
/// This executor submits transactions to the public mempool.
#[cfg(feature = "sdk-ethers")]
pub mod mempool_executor;
pub mod mempool_types;

/// This executor submits bundles to the flashbots matchmaker.
#[cfg(feature = "sdk-ethers")]
pub mod mev_share_executor;

/// Alloy-based MEV-Share executor (feature-gated)
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub mod mev_share_alloy_executor;
