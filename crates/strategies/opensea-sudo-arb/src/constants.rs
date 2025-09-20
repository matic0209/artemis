use artemis_core::eth::{Address, Hash as TxHash};
use once_cell::sync::Lazy;
use alloy_primitives::keccak256;

type LazyAddress = Lazy<Address>;
type LazyHashes = Lazy<Vec<TxHash>>;

/// Block number at which the sudo factory was deployed.
pub const FACTORY_DEPLOYMENT_BLOCK: u64 = 14650730;

/// Address of the sudo pair factory.
pub static LSSVM_PAIR_FACTORY_ADDRESS: LazyAddress = LazyAddress::new(|| {
    "0xb16c1342e617a5b6e4b631eb114483fdb289c0a4"
        .parse()
        .unwrap()
});

/// Group of event signatures which are emitted when a pool is touched.
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
