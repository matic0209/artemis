//! MEV Event System
//!
//! This module provides a unified event system for MEV arbitrage detection and execution.
//! It bridges the legacy event types with the new trait-based architecture.
//!
//! ## Features
//!
//! - **MEVEvent**: Standardized event types for blocks, transactions, and DEX updates
//! - **MEVAction**: Action types for arbitrage, liquidation, and defense
//! - **Priority System**: Multi-level priority queue for execution ordering
//! - **Event Routing**: Efficient event distribution to multiple strategies
//!
//! ## Migration Note
//!
//! This module extracts functionality from `legacy::mev_orchestrator::MEVEvent` and adapts
//! it to work with the new abstractions layer.

use std::collections::HashMap;
use alloy_primitives::{Address, U256, Bytes};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::abstractions::{
    ArbitrageOpportunity, OpportunityType, DetectionContext, MarketData, DetectionParams
};
use mev_arbitrage_graph::StateSnapshot;

/// MEV Event Types
///
/// Represents various blockchain events that could lead to MEV opportunities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MEVEvent {
    /// New block mined
    NewBlock {
        block_number: u64,
        timestamp: u64,
        gas_limit: U256,
        base_fee: U256,
    },

    /// Pending transaction in mempool
    PendingTransaction {
        tx_hash: String,
        from: Address,
        to: Option<Address>,
        value: U256,
        gas_price: U256,
        data: Bytes,
        timestamp: u64,
    },

    /// DEX state update (reserves changed)
    DexEvent {
        pool_address: Address,
        token0: Address,
        token1: Address,
        reserve0: U256,
        reserve1: U256,
        timestamp: u64,
    },

    /// Liquidation opportunity detected
    LiquidationEvent {
        protocol: String,
        user: Address,
        collateral: U256,
        debt: U256,
        timestamp: u64,
    },

    /// Price oracle update
    PriceUpdate {
        token: Address,
        price: U256,
        source: String,
        timestamp: u64,
    },
}

impl MEVEvent {
    /// Convert MEVEvent to DetectionContext for use with new architecture
    pub fn to_detection_context(&self) -> DetectionContext {
        match self {
            MEVEvent::NewBlock { block_number, timestamp, base_fee, .. } => {
                DetectionContext {
                    block_number: *block_number,
                    timestamp: *timestamp,
                    gas_price: *base_fee,
                    state_snapshot: StateSnapshot {
                        block_number: *block_number,
                        token_reserves: HashMap::new(),
                        spot_prices: HashMap::new(),
                        pools: HashMap::new(),
                        tokens: HashMap::new(),
                    },
                    market_data: MarketData {
                        token_prices: HashMap::new(),
                        pool_reserves: HashMap::new(),
                        gas_price_history: Vec::new(),
                        volume_data: HashMap::new(),
                    },
                    detection_params: DetectionParams {
                        min_profit_wei: U256::from(1000000000000000u64), // 0.001 ETH
                        max_gas_cost: U256::from(300000),
                        min_confidence: 0.6,
                        enable_flash_loans: true,
                        target_tokens: vec![],
                    },
                }
            }
            MEVEvent::DexEvent { timestamp, .. } => {
                DetectionContext {
                    block_number: 0, // Will be set by orchestrator
                    timestamp: *timestamp,
                    gas_price: U256::ZERO,
                    state_snapshot: StateSnapshot {
                        block_number: 0,
                        token_reserves: HashMap::new(),
                        spot_prices: HashMap::new(),
                        pools: HashMap::new(),
                        tokens: HashMap::new(),
                    },
                    market_data: MarketData {
                        token_prices: HashMap::new(),
                        pool_reserves: HashMap::new(),
                        gas_price_history: Vec::new(),
                        volume_data: HashMap::new(),
                    },
                    detection_params: DetectionParams {
                        min_profit_wei: U256::from(1000000000000000u64),
                        max_gas_cost: U256::from(300000),
                        min_confidence: 0.6,
                        enable_flash_loans: true,
                        target_tokens: vec![],
                    },
                }
            }
            _ => DetectionContext {
                block_number: 0,
                timestamp: chrono::Utc::now().timestamp() as u64,
                gas_price: U256::ZERO,
                state_snapshot: StateSnapshot {
                    block_number: 0,
                    token_reserves: HashMap::new(),
                    spot_prices: HashMap::new(),
                    pools: HashMap::new(),
                    tokens: HashMap::new(),
                },
                market_data: MarketData {
                    token_prices: HashMap::new(),
                    pool_reserves: HashMap::new(),
                    gas_price_history: Vec::new(),
                    volume_data: HashMap::new(),
                },
                detection_params: DetectionParams {
                    min_profit_wei: U256::from(1000000000000000u64),
                    max_gas_cost: U256::from(300000),
                    min_confidence: 0.6,
                    enable_flash_loans: true,
                    target_tokens: vec![],
                },
            }
        }
    }

    /// Get event timestamp
    pub fn timestamp(&self) -> u64 {
        match self {
            MEVEvent::NewBlock { timestamp, .. } => *timestamp,
            MEVEvent::PendingTransaction { timestamp, .. } => *timestamp,
            MEVEvent::DexEvent { timestamp, .. } => *timestamp,
            MEVEvent::LiquidationEvent { timestamp, .. } => *timestamp,
            MEVEvent::PriceUpdate { timestamp, .. } => *timestamp,
        }
    }

    /// Get event priority for processing
    pub fn priority(&self) -> EventPriority {
        match self {
            MEVEvent::PendingTransaction { .. } => EventPriority::High,
            MEVEvent::DexEvent { .. } => EventPriority::High,
            MEVEvent::LiquidationEvent { .. } => EventPriority::Critical,
            MEVEvent::PriceUpdate { .. } => EventPriority::Medium,
            MEVEvent::NewBlock { .. } => EventPriority::Low,
        }
    }
}

/// MEV Action Types
///
/// Represents actions that can be executed in response to MEV opportunities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MEVAction {
    /// Execute arbitrage opportunity
    ArbitrageExecution {
        opportunity_id: Uuid,
        path: Vec<Address>,
        amount_in: U256,
        min_amount_out: U256,
        gas_limit: u64,
        priority: ExecutionPriority,
    },

    /// Execute liquidation
    LiquidationExecution {
        target: Address,
        collateral_asset: Address,
        debt_asset: Address,
        amount: U256,
        gas_limit: u64,
    },

    /// Sandwich attack (for defense detection)
    SandwichAttack {
        front_run_tx: Bytes,
        back_run_tx: Bytes,
        target_tx_hash: String,
        profit_estimate: U256,
    },

    /// Defense action against MEV attack
    DefenseAction {
        protection_type: String,
        target_tx: String,
        counter_measure: Bytes,
    },

    /// Flash loan based arbitrage
    FlashLoanArbitrage {
        loan_token: Address,
        loan_amount: U256,
        execution_steps: Vec<ExecutionStep>,
        expected_profit: U256,
    },
}

impl MEVAction {
    /// Convert ArbitrageOpportunity to MEVAction
    pub fn from_opportunity(opp: &ArbitrageOpportunity) -> Option<Self> {
        match &opp.opportunity_type {
            OpportunityType::SimpleArbitrage { path, .. } => {
                Some(MEVAction::ArbitrageExecution {
                    opportunity_id: Uuid::new_v4(),
                    path: path.clone(),
                    amount_in: opp.required_capital,
                    min_amount_out: opp.expected_profit,
                    gas_limit: 500_000, // Default, should be estimated
                    priority: ExecutionPriority::from_confidence(opp.confidence),
                })
            }
            OpportunityType::FlashLoanArbitrage { loan_asset, loan_amount, .. } => {
                Some(MEVAction::FlashLoanArbitrage {
                    loan_token: *loan_asset,
                    loan_amount: *loan_amount,
                    execution_steps: vec![], // Will be filled by executor
                    expected_profit: opp.expected_profit,
                })
            }
            _ => None, // Not all opportunity types map to actions yet
        }
    }

    /// Get execution priority
    pub fn priority(&self) -> ExecutionPriority {
        match self {
            MEVAction::ArbitrageExecution { priority, .. } => *priority,
            MEVAction::LiquidationExecution { .. } => ExecutionPriority::High,
            MEVAction::SandwichAttack { .. } => ExecutionPriority::Critical,
            MEVAction::DefenseAction { .. } => ExecutionPriority::Critical,
            MEVAction::FlashLoanArbitrage { .. } => ExecutionPriority::High,
        }
    }

    /// Get estimated gas cost
    pub fn gas_limit(&self) -> u64 {
        match self {
            MEVAction::ArbitrageExecution { gas_limit, .. } => *gas_limit,
            MEVAction::LiquidationExecution { gas_limit, .. } => *gas_limit,
            MEVAction::FlashLoanArbitrage { .. } => 800_000, // Flash loans need more gas
            MEVAction::SandwichAttack { .. } => 600_000,
            MEVAction::DefenseAction { .. } => 400_000,
        }
    }
}

/// Execution Priority Levels
///
/// Determines the order and urgency of action execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub enum ExecutionPriority {
    /// Immediate execution required (e.g., defense, liquidation)
    Critical = 0,
    /// High priority (e.g., high-profit arbitrage)
    High = 1,
    /// Medium priority
    Medium = 2,
    /// Low priority (e.g., small profit opportunities)
    Low = 3,
}

impl ExecutionPriority {
    /// Determine priority from opportunity confidence
    pub fn from_confidence(confidence: f64) -> Self {
        if confidence >= 0.9 {
            ExecutionPriority::Critical
        } else if confidence >= 0.7 {
            ExecutionPriority::High
        } else if confidence >= 0.5 {
            ExecutionPriority::Medium
        } else {
            ExecutionPriority::Low
        }
    }

    /// Determine priority from profit/gas ratio
    pub fn from_profit_ratio(profit: U256, gas_cost: U256) -> Self {
        if gas_cost.is_zero() {
            return ExecutionPriority::Low;
        }

        // Simple ratio: profit / gas_cost
        // Use checked_div to avoid division issues
        let ratio = profit.checked_div(gas_cost).unwrap_or(U256::ZERO);

        if ratio > U256::from(10) {
            ExecutionPriority::Critical
        } else if ratio > U256::from(5) {
            ExecutionPriority::High
        } else if ratio > U256::from(2) {
            ExecutionPriority::Medium
        } else {
            ExecutionPriority::Low
        }
    }
}

/// Event Priority for processing order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

/// Execution Step
///
/// Represents a single step in a multi-step MEV execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_type: String,
    pub target_contract: Address,
    pub calldata: Bytes,
    pub value: U256,
    pub gas_estimate: u64,
}

impl ExecutionStep {
    /// Create a new execution step
    pub fn new(
        step_type: impl Into<String>,
        target_contract: Address,
        calldata: Bytes,
    ) -> Self {
        Self {
            step_type: step_type.into(),
            target_contract,
            calldata,
            value: U256::ZERO,
            gas_estimate: 100_000, // Default estimate
        }
    }

    /// Set the ETH value to send
    pub fn with_value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }

    /// Set the gas estimate
    pub fn with_gas_estimate(mut self, gas: u64) -> Self {
        self.gas_estimate = gas;
        self
    }
}

/// Event Router
///
/// Routes events to appropriate handlers based on event type and priority.
pub struct EventRouter {
    /// Event handlers by event type
    handlers: HashMap<String, Vec<Box<dyn Fn(&MEVEvent) + Send + Sync>>>,
}

impl EventRouter {
    /// Create a new event router
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register an event handler
    pub fn register_handler<F>(&mut self, event_type: &str, handler: F)
    where
        F: Fn(&MEVEvent) + Send + Sync + 'static,
    {
        self.handlers
            .entry(event_type.to_string())
            .or_insert_with(|| Vec::with_capacity(4)) // Pre-allocate for typical handler count
            .push(Box::new(handler));
    }

    /// Route an event to registered handlers
    pub fn route(&self, event: &MEVEvent) {
        let event_type = match event {
            MEVEvent::NewBlock { .. } => "new_block",
            MEVEvent::PendingTransaction { .. } => "pending_tx",
            MEVEvent::DexEvent { .. } => "dex_event",
            MEVEvent::LiquidationEvent { .. } => "liquidation",
            MEVEvent::PriceUpdate { .. } => "price_update",
        };

        if let Some(handlers) = self.handlers.get(event_type) {
            for handler in handlers {
                handler(event);
            }
        }
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_priority() {
        let liquidation = MEVEvent::LiquidationEvent {
            protocol: "Aave".to_string(),
            user: Address::ZERO,
            collateral: U256::from(1000),
            debt: U256::from(800),
            timestamp: 1234567890,
        };
        assert_eq!(liquidation.priority(), EventPriority::Critical);

        let block = MEVEvent::NewBlock {
            block_number: 100,
            timestamp: 1234567890,
            gas_limit: U256::from(30_000_000),
            base_fee: U256::from(20_000_000_000u64),
        };
        assert_eq!(block.priority(), EventPriority::Low);
    }

    #[test]
    fn test_execution_priority_from_confidence() {
        assert_eq!(
            ExecutionPriority::from_confidence(0.95),
            ExecutionPriority::Critical
        );
        assert_eq!(
            ExecutionPriority::from_confidence(0.75),
            ExecutionPriority::High
        );
        assert_eq!(
            ExecutionPriority::from_confidence(0.55),
            ExecutionPriority::Medium
        );
        assert_eq!(
            ExecutionPriority::from_confidence(0.3),
            ExecutionPriority::Low
        );
    }

    #[test]
    fn test_execution_step_builder() {
        let step = ExecutionStep::new(
            "swap",
            Address::repeat_byte(1),
            Bytes::from(vec![1, 2, 3]),
        )
        .with_value(U256::from(1000))
        .with_gas_estimate(150_000);

        assert_eq!(step.step_type, "swap");
        assert_eq!(step.value, U256::from(1000));
        assert_eq!(step.gas_estimate, 150_000);
    }

    #[test]
    fn test_event_to_detection_context() {
        let event = MEVEvent::NewBlock {
            block_number: 12345,
            timestamp: 1234567890,
            gas_limit: U256::from(30_000_000),
            base_fee: U256::from(20_000_000_000u64),
        };

        let ctx = event.to_detection_context();
        assert_eq!(ctx.block_number, 12345);
        assert_eq!(ctx.timestamp, 1234567890);
    }
}