//! SDK adapter layer.
//!
//! This module provides an indirection layer so the rest of Artemis can avoid
//! depending directly on a particular Ethereum SDK. We currently ship with an
//! `ethers-rs` implementation and keep an Alloy implementation scaffolded
//! behind the `sdk-alloy` feature so we can migrate incrementally.

#[cfg(feature = "sdk-ethers")]
mod ethers;
#[cfg(feature = "sdk-ethers")]
pub use self::ethers::*;

#[cfg(feature = "sdk-alloy")]
pub mod alloy_support;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub use alloy_support::{
    helpers, Address, BlockId, BlockNumberOrTag, Bytes, Filter, Hash, LocalWallet, Log, Provider,
    Transaction, TxRequest, U256, U64,
};
