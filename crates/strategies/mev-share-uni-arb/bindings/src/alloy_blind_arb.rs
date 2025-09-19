#[allow(clippy::all)]
pub use generated::*;

#[allow(clippy::all)]
mod generated {
    use alloy_sol_types::sol;

    sol! {
        #[allow(missing_docs)]
        #[sol(rpc)]
        contract BlindArb {
            function executeArb__WETH_token0(
                address v2Pair,
                address v3Pair,
                uint256 amountIn,
                uint256 percentageToPayToCoinbase
            ) external;

            function executeArb__WETH_token1(
                address v2Pair,
                address v3Pair,
                uint256 amountIn,
                uint256 percentageToPayToCoinbase
            ) external;
        }
    }
}
