use artemis_core::eth::{Address, Hash as TxHash};
#[cfg(not(feature = "sdk-ethers"))]
use once_cell::sync::Lazy;

#[cfg(feature = "sdk-ethers")]
use ethers::contract::EthEvent;
#[cfg(feature = "sdk-ethers")]
use ethers::prelude::Lazy as EthersLazy;
#[cfg(feature = "sdk-ethers")]
type LazyAddress = EthersLazy<Address>;
#[cfg(feature = "sdk-ethers")]
type LazyHashes = EthersLazy<Vec<TxHash>>;

#[cfg(not(feature = "sdk-ethers"))]
type LazyAddress = Lazy<Address>;
#[cfg(not(feature = "sdk-ethers"))]
type LazyHashes = Lazy<Vec<TxHash>>;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use alloy_primitives::keccak256;

/// Block number at which the sudo factory was deployed.
pub const FACTORY_DEPLOYMENT_BLOCK: u64 = 14650730;

/// Address of the sudo pair factory.
pub static LSSVM_PAIR_FACTORY_ADDRESS: LazyAddress = LazyAddress::new(|| {
    "0xb16c1342e617a5b6e4b631eb114483fdb289c0a4"
        .parse()
        .unwrap()
});

/// Group of event signatures which are emitted when a pool is touched.
#[cfg(feature = "sdk-ethers")]
pub static POOL_EVENT_SIGNATURES: LazyHashes = LazyHashes::new(|| {
    vec![
        bindings::lssvm_pair::SwapNFTInPairFilter::signature(),
        bindings::lssvm_pair::SwapNFTInPairFilter::signature(),
        bindings::lssvm_pair::SpotPriceUpdateFilter::signature(),
        bindings::lssvm_pair::TokenWithdrawalFilter::signature(),
    ]
});

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub static POOL_EVENT_SIGNATURES: LazyHashes = LazyHashes::new(|| {
    [
        "SwapNFTInPair()",
        "SwapNFTInPair()",
        "SpotPriceUpdate(uint128)",
        "TokenWithdrawal(uint256)",
    ]
    .iter()
    .map(|sig| {
        let hash = keccak256(sig.as_bytes());
        TxHash::from_slice(hash.as_slice())
    })
    .collect()
});
