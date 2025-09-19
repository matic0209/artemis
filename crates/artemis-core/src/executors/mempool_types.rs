use crate::eth::U256;

#[cfg(feature = "sdk-ethers")]
use crate::eth::TxRequest;

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use crate::eth::TxRequest;

/// Information about the gas bid for a transaction.
#[derive(Debug, Clone)]
pub struct GasBidInfo {
    /// Total profit expected from opportunity.
    pub total_profit: U256,
    /// Percentage of bid profit to use for gas.
    pub bid_percentage: u64,
}

#[cfg(feature = "sdk-ethers")]
#[derive(Debug, Clone)]
pub struct SubmitTxToMempool {
    pub tx: TxRequest,
    pub gas_bid_info: Option<GasBidInfo>,
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
#[derive(Debug, Clone)]
pub struct SubmitTxToMempool {
    pub tx: TxRequest,
    pub gas_bid_info: Option<GasBidInfo>,
}
