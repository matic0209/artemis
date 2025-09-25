use crate::eth::{TxRequest, U256};

/// Information about the gas bid for a transaction.
#[derive(Debug, Clone)]
pub struct GasBidInfo {
    /// Total profit expected from opportunity.
    pub total_profit: U256,
    /// Percentage of bid profit to use for gas.
    pub bid_percentage: u64,
}

#[derive(Debug, Clone)]
pub struct SubmitTxToMempool {
    pub tx: TxRequest,
    pub gas_bid_info: Option<GasBidInfo>,
}
