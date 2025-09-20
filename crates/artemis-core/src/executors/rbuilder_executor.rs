#[cfg(feature = "rbuilder-integration")]
use std::sync::Arc;

#[cfg(feature = "rbuilder-integration")]
use anyhow::{anyhow, Result};
#[cfg(feature = "rbuilder-integration")]
use async_trait::async_trait;
#[cfg(feature = "rbuilder-integration")]
use tracing::{info, warn};

#[cfg(feature = "rbuilder-integration")]
use crate::types::Executor;

/// Configuration for rbuilder integration
#[cfg(feature = "rbuilder-integration")]
#[derive(Debug, Clone)]
pub struct RbuilderConfig {
    /// rbuilder JSON-RPC endpoint
    pub rpc_url: String,
    /// Block building algorithm to use
    pub sorting_algorithm: String,
    /// Relay endpoints to submit to
    pub relay_urls: Vec<String>,
    /// Enable local optimization before submission
    pub enable_optimization: bool,
}

#[cfg(feature = "rbuilder-integration")]
impl Default for RbuilderConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8645".to_string(),
            sorting_algorithm: "max-profit".to_string(),
            relay_urls: vec!["https://boost-relay.flashbots.net".to_string()],
            enable_optimization: true,
        }
    }
}

/// Bundle data structure compatible with rbuilder
#[cfg(feature = "rbuilder-integration")]
#[derive(Debug, Clone)]
pub struct RbuilderBundle {
    /// Raw signed transactions (0x-prefixed hex)
    pub txs: Vec<String>,
    /// Target block number
    pub target_block: Option<u64>,
    /// Minimum timestamp
    pub min_timestamp: Option<u64>,
    /// Maximum timestamp  
    pub max_timestamp: Option<u64>,
    /// Transactions that can revert
    pub reverting_tx_hashes: Vec<alloy_primitives::B256>,
    /// Bundle replacement UUID
    pub replacement_uuid: Option<uuid::Uuid>,
}

/// Advanced executor using rbuilder for local block building optimization
#[cfg(feature = "rbuilder-integration")]
pub struct RbuilderExecutor {
    config: RbuilderConfig,
    client: reqwest::Client,
}

#[cfg(feature = "rbuilder-integration")]
impl RbuilderExecutor {
    pub fn new(config: RbuilderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Submit bundle via rbuilder's JSON-RPC API
    async fn submit_bundle_to_rbuilder(&self, bundle: &RbuilderBundle) -> Result<String> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendBundle",
            "params": [{
                "txs": bundle.txs,
                "blockNumber": bundle.target_block.map(|n| format!("0x{:x}", n)),
                "minTimestamp": bundle.min_timestamp,
                "maxTimestamp": bundle.max_timestamp,
                "revertingTxHashes": bundle.reverting_tx_hashes.iter()
                    .map(|h| format!("0x{:x}", h))
                    .collect::<Vec<_>>(),
                "replacementUuid": bundle.replacement_uuid.map(|u| u.to_string()),
            }],
            "id": 1
        });

        let response = self.client
            .post(&self.config.rpc_url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("rbuilder request failed: {}", response.status()));
        }

        let result: serde_json::Value = response.json().await?;
        
        if let Some(error) = result.get("error") {
            return Err(anyhow!("rbuilder error: {}", error));
        }

        let bundle_hash = result["result"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        Ok(bundle_hash)
    }

    /// Simulate bundle locally using rbuilder
    async fn simulate_bundle(&self, bundle: &RbuilderBundle) -> Result<serde_json::Value> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0", 
            "method": "eth_callBundle",
            "params": [{
                "txs": bundle.txs,
                "blockNumber": bundle.target_block.map(|n| format!("0x{:x}", n)),
                "stateBlockNumber": "latest"
            }],
            "id": 2
        });

        let response = self.client
            .post(&self.config.rpc_url)
            .json(&payload)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        Ok(result)
    }
}

#[cfg(feature = "rbuilder-integration")]
#[async_trait]
impl Executor<RbuilderBundle> for RbuilderExecutor {
    async fn execute(&self, bundle: RbuilderBundle) -> Result<()> {
        let start = std::time::Instant::now();
        
        // 1. Local simulation for validation
        if self.config.enable_optimization {
            match self.simulate_bundle(&bundle).await {
                Ok(sim_result) => {
                    info!("Bundle simulation successful: {:?}", sim_result);
                    metrics::counter!("artemis.rbuilder.simulation_success").increment(1);
                }
                Err(e) => {
                    warn!("Bundle simulation failed: {e}");
                    metrics::counter!("artemis.rbuilder.simulation_failed").increment(1);
                    // Continue with submission anyway
                }
            }
        }

        // 2. Submit to rbuilder for optimized block building
        match self.submit_bundle_to_rbuilder(&bundle).await {
            Ok(bundle_hash) => {
                let elapsed = start.elapsed();
                info!("Bundle submitted via rbuilder: {bundle_hash}");
                metrics::histogram!("artemis.rbuilder.submit_duration")
                    .record(elapsed.as_millis() as f64);
                metrics::counter!("artemis.rbuilder.bundles_submitted").increment(1);
            }
            Err(e) => {
                warn!("rbuilder submission failed: {e}");
                metrics::counter!("artemis.rbuilder.submit_failed").increment(1);
                return Err(e);
            }
        }

        Ok(())
    }
}

// Stub implementations when rbuilder feature is disabled
#[cfg(not(feature = "rbuilder-integration"))]
pub struct RbuilderExecutor;

#[cfg(not(feature = "rbuilder-integration"))]
impl RbuilderExecutor {
    pub fn new(_config: ()) -> Self {
        Self
    }
}
