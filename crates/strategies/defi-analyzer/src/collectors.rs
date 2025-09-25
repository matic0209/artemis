//! DeFi Analyzer Collectors for Artemis Integration
//! 
//! This module provides collectors that integrate with Artemis core collectors
//! to convert various event types into DeFi analysis events.

use anyhow::Result;
use async_trait::async_trait;
use tokio_stream::{Stream, StreamExt};
use tracing::{info, debug, error};

use crate::{
    types::{AnalysisEvent, EventType},
    error::DeFiResult,
};

use artemis_core::{
    types::Collector,
    collectors::{
        block_collector::NewBlock,
        log_collector::LogCollector,
        mempool_collector::MempoolCollector,
    },
};
use alloy_rpc_types_eth::Log;
use artemis_core::eth::Transaction;

/// DeFi Block Collector
/// Converts NewBlock events to DeFi analysis events
pub struct DeFiBlockCollector {
    /// Underlying block collector
    block_collector: Box<dyn Collector<NewBlock>>,
    /// Configuration
    config: DeFiCollectorConfig,
}

/// DeFi Log Collector
/// Converts Log events to DeFi analysis events
pub struct DeFiLogCollector {
    /// Underlying log collector
    log_collector: Box<dyn Collector<Log>>,
    /// Configuration
    config: DeFiCollectorConfig,
}

/// DeFi Mempool Collector
/// Converts Transaction events to DeFi analysis events
pub struct DeFiMempoolCollector {
    /// Underlying mempool collector
    mempool_collector: Box<dyn Collector<Transaction>>,
    /// Configuration
    config: DeFiCollectorConfig,
}

/// DeFi Collector Configuration
#[derive(Debug, Clone)]
pub struct DeFiCollectorConfig {
    /// Enable block analysis
    pub enable_block_analysis: bool,
    /// Enable log analysis
    pub enable_log_analysis: bool,
    /// Enable mempool analysis
    pub enable_mempool_analysis: bool,
    /// Minimum gas price threshold
    pub min_gas_price: u64,
    /// DeFi protocol addresses to monitor
    pub monitored_protocols: Vec<String>,
}

impl Default for DeFiCollectorConfig {
    fn default() -> Self {
        Self {
            enable_block_analysis: true,
            enable_log_analysis: true,
            enable_mempool_analysis: true,
            min_gas_price: 20_000_000_000, // 20 gwei
            monitored_protocols: vec![
                "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".to_string(), // Uniswap V2 Router
                "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(), // Uniswap V3 Router
                "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".to_string(), // SushiSwap Router
            ],
        }
    }
}

impl DeFiBlockCollector {
    /// Create a new DeFi block collector
    pub fn new(block_collector: Box<dyn Collector<NewBlock>>, config: DeFiCollectorConfig) -> Self {
        Self {
            block_collector,
            config,
        }
    }
    
    /// Convert NewBlock to AnalysisEvent
    fn convert_block_to_analysis_event(&self, block: NewBlock) -> Option<AnalysisEvent> {
        if !self.config.enable_block_analysis {
            return None;
        }
        
        // Check if block contains DeFi activity
        let has_defi_activity = self.check_defi_activity(&block);
        if !has_defi_activity {
            return None;
        }
        
        Some(AnalysisEvent {
            event_type: "block_analysis".to_string(),
            event_kind: None,
            contract_address: [0u8; 20].into(),
            transaction_hash: [0u8; 32],
            transaction_data: vec![],
            tx_data: None,
            event_data: serde_json::to_vec(&block).unwrap_or_default(),
            block_number: block.number,
            timestamp: block.timestamp,
            metadata: std::collections::HashMap::new(),
        })
    }
    
    /// Check if block contains DeFi activity
    fn check_defi_activity(&self, block: &NewBlock) -> bool {
        // Check if any transactions in the block interact with monitored protocols
        // This is a simplified check - in practice, you'd analyze transaction data
        block.transactions.len() > 0
    }
}

impl DeFiLogCollector {
    /// Create a new DeFi log collector
    pub fn new(log_collector: Box<dyn Collector<Log>>, config: DeFiCollectorConfig) -> Self {
        Self {
            log_collector,
            config,
        }
    }
    
    /// Convert Log to AnalysisEvent
    fn convert_log_to_analysis_event(&self, log: Log) -> Option<AnalysisEvent> {
        if !self.config.enable_log_analysis {
            return None;
        }
        
        // Check if log is from a monitored protocol
        let is_monitored = self.is_monitored_protocol(&log.address);
        if !is_monitored {
            return None;
        }
        
        Some(AnalysisEvent {
            event_type: "log_analysis".to_string(),
            event_kind: None,
            contract_address: log.address,
            transaction_hash: log.transaction_hash,
            transaction_data: log.data,
            tx_data: None,
            event_data: serde_json::to_vec(&log).unwrap_or_default(),
            block_number: log.block_number,
            timestamp: log.timestamp,
            metadata: std::collections::HashMap::new(),
        })
    }
    
    /// Check if address is a monitored protocol
    fn is_monitored_protocol(&self, address: &[u8; 20]) -> bool {
        let address_str = format!("0x{}", hex::encode(address));
        self.config.monitored_protocols.contains(&address_str)
    }
}

impl DeFiMempoolCollector {
    /// Create a new DeFi mempool collector
    pub fn new(mempool_collector: Box<dyn Collector<Transaction>>, config: DeFiCollectorConfig) -> Self {
        Self {
            mempool_collector,
            config,
        }
    }
    
    /// Convert Transaction to AnalysisEvent
    fn convert_tx_to_analysis_event(&self, tx: Transaction) -> Option<AnalysisEvent> {
        if !self.config.enable_mempool_analysis {
            return None;
        }
        
        // Check if transaction meets gas price threshold
        if tx.gas_price < self.config.min_gas_price {
            return None;
        }
        
        // Check if transaction interacts with monitored protocols
        let is_defi_tx = self.is_defi_transaction(&tx);
        if !is_defi_tx {
            return None;
        }
        
        Some(AnalysisEvent {
            event_type: "mempool_analysis".to_string(),
            event_kind: None,
            contract_address: tx.to.unwrap_or([0u8; 20].into()),
            transaction_hash: tx.hash,
            transaction_data: tx.input,
            tx_data: None,
            event_data: serde_json::to_vec(&tx).unwrap_or_default(),
            block_number: 0,
            timestamp: tx.timestamp,
            metadata: std::collections::HashMap::new(),
        })
    }
    
    /// Check if transaction is a DeFi transaction
    fn is_defi_transaction(&self, tx: &Transaction) -> bool {
        // Check if transaction is to a monitored protocol
        if let Some(to) = tx.to {
            let address_str = format!("0x{}", hex::encode(to));
            return self.config.monitored_protocols.contains(&address_str);
        }
        
        // Check if transaction data contains DeFi function selectors
        self.contains_defi_function_selector(&tx.input)
    }
    
    /// Check if transaction data contains DeFi function selectors
    fn contains_defi_function_selector(&self, data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }
        
        let selector = &data[0..4];
        
        // Common DeFi function selectors
        let defi_selectors = [
            [0x7f, 0xff, 0x0a, 0x95], // swapExactTokensForTokens
            [0x38, 0xed, 0x17, 0x39], // swapExactETHForTokens
            [0x88, 0x03, 0xdb, 0xee], // swapTokensForExactTokens
            [0x4a, 0x25, 0xd9, 0x41], // swapExactTokensForETH
            [0x02, 0x2c, 0x0d, 0x9f], // swapExactETHForTokensSupportingFeeOnTransferTokens
            [0x5c, 0x11, 0xd7, 0x95], // swapExactTokensForTokensSupportingFeeOnTransferTokens
        ];
        
        defi_selectors.contains(selector)
    }
}

// Implement Collector trait for DeFi collectors
#[async_trait]
impl Collector<AnalysisEvent> for DeFiBlockCollector {
    async fn get_event_stream(&self) -> Result<Box<dyn Stream<Item = AnalysisEvent> + Send + '_>> {
        let block_stream = self.block_collector.get_event_stream().await?;
        let config = self.config.clone();
        
        let analysis_stream = block_stream
            .filter_map(move |block| {
                let collector = DeFiBlockCollector {
                    block_collector: Box::new(artemis_core::collectors::block_collector::BlockCollector::new()),
                    config: config.clone(),
                };
                collector.convert_block_to_analysis_event(block)
            });
        
        Ok(Box::new(analysis_stream))
    }
}

#[async_trait]
impl Collector<AnalysisEvent> for DeFiLogCollector {
    async fn get_event_stream(&self) -> Result<Box<dyn Stream<Item = AnalysisEvent> + Send + '_>> {
        let log_stream = self.log_collector.get_event_stream().await?;
        let config = self.config.clone();
        
        let analysis_stream = log_stream
            .filter_map(move |log| {
                let collector = DeFiLogCollector {
                    log_collector: Box::new(artemis_core::collectors::log_collector::LogCollector::new()),
                    config: config.clone(),
                };
                collector.convert_log_to_analysis_event(log)
            });
        
        Ok(Box::new(analysis_stream))
    }
}

#[async_trait]
impl Collector<AnalysisEvent> for DeFiMempoolCollector {
    async fn get_event_stream(&self) -> Result<Box<dyn Stream<Item = AnalysisEvent> + Send + '_>> {
        let mempool_stream = self.mempool_collector.get_event_stream().await?;
        let config = self.config.clone();
        
        let analysis_stream = mempool_stream
            .filter_map(move |tx| {
                let collector = DeFiMempoolCollector {
                    mempool_collector: Box::new(artemis_core::collectors::mempool_collector::MempoolCollector::new()),
                    config: config.clone(),
                };
                collector.convert_tx_to_analysis_event(tx)
            });
        
        Ok(Box::new(analysis_stream))
    }
}
