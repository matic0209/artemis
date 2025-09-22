//! SIMD 优化的数值计算模块

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use crate::eth::U256;

/// SIMD 优化的数学计算器
pub struct SimdCalculator;

impl SimdCalculator {
    /// 使用 SIMD 计算批量利润
    #[cfg(target_arch = "x86_64")]
    pub fn calculate_profits_simd(amounts: &[f64], price_diffs: &[f64]) -> Vec<f64> {
        if amounts.len() != price_diffs.len() {
            panic!("Arrays must have the same length");
        }

        let mut profits = vec![0.0; amounts.len()];
        let chunks = amounts.len() / 4; // AVX2 可以处理 4 个 f64

        unsafe {
            for i in 0..chunks {
                let base_idx = i * 4;
                
                // 加载数据
                let amounts_vec = _mm256_loadu_pd(amounts.as_ptr().add(base_idx));
                let price_diffs_vec = _mm256_loadu_pd(price_diffs.as_ptr().add(base_idx));
                
                // 计算利润 = amount * price_diff * 0.997 (考虑手续费)
                let fee_factor = _mm256_set1_pd(0.997);
                let profit_vec = _mm256_mul_pd(
                    _mm256_mul_pd(amounts_vec, price_diffs_vec),
                    fee_factor
                );
                
                // 存储结果
                _mm256_storeu_pd(profits.as_mut_ptr().add(base_idx), profit_vec);
            }
        }

        // 处理剩余元素
        for i in (chunks * 4)..amounts.len() {
            profits[i] = amounts[i] * price_diffs[i] * 0.997;
        }

        profits
    }

    /// 标量版本的利润计算（用于不支持 SIMD 的平台）
    #[cfg(not(target_arch = "x86_64"))]
    pub fn calculate_profits_simd(amounts: &[f64], price_diffs: &[f64]) -> Vec<f64> {
        Self::calculate_profits_scalar(amounts, price_diffs)
    }

    /// 标量版本的利润计算
    pub fn calculate_profits_scalar(amounts: &[f64], price_diffs: &[f64]) -> Vec<f64> {
        amounts
            .iter()
            .zip(price_diffs.iter())
            .map(|(amount, price_diff)| amount * price_diff * 0.997)
            .collect()
    }

    /// SIMD 优化的价格影响计算
    #[cfg(target_arch = "x86_64")]
    pub fn calculate_price_impacts_simd(
        amounts: &[f64], 
        liquidities: &[f64]
    ) -> Vec<f64> {
        if amounts.len() != liquidities.len() {
            panic!("Arrays must have the same length");
        }

        let mut impacts = vec![0.0; amounts.len()];
        let chunks = amounts.len() / 4;

        unsafe {
            for i in 0..chunks {
                let base_idx = i * 4;
                
                let amounts_vec = _mm256_loadu_pd(amounts.as_ptr().add(base_idx));
                let liquidities_vec = _mm256_loadu_pd(liquidities.as_ptr().add(base_idx));
                
                // 价格影响 = amount / (liquidity + amount)
                let sum_vec = _mm256_add_pd(liquidities_vec, amounts_vec);
                let impact_vec = _mm256_div_pd(amounts_vec, sum_vec);
                
                _mm256_storeu_pd(impacts.as_mut_ptr().add(base_idx), impact_vec);
            }
        }

        // 处理剩余元素
        for i in (chunks * 4)..amounts.len() {
            impacts[i] = amounts[i] / (liquidities[i] + amounts[i]);
        }

        impacts
    }

    /// 标量版本的价格影响计算
    #[cfg(not(target_arch = "x86_64"))]
    pub fn calculate_price_impacts_simd(
        amounts: &[f64], 
        liquidities: &[f64]
    ) -> Vec<f64> {
        Self::calculate_price_impacts_scalar(amounts, liquidities)
    }

    /// 标量版本的价格影响计算
    pub fn calculate_price_impacts_scalar(
        amounts: &[f64], 
        liquidities: &[f64]
    ) -> Vec<f64> {
        amounts
            .iter()
            .zip(liquidities.iter())
            .map(|(amount, liquidity)| amount / (liquidity + amount))
            .collect()
    }

    /// SIMD 优化的滑点计算
    #[cfg(target_arch = "x86_64")]
    pub fn calculate_slippage_simd(
        amounts: &[f64],
        reserves_in: &[f64],
        reserves_out: &[f64],
    ) -> Vec<f64> {
        let mut slippages = vec![0.0; amounts.len()];
        let chunks = amounts.len() / 4;

        unsafe {
            for i in 0..chunks {
                let base_idx = i * 4;
                
                let amounts_vec = _mm256_loadu_pd(amounts.as_ptr().add(base_idx));
                let reserves_in_vec = _mm256_loadu_pd(reserves_in.as_ptr().add(base_idx));
                let reserves_out_vec = _mm256_loadu_pd(reserves_out.as_ptr().add(base_idx));
                
                // Uniswap V2 公式: amount_out = (amount_in * 997 * reserve_out) / (reserve_in * 1000 + amount_in * 997)
                let fee_factor = _mm256_set1_pd(997.0);
                let thousand = _mm256_set1_pd(1000.0);
                
                let numerator = _mm256_mul_pd(
                    _mm256_mul_pd(amounts_vec, fee_factor),
                    reserves_out_vec
                );
                
                let denominator = _mm256_add_pd(
                    _mm256_mul_pd(reserves_in_vec, thousand),
                    _mm256_mul_pd(amounts_vec, fee_factor)
                );
                
                let amount_out = _mm256_div_pd(numerator, denominator);
                
                // 计算滑点 = (理想价格 - 实际价格) / 理想价格
                let ideal_price = _mm256_div_pd(reserves_out_vec, reserves_in_vec);
                let actual_price = _mm256_div_pd(amount_out, amounts_vec);
                let slippage = _mm256_div_pd(
                    _mm256_sub_pd(ideal_price, actual_price),
                    ideal_price
                );
                
                _mm256_storeu_pd(slippages.as_mut_ptr().add(base_idx), slippage);
            }
        }

        // 处理剩余元素
        for i in (chunks * 4)..amounts.len() {
            let amount_out = (amounts[i] * 997.0 * reserves_out[i]) / 
                            (reserves_in[i] * 1000.0 + amounts[i] * 997.0);
            let ideal_price = reserves_out[i] / reserves_in[i];
            let actual_price = amount_out / amounts[i];
            slippages[i] = (ideal_price - actual_price) / ideal_price;
        }

        slippages
    }

    /// 标量版本的滑点计算
    pub fn calculate_slippage_scalar(
        amounts: &[f64],
        reserves_in: &[f64],
        reserves_out: &[f64],
    ) -> Vec<f64> {
        amounts
            .iter()
            .zip(reserves_in.iter())
            .zip(reserves_out.iter())
            .map(|((amount, reserve_in), reserve_out)| {
                let amount_out = (amount * 997.0 * reserve_out) / 
                                (reserve_in * 1000.0 + amount * 997.0);
                let ideal_price = reserve_out / reserve_in;
                let actual_price = amount_out / amount;
                (ideal_price - actual_price) / ideal_price
            })
            .collect()
    }

    /// 批量 U256 到 f64 转换（用于链上数据处理）
    pub fn u256_to_f64_batch(values: &[U256]) -> Vec<f64> {
        values
            .iter()
            .map(|v| v.as_u128() as f64)
            .collect()
    }

    /// 批量 f64 到 U256 转换
    pub fn f64_to_u256_batch(values: &[f64]) -> Vec<U256> {
        values
            .iter()
            .map(|&v| U256::from(v.max(0.0) as u128))
            .collect()
    }

    /// 向量化的套利机会评分
    pub fn score_arbitrage_opportunities(
        profits: &[f64],
        risks: &[f64],
        gas_costs: &[f64],
    ) -> Vec<f64> {
        let len = profits.len().min(risks.len()).min(gas_costs.len());
        let mut scores = Vec::with_capacity(len);

        for i in 0..len {
            let net_profit = profits[i] - gas_costs[i];
            let risk_adjusted_score = net_profit / (1.0 + risks[i]);
            scores.push(risk_adjusted_score);
        }

        scores
    }
}

/// 基准测试工具
pub struct SimdBenchmark;

impl SimdBenchmark {
    /// 比较 SIMD 和标量版本的性能
    pub fn benchmark_profit_calculation(size: usize, iterations: usize) -> (f64, f64) {
        use std::time::Instant;
        
        let amounts: Vec<f64> = (0..size).map(|i| i as f64 * 1000.0).collect();
        let price_diffs: Vec<f64> = (0..size).map(|i| 0.01 + (i as f64 * 0.001)).collect();

        // 基准测试标量版本
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = SimdCalculator::calculate_profits_scalar(&amounts, &price_diffs);
        }
        let scalar_time = start.elapsed().as_nanos() as f64 / iterations as f64;

        // 基准测试 SIMD 版本
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = SimdCalculator::calculate_profits_simd(&amounts, &price_diffs);
        }
        let simd_time = start.elapsed().as_nanos() as f64 / iterations as f64;

        (scalar_time, simd_time)
    }

    /// 验证 SIMD 计算正确性
    pub fn verify_simd_accuracy() -> bool {
        let amounts = vec![1000.0, 2000.0, 3000.0, 4000.0, 5000.0];
        let price_diffs = vec![0.01, 0.02, 0.015, 0.025, 0.03];

        let scalar_results = SimdCalculator::calculate_profits_scalar(&amounts, &price_diffs);
        let simd_results = SimdCalculator::calculate_profits_simd(&amounts, &price_diffs);

        for (scalar, simd) in scalar_results.iter().zip(simd_results.iter()) {
            if (scalar - simd).abs() > 1e-10 {
                return false;
            }
        }

        true
    }
}

/// SIMD 特性检测
pub struct SimdFeatures;

impl SimdFeatures {
    /// 检查是否支持 AVX2
    #[cfg(target_arch = "x86_64")]
    pub fn has_avx2() -> bool {
        is_x86_feature_detected!("avx2")
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub fn has_avx2() -> bool {
        false
    }

    /// 检查是否支持 FMA
    #[cfg(target_arch = "x86_64")]
    pub fn has_fma() -> bool {
        is_x86_feature_detected!("fma")
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub fn has_fma() -> bool {
        false
    }

    /// 获取 SIMD 能力报告
    pub fn get_capabilities() -> String {
        format!(
            "SIMD Capabilities:\n- AVX2: {}\n- FMA: {}",
            Self::has_avx2(),
            Self::has_fma()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profit_calculation() {
        let amounts = vec![1000.0, 2000.0, 3000.0];
        let price_diffs = vec![0.01, 0.02, 0.015];

        let scalar_results = SimdCalculator::calculate_profits_scalar(&amounts, &price_diffs);
        let simd_results = SimdCalculator::calculate_profits_simd(&amounts, &price_diffs);

        assert_eq!(scalar_results.len(), 3);
        assert_eq!(simd_results.len(), 3);

        // 验证结果近似相等
        for (scalar, simd) in scalar_results.iter().zip(simd_results.iter()) {
            assert!((scalar - simd).abs() < 1e-10);
        }
    }

    #[test]
    fn test_price_impact_calculation() {
        let amounts = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let liquidities = vec![10000.0, 20000.0, 30000.0, 40000.0];

        let scalar_results = SimdCalculator::calculate_price_impacts_scalar(&amounts, &liquidities);
        let simd_results = SimdCalculator::calculate_price_impacts_simd(&amounts, &liquidities);

        for (scalar, simd) in scalar_results.iter().zip(simd_results.iter()) {
            assert!((scalar - simd).abs() < 1e-10);
        }
    }

    #[test]
    fn test_simd_accuracy() {
        assert!(SimdBenchmark::verify_simd_accuracy());
    }

    #[test]
    fn test_u256_conversion() {
        let u256_values = vec![
            U256::from(1000),
            U256::from(2000),
            U256::from(3000),
        ];

        let f64_values = SimdCalculator::u256_to_f64_batch(&u256_values);
        let converted_back = SimdCalculator::f64_to_u256_batch(&f64_values);

        assert_eq!(u256_values, converted_back);
    }

    #[test]
    fn test_arbitrage_scoring() {
        let profits = vec![100.0, 200.0, 300.0];
        let risks = vec![0.1, 0.2, 0.15];
        let gas_costs = vec![10.0, 15.0, 20.0];

        let scores = SimdCalculator::score_arbitrage_opportunities(&profits, &risks, &gas_costs);
        
        assert_eq!(scores.len(), 3);
        assert!(scores[0] > 0.0);
        assert!(scores[1] > scores[0]); // 更高利润应该有更高分数
    }

    #[test]
    fn test_feature_detection() {
        let capabilities = SimdFeatures::get_capabilities();
        assert!(capabilities.contains("SIMD Capabilities"));
        assert!(capabilities.contains("AVX2"));
        assert!(capabilities.contains("FMA"));
    }
}
