//! Basic integration tests for MEV arbitrage

use mev_arbitrage::utils::amm::*;
use alloy_primitives::U256;

#[test]
fn test_utils_module_exists() {
    // Test that utils module compiles and is accessible
    let output = uniswap_v2_output(
        U256::from(1000),
        U256::from(10000),
        U256::from(10000)
    );
    assert!(output > U256::ZERO);
}

#[test]
fn test_basic_compilation() {
    // This test ensures the crate compiles correctly
    assert!(true);
}
