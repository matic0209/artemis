//! Aave V2 and V3 contract bindings

use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;

// Aave V2 Lending Pool interface
sol! {
    #[sol(rpc)]
    interface IAaveV2Pool {
        /// Get user account data
        function getUserAccountData(address user)
            external
            view
            returns (
                uint256 totalCollateralETH,
                uint256 totalDebtETH,
                uint256 availableBorrowsETH,
                uint256 currentLiquidationThreshold,
                uint256 ltv,
                uint256 healthFactor
            );

        /// Get reserve data
        function getReserveData(address asset)
            external
            view
            returns (
                uint256 configuration,
                uint128 liquidityIndex,
                uint128 variableBorrowIndex,
                uint128 currentLiquidityRate,
                uint128 currentVariableBorrowRate,
                uint128 currentStableBorrowRate,
                uint40 lastUpdateTimestamp,
                address aTokenAddress,
                address stableDebtTokenAddress,
                address variableDebtTokenAddress,
                address interestRateStrategyAddress,
                uint8 id
            );

        /// Liquidation call
        function liquidationCall(
            address collateral,
            address debt,
            address user,
            uint256 debtToCover,
            bool receiveAToken
        ) external;
    }
}

// Aave V3 Pool interface
sol! {
    #[sol(rpc)]
    interface IAaveV3Pool {
        /// Get user account data
        function getUserAccountData(address user)
            external
            view
            returns (
                uint256 totalCollateralBase,
                uint256 totalDebtBase,
                uint256 availableBorrowsBase,
                uint256 currentLiquidationThreshold,
                uint256 ltv,
                uint256 healthFactor
            );

        /// Get reserve data
        function getReserveData(address asset)
            external
            view
            returns (
                uint256 configuration,
                uint128 liquidityIndex,
                uint128 currentLiquidityRate,
                uint128 variableBorrowIndex,
                uint128 currentVariableBorrowRate,
                uint128 currentStableBorrowRate,
                uint40 lastUpdateTimestamp,
                uint16 id,
                address aTokenAddress,
                address stableDebtTokenAddress,
                address variableDebtTokenAddress,
                address interestRateStrategyAddress,
                uint128 accruedToTreasury,
                uint128 unbacked,
                uint128 isolationModeTotalDebt
            );

        /// Liquidation call
        function liquidationCall(
            address collateralAsset,
            address debtAsset,
            address user,
            uint256 debtToCover,
            bool receiveAToken
        ) external;
    }
}

// Data Token (aToken) interface for getting user balance
sol! {
    #[sol(rpc)]
    interface IAToken {
        function balanceOf(address user) external view returns (uint256);
    }
}

// Debt Token interface
sol! {
    #[sol(rpc)]
    interface IDebtToken {
        function balanceOf(address user) external view returns (uint256);
    }
}

/// User account data from Aave
#[derive(Debug, Clone)]
pub struct AaveAccountData {
    pub total_collateral_eth: U256,
    pub total_debt_eth: U256,
    pub available_borrows_eth: U256,
    pub current_liquidation_threshold: U256,
    pub ltv: U256,
    pub health_factor: U256,
}

impl AaveAccountData {
    /// Convert health factor from U256 to f64
    /// Aave returns health factor with 18 decimals (1e18 = 1.0)
    pub fn health_factor_as_f64(&self) -> f64 {
        if self.health_factor == U256::MAX {
            // No debt, infinite health factor
            return f64::INFINITY;
        }

        // Convert from 18 decimals to float
        let hf_u128 = self.health_factor.to::<u128>();
        hf_u128 as f64 / 1e18
    }

    /// Check if position is underwater (can be liquidated)
    pub fn is_liquidatable(&self) -> bool {
        self.health_factor_as_f64() < 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_factor_conversion() {
        let account_data = AaveAccountData {
            total_collateral_eth: U256::from(2_000_000_000_000_000_000u64),
            total_debt_eth: U256::from(1_000_000_000_000_000_000u64),
            available_borrows_eth: U256::from(500_000_000_000_000_000u64),
            current_liquidation_threshold: U256::from(8000u64),
            ltv: U256::from(7500u64),
            health_factor: U256::from(950_000_000_000_000_000u64), // 0.95
        };

        let hf = account_data.health_factor_as_f64();
        assert!((hf - 0.95).abs() < 0.01);
        assert!(account_data.is_liquidatable());
    }

    #[test]
    fn test_healthy_position() {
        let account_data = AaveAccountData {
            total_collateral_eth: U256::from(2_000_000_000_000_000_000u64),
            total_debt_eth: U256::from(1_000_000_000_000_000_000u64),
            available_borrows_eth: U256::from(500_000_000_000_000_000u64),
            current_liquidation_threshold: U256::from(8000u64),
            ltv: U256::from(7500u64),
            health_factor: U256::from(1_500_000_000_000_000_000u64), // 1.5
        };

        let hf = account_data.health_factor_as_f64();
        assert!((hf - 1.5).abs() < 0.01);
        assert!(!account_data.is_liquidatable());
    }
}
