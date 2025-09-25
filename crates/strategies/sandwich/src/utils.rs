use artemis_core::eth::{Address, U256, Hash};
use anyhow::Result;

/// 工具函数集合

/// 解析交易 calldata 获取代币地址
pub fn extract_tokens_from_calldata(calldata: &[u8]) -> Vec<Address> {
    if calldata.len() < 4 {
        return vec![];
    }

    let selector = &calldata[0..4];
    let params = &calldata[4..];

    match selector {
        // swapExactTokensForTokens(uint,uint,address[],address,uint)
        [0x38, 0xed, 0x17, 0x39] => {
            extract_path_from_swap_params(params)
        }
        // swapExactETHForTokens(uint,address[],address,uint)
        [0x7f, 0xf3, 0x6a, 0xb5] => {
            extract_path_from_eth_swap_params(params)
        }
        // 其他 swap 函数...
        _ => vec![],
    }
}

/// 从 swap 参数中提取代币路径
fn extract_path_from_swap_params(params: &[u8]) -> Vec<Address> {
    // 实现基础的 ABI 解码
    if params.len() < 32 * 3 { // 至少需要 amountIn, amountOutMin, path offset
        return vec![];
    }
    
    // 跳过 amountIn (32 bytes) 和 amountOutMin (32 bytes)
    // path 数组的偏移量在第 3 个位置
    let path_offset = u32::from_be_bytes([
        params[64], params[65], params[66], params[67]
    ]) as usize;
    
    if path_offset + 32 > params.len() {
        return vec![];
    }
    
    // 读取数组长度
    let array_length = u32::from_be_bytes([
        params[path_offset], 
        params[path_offset + 1], 
        params[path_offset + 2], 
        params[path_offset + 3]
    ]) as usize;
    
    // 提取地址
    let mut addresses = Vec::new();
    for i in 0..array_length {
        let addr_offset = path_offset + 32 + (i * 32) + 12; // 地址在 32 字节的后 20 字节
        if addr_offset + 20 <= params.len() {
            let addr_bytes: [u8; 20] = params[addr_offset..addr_offset + 20].try_into().unwrap();
            addresses.push(Address::from(addr_bytes));
        }
    }
    
    addresses
}

/// 从 ETH swap 参数中提取代币路径
fn extract_path_from_eth_swap_params(params: &[u8]) -> Vec<Address> {
    // ETH swap 的参数结构略有不同，但解码逻辑类似
    if params.len() < 32 * 2 { // amountOutMin, path offset
        return vec![];
    }
    
    // path 数组的偏移量在第 2 个位置
    let path_offset = u32::from_be_bytes([
        params[32], params[33], params[34], params[35]
    ]) as usize;
    
    if path_offset + 32 > params.len() {
        return vec![];
    }
    
    // 读取数组长度
    let array_length = u32::from_be_bytes([
        params[path_offset], 
        params[path_offset + 1], 
        params[path_offset + 2], 
        params[path_offset + 3]
    ]) as usize;
    
    // 提取地址
    let mut addresses = Vec::new();
    for i in 0..array_length {
        let addr_offset = path_offset + 32 + (i * 32) + 12;
        if addr_offset + 20 <= params.len() {
            let addr_bytes: [u8; 20] = params[addr_offset..addr_offset + 20].try_into().unwrap();
            addresses.push(Address::from(addr_bytes));
        }
    }
    
    addresses
}

/// 计算 Uniswap V2 的输出金额
pub fn calculate_v2_amount_out(
    amount_in: U256,
    reserve_in: U256,
    reserve_out: U256,
    fee: u32, // 通常是 3000 (0.3%)
) -> U256 {
    if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return U256::ZERO;
    }

    let amount_in_with_fee = amount_in * U256::from(10000 - fee);
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * U256::from(10000) + amount_in_with_fee;

    numerator / denominator
}

/// 计算 Uniswap V2 的输入金额
pub fn calculate_v2_amount_in(
    amount_out: U256,
    reserve_in: U256,
    reserve_out: U256,
    fee: u32,
) -> U256 {
    if amount_out.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return U256::ZERO;
    }

    if amount_out >= reserve_out {
        return U256::MAX; // 不可能的交易
    }

    let numerator = reserve_in * amount_out * U256::from(10000);
    let denominator = (reserve_out - amount_out) * U256::from(10000 - fee);

    numerator / denominator + U256::from(1) // 向上取整
}

/// 计算最优的 sandwich 输入金额
pub fn calculate_optimal_sandwich_input(
    victim_amount_in: U256,
    reserve_in: U256,
    reserve_out: U256,
    fee: u32,
) -> U256 {
    // 使用数学优化公式计算最优输入
    // 这是一个简化版本，实际实现需要更复杂的数学计算
    
    let victim_impact = victim_amount_in * U256::from(1000) / reserve_in;
    
    // 基于受害者交易的影响调整我们的输入
    match victim_impact.to::<u64>() {
        0..=10 => victim_amount_in * U256::from(2),      // 小影响：2x
        11..=50 => victim_amount_in * U256::from(3),     // 中等影响：3x
        51..=100 => victim_amount_in * U256::from(4),    // 大影响：4x
        _ => victim_amount_in * U256::from(5),           // 巨大影响：5x
    }
}

/// 检查地址是否是合约
pub async fn is_contract(provider: &artemis_core::eth::Provider, address: Address) -> bool {
    use alloy_provider::Provider as ProviderTrait;
    
    match provider.get_code_at(address).await {
        Ok(code) => !code.is_empty(),
        Err(_) => false,
    }
}

/// 格式化 ETH 金额用于显示
pub fn format_eth_amount(wei: U256) -> String {
    let eth = wei.to::<u128>() as f64 / 1e18;
    format!("{:.6} ETH", eth)
}

/// 格式化 gas 金额用于显示
pub fn format_gas_price(wei: U256) -> String {
    let gwei = wei.to::<u128>() as f64 / 1e9;
    format!("{:.2} gwei", gwei)
}

/// 计算交易哈希（用于去重）
pub fn calculate_tx_hash(
    from: Address,
    to: Option<Address>,
    value: U256,
    data: &[u8],
    nonce: u64,
) -> Hash {
    use alloy_primitives::keccak256;
    
    let mut hasher_input = Vec::new();
    hasher_input.extend_from_slice(from.as_slice());
    
    if let Some(to_addr) = to {
        hasher_input.extend_from_slice(to_addr.as_slice());
    }
    
    hasher_input.extend_from_slice(&value.to_be_bytes::<32>());
    hasher_input.extend_from_slice(data);
    hasher_input.extend_from_slice(&nonce.to_be_bytes());
    
    keccak256(&hasher_input)
}

/// 检查两个地址是否形成有效的代币对
pub fn is_valid_token_pair(token_a: Address, token_b: Address, weth_address: Address) -> bool {
    // 至少有一个是 WETH
    (token_a == weth_address || token_b == weth_address) &&
    // 两个代币不能相同
    token_a != token_b &&
    // 都不能是零地址
    !token_a.is_zero() && !token_b.is_zero()
}

/// 性能优化的地址比较
#[inline(always)]
pub fn addresses_equal(a: &Address, b: &Address) -> bool {
    // 使用字节比较，比 PartialEq 更快
    a.as_slice() == b.as_slice()
}

/// 快速检查交易是否可能盈利
pub fn quick_profitability_check(
    victim_value: U256,
    min_profit_threshold: U256,
    estimated_gas_cost: U256,
) -> bool {
    // 快速启发式检查
    let potential_profit = victim_value / U256::from(100); // 假设能捕获 1% 的价值
    
    potential_profit > min_profit_threshold + estimated_gas_cost
}

/// 批量检查多个机会的盈利性
pub fn batch_profitability_check(
    opportunities: &[(U256, U256)], // (victim_value, estimated_profit)
    min_threshold: U256,
) -> Vec<bool> {
    opportunities
        .iter()
        .map(|(_, profit)| *profit >= min_threshold)
        .collect()
}

/// 计算给矿工的最优贿赂
pub fn calculate_optimal_bribe(
    profit: U256,
    base_fee: U256,
    gas_used: u64,
    max_bribe_percentage: u8, // 最大贿赂比例（如 50%）
) -> U256 {
    let max_bribe = profit * U256::from(max_bribe_percentage) / U256::from(100);
    let min_tip = U256::from(1_000_000_000u64); // 1 gwei 最小小费
    let calculated_tip = max_bribe / U256::from(gas_used);
    
    calculated_tip.max(min_tip).min(base_fee * U256::from(10)) // 不超过 base_fee 的 10 倍
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_amount_calculation() {
        let amount_in = U256::from(1_000_000_000_000_000_000u64); // 1 ETH
        let reserve_in = U256::from(100_000_000_000_000_000_000u64); // 100 ETH
        let reserve_out = U256::from(200_000_000_000_000_000_000u64); // 200 ETH
        let fee = 30; // 0.3%

        let amount_out = calculate_v2_amount_out(amount_in, reserve_in, reserve_out, fee);
        
        // 应该得到大约 1.97 ETH（考虑 0.3% 费用）
        assert!(amount_out > U256::from(1_900_000_000_000_000_000u64));
        assert!(amount_out < U256::from(2_000_000_000_000_000_000u64));
    }

    #[test]
    fn test_optimal_input_calculation() {
        let victim_amount = U256::from(1_000_000_000_000_000_000u64); // 1 ETH
        let reserve_in = U256::from(100_000_000_000_000_000_000u64); // 100 ETH
        let reserve_out = U256::from(200_000_000_000_000_000_000u64); // 200 ETH
        let fee = 30;

        let optimal = calculate_optimal_sandwich_input(victim_amount, reserve_in, reserve_out, fee);
        
        // 最优输入应该是受害者金额的 2-5 倍
        assert!(optimal >= victim_amount * U256::from(2));
        assert!(optimal <= victim_amount * U256::from(5));
    }

    #[test]
    fn test_profitability_check() {
        let victim_value = U256::from(10_000_000_000_000_000_000u64); // 10 ETH
        let min_threshold = U256::from(1_000_000_000_000_000u64); // 0.001 ETH
        let gas_cost = U256::from(5_000_000_000_000_000u64); // 0.005 ETH

        assert!(quick_profitability_check(victim_value, min_threshold, gas_cost));
    }
}
