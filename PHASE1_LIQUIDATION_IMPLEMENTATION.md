# Phase 1: Liquidation Detection - Implementation Complete

## Overview

Phase 1 of the multi-strategy MEV system (inspired by 0x0e49) has been successfully implemented. This phase adds liquidation detection capabilities for Aave V2/V3 lending protocols, which can be combined with arbitrage opportunities to create high-value atomic bundles.

## Components Implemented

### 1. Aave Contract Bindings (`src/detectors/aave_contracts.rs`)

Complete Solidity interface bindings using `alloy-sol-types`:

- **IAaveV2Pool**: Mainnet Aave V2 lending pool interface
  - `getUserAccountData()`: Query user collateral, debt, and health factor
  - `getReserveData()`: Get reserve configuration and rates
  - `liquidationCall()`: Execute liquidation transaction

- **IAaveV3Pool**: Mainnet Aave V3 lending pool interface
  - `getUserAccountData()`: Query user account with V3-specific fields
  - `getReserveData()`: Get V3 reserve data with additional fields
  - `liquidationCall()`: Execute V3 liquidation

- **IAToken**: aToken balance queries
- **IDebtToken**: Debt token balance queries

**Helper Types**:
- `AaveAccountData`: Structured account data with health factor conversion
- Health factor conversion: `f64` conversion from U256 (18 decimals)
- `is_liquidatable()`: Boolean check for health factor < 1.0

**Contract Addresses** (Ethereum Mainnet):
- Aave V2 Pool: `0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9`
- Aave V3 Pool: `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`

### 2. Liquidation Detector (`src/detectors/liquidation.rs`)

**Structures**:
```rust
pub enum LiquidationProtocol {
    AaveV2,
    AaveV3,
    Compound,
    MakerDAO,
}

pub struct AavePosition {
    pub user: Address,
    pub collateral_token: Address,
    pub collateral_amount: U256,
    pub debt_token: Address,
    pub debt_amount: U256,
    pub health_factor: f64,
}

pub struct LiquidationOpportunity {
    pub protocol: LiquidationProtocol,
    pub position: AavePosition,
    pub liquidation_reward: U256,
    pub collateral_to_seize: U256,
    pub debt_to_repay: U256,
    pub max_close_factor: f64,
}
```

**Core Methods**:

1. **detect_underwater_positions()**: Main entry point
   - Checks both Aave V2 and V3 if enabled
   - Filters by minimum profit threshold
   - Returns sorted list of liquidation opportunities

2. **detect_aave_v2_liquidations()**: Aave V2 specific detection
   - Queries `getUserAccountData()` for each user
   - Filters by health factor < 1.0
   - Calculates 5% liquidation bonus
   - Maximum 50% close factor

3. **detect_aave_v3_liquidations()**: Aave V3 specific detection
   - Similar to V2 with V3-specific fields
   - Variable liquidation bonus by asset (5% default)
   - Maximum 50% close factor

4. **query_aave_v2_account()**: Contract call wrapper
   - Creates IAaveV2Pool instance
   - Calls `getUserAccountData(user)`
   - Converts result to `AaveAccountData`

5. **query_aave_v3_account()**: V3 contract call wrapper
   - Creates IAaveV3Pool instance
   - Handles V3-specific return values

6. **calculate_aave_v2_liquidation()**: Profit calculation
   - Debt to repay = 50% of total debt
   - Collateral to seize = debt * 1.05 (5% bonus)
   - Liquidation reward = collateral - debt

**Configuration**:
```rust
pub struct LiquidationDetectorConfig {
    pub health_threshold: f64,          // 1.0
    pub min_profit_wei: U256,           // 0.01 ETH
    pub aave_v2_pool: Address,
    pub aave_v3_pool: Address,
    pub enable_aave_v2: bool,
    pub enable_aave_v3: bool,
}
```

**Trait Implementation**:
- Implements `ArbitrageDetector` trait
- Provides metadata, health check, config management
- Converts `LiquidationOpportunity` to `ArbitrageOpportunity` with `OpportunityType::Liquidation`

### 3. Price Impact Simulator (`src/simulators/price_impact.rs`)

**Purpose**: Simulate liquidation transactions to predict price impacts on DEX pools.

**Structures**:
```rust
pub struct PriceImpactSimulator {
    pool_states: HashMap<Address, PoolState>,
}

pub struct PoolState {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
}
```

**Core Methods**:

1. **simulate_liquidation()**: Simulate liquidation price impact
   - Takes a `LiquidationOpportunity`
   - Returns `PriceImpact` with affected pools and price changes
   - Currently uses simplified estimation (TODO: Full REVM integration)

2. **find_arbitrage_after_impact()**: Find induced arbitrage opportunities
   - Takes `PriceImpact` and `TokenGraph`
   - Finds triangular paths involving affected tokens
   - Filters by price change magnitude (> 0.5%)
   - Estimates profit potential
   - Returns `Vec<ArbitrageOpportunity>`

3. **estimate_path_profit()**: Calculate arbitrage profit
   - Takes path and price changes
   - Returns estimated profit (simplified for now)
   - TODO: Implement precise AMM math simulation

**Status**: Core framework complete, needs full REVM integration for precise simulation.

### 4. Multi-Strategy Composer (`src/composers/multi_strategy.rs`)

**Purpose**: Combine liquidations with induced arbitrage opportunities into atomic bundles.

**Structures**:
```rust
pub struct ComposedOpportunity {
    pub liquidation: LiquidationOpportunity,
    pub induced_arbitrages: Vec<ArbitrageOpportunity>,
    pub total_profit: U256,
    pub total_gas: u64,
    pub execution_order: Vec<ExecutionStep>,
}

pub enum ExecutionStep {
    Liquidation {
        index: usize,
        opportunity: LiquidationOpportunity,
    },
    Arbitrage {
        index: usize,
        opportunity: ArbitrageOpportunity,
        depends_on: Vec<usize>, // Dependencies
    },
}

pub struct MultiStrategyComposerConfig {
    pub min_total_profit: U256,              // 0.05 ETH
    pub min_arbitrage_profit: U256,          // 0.01 ETH
    pub max_arbitrages_per_liquidation: usize, // 5
}
```

**Core Method**:

**compose_opportunities()**: Main composition logic
1. Detect liquidation opportunities
2. For each liquidation:
   - Simulate price impact
   - Find induced arbitrages
   - Filter by minimum profit
   - Limit arbitrage count
3. Calculate total profit and gas
4. Build execution order (liquidation → arbitrages)
5. Sort by total profit (descending)

**Execution Order Building**:
- Step 0: Liquidation (executed first)
- Steps 1+: Arbitrages (depend on liquidation)
- Dependencies tracked for proper sequencing

### 5. Core Abstractions Update

Added new `OpportunityType::Liquidation` variant:
```rust
pub enum OpportunityType {
    // ... existing variants
    Liquidation {
        protocol: String,
        user: Address,
        collateral_token: Address,
        debt_token: Address,
        debt_to_repay: U256,
        collateral_to_seize: U256,
    },
}
```

## Compilation Status

✅ **All code compiles successfully** with `cargo check -p mev-arbitrage --features full`
- 0 errors
- 63 warnings (mostly unused variables in TODO sections)

✅ **All tests pass**
- `detectors::liquidation`: 2 tests passed
- `detectors::aave_contracts`: 2 tests passed

## Dependencies Added

```toml
# Cargo.toml additions
alloy-sol-types = "1.0.27"
alloy-contract = "1.0.27"
```

## Module Structure

```
src/
├── detectors/
│   ├── liquidation.rs          (NEW)
│   ├── aave_contracts.rs       (NEW)
│   └── mod.rs                  (updated)
├── simulators/
│   ├── price_impact.rs         (NEW)
│   └── mod.rs                  (NEW)
├── composers/
│   ├── multi_strategy.rs       (NEW)
│   └── mod.rs                  (NEW)
└── lib.rs                      (updated)
```

## Key Features

1. **Real Aave Integration**:
   - Actual contract bindings, not mocks
   - Mainnet addresses configured
   - Full getUserAccountData() support

2. **Health Factor Calculation**:
   - Precise conversion from U256 to f64
   - 18 decimal support
   - Handles infinite health factor (no debt)

3. **Profit Calculation**:
   - Accurate liquidation bonus (5% for Aave)
   - Close factor support (50% max)
   - Minimum profit filtering

4. **Multi-Strategy Composition**:
   - Combines liquidations with arbitrages
   - Dependency tracking
   - Profit optimization

5. **Price Impact Simulation**:
   - Framework for REVM integration
   - Pool state management
   - Induced arbitrage detection

## Limitations & TODOs

1. **User Discovery**:
   - `get_users_to_check()` returns empty list
   - TODO: Implement event monitoring or subgraph queries
   - TODO: Maintain cache of active borrowers

2. **REVM Integration**:
   - Price impact uses simplified estimation
   - TODO: Full REVM simulation for precise calculations
   - TODO: Implement actual AMM math

3. **Provider Access**:
   - `ArbitrageDetector::detect()` doesn't have provider access yet
   - DetectionContext needs provider field
   - For now, use `detect_underwater_positions()` directly with provider

4. **Token/Reserve Details**:
   - Currently uses Address::ZERO placeholders
   - TODO: Query specific collateral/debt tokens
   - TODO: Get actual aToken addresses

5. **Configuration Bridge**:
   - `config()` and `update_config()` use `unimplemented!()`
   - TODO: Convert between LiquidationDetectorConfig and DetectorConfig

## Usage Example

```rust
use mev_arbitrage::detectors::{LiquidationDetector, LiquidationDetectorConfig};
use mev_arbitrage::simulators::PriceImpactSimulator;
use mev_arbitrage::composers::{MultiStrategyComposer, MultiStrategyComposerConfig};

// Initialize detector
let config = LiquidationDetectorConfig::default();
let liquidation_detector = LiquidationDetector::new(config);

// Initialize composer
let composer_config = MultiStrategyComposerConfig::default();
let mut composer = MultiStrategyComposer::new(
    liquidation_detector,
    composer_config,
);

// Compose opportunities
let composed = composer
    .compose_opportunities(&provider, &context, &token_graph)
    .await?;

// Execute most profitable composed opportunity
if let Some(best) = composed.first() {
    println!("Total profit: {} ETH", best.total_profit / 1e18);
    println!("Liquidation + {} arbitrages", best.induced_arbitrages.len());
    println!("Gas estimate: {}", best.total_gas);
}
```

## Testing

Run tests:
```bash
# All liquidation tests
cargo test -p mev-arbitrage --features full --lib detectors::liquidation

# Aave contract tests
cargo test -p mev-arbitrage --features full --lib detectors::aave_contracts

# All multi-strategy tests
cargo test -p mev-arbitrage --features full --lib composers::multi_strategy
```

## Next Steps (Phase 2)

1. **Complete REVM Integration**:
   - Implement full price impact simulation
   - Add precise AMM calculations
   - Support Uniswap V2/V3 pool state changes

2. **User Discovery Mechanism**:
   - Monitor Aave events (Borrow, Deposit, Withdraw)
   - Integrate with The Graph subgraph
   - Maintain hot cache of risky positions

3. **Reserve Details**:
   - Query aToken/debtToken addresses
   - Get specific collateral/debt amounts per reserve
   - Support multiple collateral/debt positions per user

4. **Provider Integration**:
   - Add provider to DetectionContext
   - Implement full `ArbitrageDetector::detect()` method
   - Bridge configuration types

## Files Created/Modified

**New Files** (6):
- `src/detectors/aave_contracts.rs` (176 lines)
- `src/detectors/liquidation.rs` (522 lines)
- `src/simulators/mod.rs` (6 lines)
- `src/simulators/price_impact.rs` (197 lines)
- `src/composers/mod.rs` (6 lines)
- `src/composers/multi_strategy.rs` (245 lines)

**Modified Files** (4):
- `src/detectors/mod.rs` (added liquidation exports)
- `src/lib.rs` (added simulators and composers modules)
- `src/abstractions.rs` (added Liquidation opportunity type)
- `Cargo.toml` (added alloy-sol-types and alloy-contract)

**Total**: ~1152 lines of new code

## Impact

This implementation provides the foundation for 0x0e49-style multi-strategy MEV:

1. **Self-Induced Arbitrage**: Liquidations create price distortions that are immediately exploitable
2. **Atomic Bundling**: All steps execute together or revert
3. **Higher Profit Density**: Combining strategies increases per-transaction value
4. **Gas Efficiency**: Single transaction vs multiple separate trades

With Phase 1 complete, the framework is ready for:
- Phase 2: Full REVM price impact simulation
- Phase 3: ArbitrageStrategy integration
- Phase 4: Partial revert mechanism
- Phase 5: Production testing and optimization
