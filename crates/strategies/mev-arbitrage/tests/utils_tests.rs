use mev_arbitrage::utils::amm::*;
use alloy_primitives::U256;

#[test]
fn test_uniswap_v2_output_basic() {
    let amount_in = U256::from(1000);
    let reserve_in = U256::from(10000);
    let reserve_out = U256::from(10000);

    let output = uniswap_v2_output(amount_in, reserve_in, reserve_out);

    // Output should be less than input due to fees
    assert!(output > U256::ZERO);
    assert!(output < amount_in);
}

#[test]
fn test_uniswap_v2_output_zero_input() {
    let output = uniswap_v2_output(
        U256::ZERO,
        U256::from(10000),
        U256::from(10000)
    );

    assert_eq!(output, U256::ZERO);
}

#[test]
fn test_uniswap_v2_output_zero_reserve() {
    let output = uniswap_v2_output(
        U256::from(1000),
        U256::ZERO,
        U256::from(10000)
    );

    assert_eq!(output, U256::ZERO);
}

#[test]
fn test_uniswap_v2_input_calculation() {
    let amount_out = U256::from(900);
    let reserve_in = U256::from(10000);
    let reserve_out = U256::from(10000);

    let input = uniswap_v2_input(amount_out, reserve_in, reserve_out);

    // Input should be more than output due to fees
    assert!(input > amount_out);
}

#[test]
fn test_curve_stable_swap() {
    let amount_in = U256::from(1000);
    let balance_in = U256::from(100000);
    let balance_out = U256::from(100000);

    let output = curve_stable_swap_output(amount_in, balance_in, balance_out);

    // For stable swaps with balanced pools, output should be close to input
    assert!(output > U256::ZERO);
}

#[test]
fn test_uniswap_v3_output() {
    let amount_in = U256::from(1000);
    let sqrt_price = U256::from(1000000); // Simplified
    let liquidity = U256::from(100000);
    let fee = 3000; // 0.3%

    let output = uniswap_v3_output_single_tick(
        amount_in,
        sqrt_price,
        liquidity,
        fee
    );

    assert!(output > U256::ZERO);
}
