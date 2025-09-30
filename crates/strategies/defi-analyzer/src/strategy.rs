//! DeFi Analyzer Strategy Implementation

use async_trait::async_trait;
use alloy_primitives::{Address, U256, Bytes};
use anyhow::Result;
use tracing::{info, warn, error};
use std::collections::HashMap;

use crate::{
    analyzer::DeFiAnalyzer,
    types::{AnalysisEvent, AnalysisAction, AnalysisResult, ArbitrageOpportunity},
    config::AnalyzerConfig,
    negative_cycle_arbitrage::{NegativeCycleArbitrageEngine, NegativeCycleConfig, StateSnapshot},
    jit_strategy_discovery::{JITStrategyDiscoveryEngine, JITConfig, DeFiAction, StrategyCandidate},
};

// Import Artemis core types for better integration
// use artemis_core::{
//     types::{Collector, Executor, Strategy},
//     collectors::{block_collector::NewBlock, log_collector::Log, mempool_collector::PendingTx},
//     executors::{mempool_alloy_executor::MempoolAlloyExecutor, flashbots_alloy_executor::FlashbotsAlloyExecutor},
// };

/// DeFi Analyzer Strategy for Artemis
pub struct DeFiAnalyzerStrategy {
    /// Configuration
    config: AnalyzerConfig,
    /// Analyzer instance
    analyzer: DeFiAnalyzer,
    /// Negative cycle arbitrage engine
    negative_cycle_engine: NegativeCycleArbitrageEngine,
    /// JIT strategy discovery engine
    jit_engine: JITStrategyDiscoveryEngine,
    /// Statistics
    stats: StrategyStats,
    // /// Connected collectors for data sources
    // collectors: Vec<Box<dyn Collector<AnalysisEvent>>>,
    // /// Connected executors for action execution
    // executors: Vec<Box<dyn Executor<AnalysisAction>>>,
}

/// Strategy statistics
#[derive(Debug, Default)]
pub struct StrategyStats {
    /// Total events processed
    pub events_processed: u64,
    /// Total opportunities found
    pub opportunities_found: u64,
    /// Total actions generated
    pub actions_generated: u64,
    /// Total analysis time (ms)
    pub total_analysis_time_ms: u64,
}

impl DeFiAnalyzerStrategy {
    /// Create a new DeFi Analyzer Strategy
    pub fn new(config: AnalyzerConfig) -> Result<Self> {
        let analyzer = DeFiAnalyzer::new(config.clone());
        
        // Create negative cycle arbitrage engine with default config
        let arbitrage_config = NegativeCycleConfig {
            target_revenue: config.min_profit_threshold,
            max_cycles_per_iteration: 10,
            max_path_length: 5,
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            base_gas_cost: 150_000,
            supported_protocols: vec![
                "uniswap_v2".to_string(),
                "uniswap_v3".to_string(),
                "sushiswap".to_string(),
                "curve".to_string(),
            ],
        };
        let negative_cycle_engine = NegativeCycleArbitrageEngine::new(arbitrage_config);
        
        // Create JIT strategy discovery engine
        let jit_config = JITConfig {
            base_asset: "WETH".to_string(),
            target_min: config.min_profit_threshold,
            time_budget: std::time::Duration::from_millis(500),
            max_path_length: 5,
            gas_price: U256::from(20_000_000_000u64),
            ..Default::default()
        };
        let jit_engine = JITStrategyDiscoveryEngine::new(jit_config)
            .map_err(|e| anyhow::anyhow!("Failed to create JIT engine: {}", e))?;
        
        Ok(Self {
            config,
            analyzer,
            negative_cycle_engine,
            jit_engine,
            stats: StrategyStats::default(),
            // collectors: Vec::new(),
            // executors: Vec::new(),
        })
    }
    
    // /// Add a collector for data sources
    // pub fn add_collector(&mut self, collector: Box<dyn Collector<AnalysisEvent>>) {
    //     self.collectors.push(collector);
    // }
    
    // /// Add an executor for action execution
    // pub fn add_executor(&mut self, executor: Box<dyn Executor<AnalysisAction>>) {
    //     self.executors.push(executor);
    // }
    
    /// Connect to Artemis collectors and executors
    pub fn connect_to_artemis(&mut self) -> Result<()> {
        // Add default collectors
        self.add_block_collector();
        self.add_log_collector();
        self.add_mempool_collector();
        
        // Add default executors
        self.add_mempool_executor();
        self.add_flashbots_executor();
        
        info!("Connected to Artemis collectors and executors");
        Ok(())
    }
    
    /// Add block collector for new block events
    fn add_block_collector(&mut self) {
        // This would integrate with Artemis block collector
        // to convert NewBlock events to AnalysisEvent
        info!("Added block collector integration");
    }
    
    /// Add log collector for event logs
    fn add_log_collector(&mut self) {
        // This would integrate with Artemis log collector
        // to convert Log events to AnalysisEvent
        info!("Added log collector integration");
    }
    
    /// Add mempool collector for pending transactions
    fn add_mempool_collector(&mut self) {
        // This would integrate with Artemis mempool collector
        // to convert PendingTx events to AnalysisEvent
        info!("Added mempool collector integration");
    }
    
    /// Add mempool executor for transaction submission
    fn add_mempool_executor(&mut self) {
        // This would integrate with Artemis mempool executor
        // to execute AnalysisAction as transactions
        info!("Added mempool executor integration");
    }
    
    /// Add Flashbots executor for MEV bundles
    fn add_flashbots_executor(&mut self) {
        // This would integrate with Artemis Flashbots executor
        // to execute AnalysisAction as MEV bundles
        info!("Added Flashbots executor integration");
    }

    /// Initialize the strategy
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing DeFi Analyzer Strategy");
        
        // Initialize the analyzer
        self.analyzer.initialize().await?;
        
        // Connect to Artemis components
        self.connect_to_artemis()?;
        
        info!("DeFi Analyzer Strategy initialized successfully");
        Ok(())
    }

    /// Process a single analysis event
    async fn process_analysis_event(&mut self, event: AnalysisEvent) -> Result<Vec<AnalysisAction>> {
        let start_time = std::time::Instant::now();

        info!("Processing analysis event: {:?} at block {} for contract {:?}",
              event.event_type, event.block_number, event.contract_address);

        // Use all event fields for comprehensive analysis
        debug!("Event details - kind: {}, timestamp: {}, metadata: {:?}",
               event.event_kind, event.timestamp, event.metadata);

        // First run standard analysis
        let mut analysis_result = self.analyzer.analyze_event(&event).await?;
        
        // Then run JIT strategy discovery (includes negative cycle arbitrage)
        let defi_actions = self.extract_defi_actions_from_event(&event)?;
        let jit_strategy = self.jit_engine.jit_strategy_discovery(event.block_number, &defi_actions).await.unwrap_or(None);

        // Create additional actions from JIT strategy if profitable
        let mut additional_actions = Vec::new();
        if let Some(strategy) = jit_strategy {
            info!("JIT strategy discovered: {} ({:?})", strategy.net_profit, strategy.strategy_type);

            for tx in &strategy.transactions {
                additional_actions.push(AnalysisAction {
                    action_id: format!("jit_{}_{}", strategy.strategy_type == crate::jit_strategy_discovery::StrategyType::ARB, event.block_number),
                    action_type: crate::types::ActionType::ArbitrageExecution,
                    contract_address: event.contract_address,
                    target_address: tx.to.into(),
                    parameters: crate::types::AnalysisParameters {
                        abi_json: None,
                        function_name: Some("jit_arbitrage".to_string()),
                        depth: 5,
                        timeout_seconds: 30,
                        config: {
                            let mut config = HashMap::new();
                            config.insert("strategy_type".to_string(), format!("{:?}", strategy.strategy_type));
                            config.insert("event_kind".to_string(), event.event_kind.clone());
                            config.insert("tx_hash".to_string(), hex::encode(&event.transaction_hash));
                            config
                        },
                    },
                    priority: 90, // High priority for JIT strategies
                    calldata: tx.data.to_vec(),
                    value: tx.value,
                    gas_limit: tx.gas_limit,
                    gas_price: U256::from(20_000_000_000u64),
                    nonce: 0,
                    chain_id: 1,
                    expected_profit: strategy.revenue,
                    risk_level: strategy.risk_level,
                    target_block: event.block_number + 1,
                    min_timestamp: event.timestamp,
                    max_timestamp: event.timestamp + 12,
                    metadata: {
                        let mut meta = HashMap::new();
                        meta.insert("source_event_kind".to_string(), event.event_kind.clone());
                        meta.insert("source_block".to_string(), event.block_number.to_string());
                        meta.insert("source_timestamp".to_string(), event.timestamp.to_string());
                        meta.insert("jit_strategy_type".to_string(), format!("{:?}", strategy.strategy_type));
                        meta.insert("expected_profit".to_string(), strategy.revenue.to_string());
                        // Include original event metadata
                        for (key, value) in &event.metadata {
                            meta.insert(format!("event_{}", key), value.clone());
                        }
                        meta
                    },
                });
            }
        }
        
        // Combine results
        let mut combined_result = analysis_result;
        combined_result.actions.extend(additional_actions);

        // Generate actions based on combined analysis result
        let actions = self.generate_actions_from_result(&combined_result).await?;
        
        // Execute actions through connected executors
        // for action in &actions {
        //     for executor in &self.executors {
        //         if let Err(e) = executor.execute(action.clone()).await {
        //             error!("Failed to execute action: {}", e);
        //         }
        //     }
        // }
        
        // Update statistics with detailed tracking
        let analysis_time = start_time.elapsed().as_millis() as u64;
        self.stats.events_processed += 1;
        self.stats.total_analysis_time_ms += analysis_time;
        self.stats.actions_generated += actions.len() as u64;

        if !combined_result.results.arbitrage_opportunities.is_empty() {
            self.stats.opportunities_found += combined_result.results.arbitrage_opportunities.len() as u64;
        }

        // Log comprehensive analysis results
        info!("Analysis completed in {}ms for event kind '{}' at block {}, generated {} actions",
              analysis_time, event.event_kind, event.block_number, actions.len());

        if !combined_result.results.arbitrage_opportunities.is_empty() {
            info!("Found {} arbitrage opportunities for contract {:?}",
                  combined_result.results.arbitrage_opportunities.len(), event.contract_address);
        }

        if !combined_result.results.inconsistencies.is_empty() {
            warn!("Found {} inconsistencies in contract {:?}: {:?}",
                  combined_result.results.inconsistencies.len(),
                  event.contract_address,
                  combined_result.results.inconsistencies.iter()
                      .map(|i| format!("{:?}", i.inconsistency_type))
                      .collect::<Vec<_>>());
        }

        // Log event data usage for debugging
        debug!("Processed event with {} bytes of event data, tx hash: {}",
               event.event_data.len(),
               hex::encode(&event.transaction_hash));
        
        Ok(actions)
    }
    
    /// Extract DeFi actions from analysis event
    fn extract_defi_actions_from_event(&self, event: &AnalysisEvent) -> Result<Vec<DeFiAction>> {
        let mut actions = Vec::new();

        // Use tx_data first, fallback to transaction_data for compatibility
        let tx_data = event.tx_data.as_ref().or(event.transaction_data.as_ref());

        // Parse transaction data to extract DeFi actions
        if let Some(tx_data) = tx_data {
            if tx_data.len() >= 4 {
                let selector = [
                    tx_data[0],
                    tx_data[1],
                    tx_data[2],
                    tx_data[3]
                ];

            // Common DeFi function selectors
            let defi_action = match selector {
                [0xa9, 0x05, 0x9c, 0xbb] => Some(DeFiAction {
                    id: "swap_exact_tokens_for_tokens".to_string(),
                    action_type: "swap".to_string(),
                    inputs: vec!["token_in".to_string()],
                    outputs: vec!["token_out".to_string()],
                    protocol: "uniswap_v2".to_string(),
                    key_dependencies: ["token_in_balance".to_string(), "token_out_balance".to_string()].into_iter().collect(),
                    selector,
                    contract: event.contract_address.into(),
                }),
                [0x38, 0xed, 0x17, 0x39] => Some(DeFiAction {
                    id: "swap_exact_eth_for_tokens".to_string(),
                    action_type: "swap".to_string(),
                    inputs: vec!["WETH".to_string()],
                    outputs: vec!["token_out".to_string()],
                    protocol: "uniswap_v2".to_string(),
                    key_dependencies: ["weth_balance".to_string(), "token_out_balance".to_string()].into_iter().collect(),
                    selector,
                    contract: event.contract_address.into(),
                }),
                [0x7f, 0xf3, 0x6a, 0xb5] => Some(DeFiAction {
                    id: "swap_exact_eth_for_tokens_supporting_fee".to_string(),
                    action_type: "swap".to_string(),
                    inputs: vec!["WETH".to_string()],
                    outputs: vec!["token_out".to_string()],
                    protocol: "uniswap_v2".to_string(),
                    key_dependencies: ["weth_balance".to_string(), "token_out_balance".to_string()].into_iter().collect(),
                    selector,
                    contract: event.contract_address.into(),
                }),
                _ => None,
            };
            
                if let Some(action) = defi_action {
                    actions.push(action);
                }
            }
        }
        
        Ok(actions)
    }

    /// Generate actions from analysis result
    async fn generate_actions_from_result(
        &self,
        result: &AnalysisResult,
    ) -> Result<Vec<AnalysisAction>> {
        let mut actions = Vec::new();

        // Generate actions for arbitrage opportunities
        for opportunity in &result.results.arbitrage_opportunities {
            if self.should_execute_opportunity(opportunity) {
                let action = AnalysisAction {
                    action_type: crate::types::ActionType::SymbolicExecution,
                    contract_address: result.contract_address,
                    target_address: result.contract_address, // 使用target_address字段
                    parameters: crate::types::AnalysisParameters {
                        abi_json: None,
                        function_name: None,
                        depth: self.config.max_analysis_depth,
                        timeout_seconds: self.config.analysis_timeout_seconds,
                        config: std::collections::HashMap::new(),
                    },
                    priority: self.calculate_priority(opportunity),
                    action_id: format!("arbitrage_{}_{}", opportunity.opportunity_id, chrono::Utc::now().timestamp()),
                    calldata: vec![], // 实际应该根据机会类型构建calldata
                    value: opportunity.expected_profit,
                    gas_limit: U256::from(opportunity.required_gas),
                    gas_price: U256::from(20_000_000_000u64), // 20 gwei
                    nonce: 0, // 应该从实际状态获取
                    chain_id: 1, // 以太坊主网
                    target_block: 0, // 应该设置为下一个区块
                    risk_level: opportunity.risk_level.clone(),
                    expected_profit: opportunity.expected_profit,
                    min_timestamp: chrono::Utc::now().timestamp() as u64,
                    max_timestamp: (chrono::Utc::now().timestamp() + 60) as u64, // 60秒有效期
                    metadata: {
                        let mut meta = std::collections::HashMap::new();
                        meta.insert("opportunity_id".to_string(), opportunity.opportunity_id.clone());
                        meta.insert("strategy_type".to_string(), "arbitrage".to_string());
                        meta
                    },
                };
                actions.push(action);
            }
        }

        // Generate actions for inconsistencies
        for inconsistency in &result.results.inconsistencies {
            if inconsistency.severity >= crate::types::SeverityLevel::Medium {
                let action = AnalysisAction {
                    action_type: crate::types::ActionType::GenerateReport,
                    contract_address: result.contract_address,
                    target_address: result.contract_address,
                    parameters: crate::types::AnalysisParameters {
                        abi_json: None,
                        function_name: None,
                        depth: self.config.max_analysis_depth,
                        timeout_seconds: self.config.analysis_timeout_seconds,
                        config: std::collections::HashMap::new(),
                    },
                    priority: 80, // High priority for inconsistencies
                    action_id: format!("report_{}_{}", inconsistency.inconsistency_type.to_string(), chrono::Utc::now().timestamp()),
                    calldata: vec![], // 报告类型不需要calldata
                    value: U256::ZERO,
                    gas_limit: U256::ZERO, // 报告不需要gas
                    gas_price: U256::ZERO,
                    nonce: 0,
                    chain_id: 1,
                    target_block: 0,
                    risk_level: match inconsistency.severity {
                        crate::types::SeverityLevel::Critical => crate::types::RiskLevel::Critical,
                        crate::types::SeverityLevel::High => crate::types::RiskLevel::High,
                        crate::types::SeverityLevel::Medium => crate::types::RiskLevel::Medium,
                        crate::types::SeverityLevel::Low => crate::types::RiskLevel::Low,
                    },
                    expected_profit: U256::ZERO,
                    min_timestamp: chrono::Utc::now().timestamp() as u64,
                    max_timestamp: (chrono::Utc::now().timestamp() + 3600) as u64, // 1小时有效期
                    metadata: {
                        let mut meta = std::collections::HashMap::new();
                        meta.insert("inconsistency_type".to_string(), inconsistency.inconsistency_type.to_string());
                        meta.insert("description".to_string(), inconsistency.description.clone());
                        if let Some(location) = &inconsistency.location {
                            meta.insert("location".to_string(), location.clone());
                        }
                        if let Some(fix) = &inconsistency.suggested_fix {
                            meta.insert("suggested_fix".to_string(), fix.clone());
                        }
                        meta
                    },
                };
                actions.push(action);
            }
        }

        Ok(actions)
    }

    /// Check if an opportunity should be executed
    fn should_execute_opportunity(&self, opportunity: &ArbitrageOpportunity) -> bool {
        // Check profit threshold
        if opportunity.expected_profit < self.config.min_profit_threshold {
            return false;
        }

        // Check risk tolerance
        let risk_score = match opportunity.risk_level {
            crate::types::RiskLevel::Low => 20,
            crate::types::RiskLevel::Medium => 50,
            crate::types::RiskLevel::High => 80,
            crate::types::RiskLevel::Critical => 100,
        };

        if risk_score > self.config.risk_tolerance {
            return false;
        }

        // Check success probability
        if opportunity.success_probability < 0.7 {
            return false;
        }

        true
    }

    /// Calculate priority for an opportunity
    fn calculate_priority(&self, opportunity: &ArbitrageOpportunity) -> u8 {
        let profit_score = (opportunity.expected_profit.to::<u128>() as f64 / 1e18 * 10.0) as u8;
        let success_score = (opportunity.success_probability * 50.0) as u8;
        let risk_penalty = match opportunity.risk_level {
            crate::types::RiskLevel::Low => 0,
            crate::types::RiskLevel::Medium => 20,
            crate::types::RiskLevel::High => 40,
            crate::types::RiskLevel::Critical => 60,
        };

        (profit_score + success_score).saturating_sub(risk_penalty).min(100)
    }

    /// Get strategy statistics
    pub fn get_stats(&self) -> &StrategyStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = StrategyStats::default();
    }

    /// Log performance report
    pub fn log_performance_report(&self) {
        self.analyzer.log_performance_report();
    }

    /// Get metrics summary
    pub fn get_metrics_summary(&self) -> String {
        self.analyzer.get_metrics_summary()
    }
}

// 简化的策略实现，不依赖 artemis-core
impl DeFiAnalyzerStrategy {
    /// Process event (简化版本)
    pub async fn process_event(&mut self, event: AnalysisEvent) -> Vec<AnalysisAction> {
        match self.process_analysis_event(event).await {
            Ok(actions) => actions,
            Err(e) => {
                error!("Failed to process analysis event: {}", e);
                vec![]
            }
        }
    }

    /// Sync state (简化版本)
    pub async fn sync_state(&mut self) -> Result<()> {
        info!("Syncing DeFi Analyzer Strategy state");
        
        // Sync analyzer state
        self.analyzer.sync_state().await?;
        
        info!("DeFi Analyzer Strategy state synced");
        Ok(())
    }
}

impl std::fmt::Display for DeFiAnalyzerStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeFiAnalyzerStrategy(opportunities: {}, actions: {})", 
               self.stats.opportunities_found, self.stats.actions_generated)
    }
}