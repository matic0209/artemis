#![warn(unused_crate_dependencies)]
#![deny(unused_must_use, rust_2018_idioms)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]
//! A strategy implementing atomic, cross-market NFT arbitrage between
//! Seaport and Sudoswap. At a high level, we listen to a stream of new seaport orders,
//! and compute whether we can atomically fulfill the order and sell the NFT into a
//! sudoswap pool while making a profit.
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use {futures as _, lru as _, metrics as _, opensea_stream as _, parking_lot as _, tracing as _};

/// This module contains constants used by the strategy.
pub mod constants;

/// This module contains the core strategy implementation.
pub mod strategy;

/// This module contains the core type definitions for the strategy.
pub mod types;
