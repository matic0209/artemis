//! Sandwich strategy placeholder build.

#[cfg(feature = "full")]
pub mod types;
#[cfg(feature = "full")]
pub mod strategy;
#[cfg(feature = "full")]
pub mod collectors;
#[cfg(feature = "full")]
pub mod executors;
#[cfg(feature = "full")]
pub mod contracts;
#[cfg(feature = "full")]
pub mod simulator;
#[cfg(feature = "full")]
pub mod utils;
#[cfg(feature = "full")]
pub mod pools;
#[cfg(feature = "full")]
pub mod state_queries;
#[cfg(feature = "full")]
pub mod revm_engine;
#[cfg(feature = "full")]
pub mod transaction_executor;

#[cfg(feature = "full")]
pub use strategy::SandwichStrategy;
#[cfg(feature = "full")]
pub use types::{Event, Action, SandwichConfig, SandwichBundle, SandwichOpportunity};
#[cfg(feature = "full")]
pub use collectors::sandwich_mempool_collector::SandwichMempoolCollector;
#[cfg(feature = "full")]
pub use executors::sandwich_executor::SandwichExecutor;
#[cfg(feature = "full")]
pub use state_queries::{StateQuerier, PoolStateMonitor};
#[cfg(feature = "full")]
pub use revm_engine::{RevmEngine, RevmConfig};
#[cfg(feature = "full")]
pub use transaction_executor::{TransactionExecutor, SandwichSimulationResult};

#[cfg(not(feature = "full"))]
mod stub;
#[cfg(not(feature = "full"))]
pub use stub::*;
