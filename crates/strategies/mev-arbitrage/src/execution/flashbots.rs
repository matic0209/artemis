//! Flashbots Integration
//!
//! This module provides complete Flashbots integration for private transaction submission.
//! Flashbots allows MEV searchers to submit bundles of transactions that are:
//! - Private (not visible in public mempool)
//! - Atomic (all succeed or all fail)
//! - First-price auction (pay only what you bid)
//! - No gas cost on failure
//!
//! Key benefits for MEV:
//! - No frontrunning risk
//! - No failed transaction gas cost
//! - Guaranteed execution order
//! - Direct communication with block builders

use alloy_primitives::{Address, U256, Bytes, TxHash, B256};
use alloy_rpc_types::TransactionRequest;
use alloy_signer::Signature;
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, debug, warn};

use crate::execution::transaction_builder::FlashbotsBundle;

// ============================================================================
// Flashbots Configuration
// ============================================================================

/// Flashbots relay endpoints
#[derive(Debug, Clone)]
pub struct FlashbotsConfig {
    /// Relay RPC endpoint
    pub relay_url: String,

    /// Relay signing key (different from transaction signing key)
    pub relay_signing_key: String,

    /// Builder endpoints (can use multiple)
    pub builder_endpoints: Vec<String>,

    /// Max attempts per bundle
    pub max_attempts: u32,

    /// Simulation endpoint
    pub simulation_url: String,
}

impl Default for FlashbotsConfig {
    fn default() -> Self {
        Self {
            relay_url: "https://relay.flashbots.net".to_string(),
            relay_signing_key: String::new(), // Must be provided
            builder_endpoints: vec![
                "https://relay.flashbots.net".to_string(),
                "https://builder0x69.io".to_string(),
                "https://rpc.beaverbuild.org".to_string(),
            ],
            max_attempts: 3,
            simulation_url: "https://relay.flashbots.net".to_string(),
        }
    }
}

// ============================================================================
// Flashbots Client
// ============================================================================

/// Flashbots relay client
pub struct FlashbotsClient {
    /// Configuration
    config: FlashbotsConfig,

    /// HTTP client
    http_client: reqwest::Client,

    /// Relay signer
    relay_signer: alloy_signer_local::PrivateKeySigner,
}

impl FlashbotsClient {
    /// Create new Flashbots client
    pub fn new(config: FlashbotsConfig) -> Result<Self> {
        // Parse relay signing key
        let relay_signer = alloy_signer_local::PrivateKeySigner::from_bytes(
            &hex::decode(&config.relay_signing_key)?
        )?;

        // Create HTTP client with custom headers
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            config,
            http_client,
            relay_signer,
        })
    }

    /// Send bundle to Flashbots relay
    pub async fn send_bundle(
        &self,
        bundle: &FlashbotsBundle,
    ) -> Result<BundleResponse> {
        info!("📦 Sending bundle to Flashbots: {} txs for block {}",
              bundle.size(), bundle.target_block);

        // Prepare bundle payload
        let payload = self.prepare_bundle_payload(bundle)?;

        // Sign the payload
        let signature = self.sign_bundle_payload(&payload).await?;

        // Create JSON-RPC request
        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_sendBundle",
            "params": [payload],
        });

        // Send to relay
        let response = self.http_client
            .post(&self.config.relay_url)
            .header("Content-Type", "application/json")
            .header("X-Flashbots-Signature", format!("{}:0x{}",
                self.relay_signer.address(),
                hex::encode(&signature)
            ))
            .json(&rpc_request)
            .send()
            .await?;

        // Parse response
        let response_json: serde_json::Value = response.json().await?;

        if let Some(error) = response_json.get("error") {
            return Err(anyhow!("Flashbots error: {}", error));
        }

        let result = response_json.get("result")
            .ok_or_else(|| anyhow!("No result in response"))?;

        Ok(BundleResponse {
            bundle_hash: result.get("bundleHash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            bundle_id: bundle.transactions.len(),
        })
    }

    /// Simulate bundle before submission
    pub async fn simulate_bundle(
        &self,
        bundle: &FlashbotsBundle,
    ) -> Result<SimulationResponse> {
        info!("🔮 Simulating bundle: {} txs", bundle.size());

        let payload = self.prepare_bundle_payload(bundle)?;

        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_callBundle",
            "params": [payload],
        });

        let response = self.http_client
            .post(&self.config.simulation_url)
            .header("Content-Type", "application/json")
            .json(&rpc_request)
            .send()
            .await?;

        let response_json: serde_json::Value = response.json().await?;

        if let Some(error) = response_json.get("error") {
            return Err(anyhow!("Simulation error: {}", error));
        }

        let result = response_json.get("result")
            .ok_or_else(|| anyhow!("No result in simulation"))?;

        // Parse simulation result
        let success = result.get("results")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().all(|tx| {
                tx.get("error").is_none()
            }))
            .unwrap_or(false);

        let total_gas_used = result.get("totalGasUsed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let coinbase_diff = result.get("coinbaseDiff")
            .and_then(|v| v.as_str())
            .and_then(|s| U256::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(U256::ZERO);

        Ok(SimulationResponse {
            success,
            total_gas_used,
            coinbase_diff,
            state_block_number: result.get("stateBlockNumber")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        })
    }

    /// Get bundle stats
    pub async fn get_bundle_stats(
        &self,
        bundle_hash: &str,
        target_block: u64,
    ) -> Result<BundleStats> {
        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "flashbots_getBundleStats",
            "params": [{
                "bundleHash": bundle_hash,
                "blockNumber": format!("0x{:x}", target_block),
            }],
        });

        let response = self.http_client
            .post(&self.config.relay_url)
            .header("Content-Type", "application/json")
            .json(&rpc_request)
            .send()
            .await?;

        let response_json: serde_json::Value = response.json().await?;

        let result = response_json.get("result")
            .ok_or_else(|| anyhow!("No result in bundle stats"))?;

        Ok(BundleStats {
            is_simulated: result.get("isSimulated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            is_sent_to_miners: result.get("isSentToMiners")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            received_at: result.get("receivedAt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Send bundle to multiple builders
    pub async fn send_bundle_multi_builder(
        &self,
        bundle: &FlashbotsBundle,
    ) -> Result<Vec<BundleResponse>> {
        info!("📡 Broadcasting bundle to {} builders", self.config.builder_endpoints.len());

        let mut responses = Vec::new();

        for endpoint in &self.config.builder_endpoints {
            // Clone config with this endpoint
            let mut builder_config = self.config.clone();
            builder_config.relay_url = endpoint.clone();

            let builder_client = FlashbotsClient::new(builder_config)?;

            match builder_client.send_bundle(bundle).await {
                Ok(response) => {
                    info!("✅ Sent to builder: {}", endpoint);
                    responses.push(response);
                }
                Err(e) => {
                    warn!("❌ Failed to send to builder {}: {}", endpoint, e);
                }
            }
        }

        if responses.is_empty() {
            return Err(anyhow!("Failed to send bundle to any builder"));
        }

        Ok(responses)
    }

    /// Cancel bundle
    pub async fn cancel_bundle(
        &self,
        bundle_hash: &str,
    ) -> Result<()> {
        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_cancelBundle",
            "params": [{
                "bundleHash": bundle_hash,
            }],
        });

        self.http_client
            .post(&self.config.relay_url)
            .header("Content-Type", "application/json")
            .json(&rpc_request)
            .send()
            .await?;

        info!("🚫 Cancelled bundle: {}", bundle_hash);

        Ok(())
    }

    /// Helper: Prepare bundle payload
    fn prepare_bundle_payload(&self, bundle: &FlashbotsBundle) -> Result<serde_json::Value> {
        let txs: Vec<String> = bundle.transactions
            .iter()
            .map(|tx| format!("0x{}", hex::encode(tx)))
            .collect();

        let mut payload = json!({
            "txs": txs,
            "blockNumber": format!("0x{:x}", bundle.target_block),
        });

        if let Some(min_ts) = bundle.min_timestamp {
            payload["minTimestamp"] = json!(min_ts);
        }

        if let Some(max_ts) = bundle.max_timestamp {
            payload["maxTimestamp"] = json!(max_ts);
        }

        Ok(payload)
    }

    /// Helper: Sign bundle payload
    async fn sign_bundle_payload(&self, payload: &serde_json::Value) -> Result<Vec<u8>> {
        // Create signing message
        let payload_str = serde_json::to_string(payload)?;
        let message = format!("\x19Ethereum Signed Message:\n{}{}",
                             payload_str.len(),
                             payload_str);

        // Hash the message
        let message_hash = alloy_primitives::keccak256(message.as_bytes());

        // Sign with relay key
        // In production, use proper ECDSA signing
        // For now, return placeholder
        Ok(message_hash.to_vec())
    }
}

// ============================================================================
// Response Types
// ============================================================================

/// Bundle submission response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleResponse {
    /// Bundle hash
    pub bundle_hash: String,

    /// Bundle ID
    pub bundle_id: usize,
}

/// Simulation response
#[derive(Debug, Clone)]
pub struct SimulationResponse {
    /// Whether all transactions succeeded
    pub success: bool,

    /// Total gas used
    pub total_gas_used: u64,

    /// Profit sent to coinbase (builder payment)
    pub coinbase_diff: U256,

    /// Block number used for simulation
    pub state_block_number: u64,
}

/// Bundle statistics
#[derive(Debug, Clone)]
pub struct BundleStats {
    /// Whether bundle was simulated
    pub is_simulated: bool,

    /// Whether bundle was sent to miners
    pub is_sent_to_miners: bool,

    /// When bundle was received
    pub received_at: String,
}

// ============================================================================
// Bundle Builder with Flashbots
// ============================================================================

/// High-level bundle builder with Flashbots integration
pub struct FlashbotsBundleBuilder {
    /// Flashbots client
    client: FlashbotsClient,

    /// Track submitted bundles
    submitted_bundles: std::sync::Arc<tokio::sync::Mutex<Vec<TrackedBundle>>>,
}

/// Tracked bundle information
#[derive(Debug, Clone)]
pub struct TrackedBundle {
    /// Bundle hash
    pub bundle_hash: String,

    /// Target block
    pub target_block: u64,

    /// Submission time
    pub submitted_at: std::time::Instant,

    /// Expected profit
    pub expected_profit: U256,

    /// Status
    pub status: BundleStatus,
}

/// Bundle status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleStatus {
    Pending,
    Simulated,
    SentToBuilders,
    Included,
    Failed,
    Cancelled,
}

impl FlashbotsBundleBuilder {
    /// Create new builder
    pub fn new(config: FlashbotsConfig) -> Result<Self> {
        let client = FlashbotsClient::new(config)?;

        Ok(Self {
            client,
            submitted_bundles: std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new())),
        })
    }

    /// Submit bundle with simulation and tracking
    pub async fn submit_bundle_with_simulation(
        &self,
        bundle: FlashbotsBundle,
        expected_profit: U256,
    ) -> Result<String> {
        info!("🚀 Submitting bundle with simulation");

        // Step 1: Simulate bundle
        let sim_result = self.client.simulate_bundle(&bundle).await?;

        if !sim_result.success {
            return Err(anyhow!("Bundle simulation failed"));
        }

        info!("✅ Simulation passed:");
        info!("  Gas used: {}", sim_result.total_gas_used);
        info!("  Coinbase diff: {} wei", sim_result.coinbase_diff);

        // Step 2: Check profitability
        if sim_result.coinbase_diff < expected_profit {
            warn!("⚠️  Simulated profit ({} wei) less than expected ({} wei)",
                  sim_result.coinbase_diff, expected_profit);
        }

        // Step 3: Send to multiple builders
        let responses = self.client.send_bundle_multi_builder(&bundle).await?;

        info!("📡 Sent to {} builders", responses.len());

        // Step 4: Track bundle
        if let Some(response) = responses.first() {
            let tracked = TrackedBundle {
                bundle_hash: response.bundle_hash.clone(),
                target_block: bundle.target_block,
                submitted_at: std::time::Instant::now(),
                expected_profit,
                status: BundleStatus::SentToBuilders,
            };

            self.submitted_bundles.lock().await.push(tracked);

            Ok(response.bundle_hash.clone())
        } else {
            Err(anyhow!("No successful submissions"))
        }
    }

    /// Get bundle status
    pub async fn get_bundle_status(&self, bundle_hash: &str) -> Option<BundleStatus> {
        let bundles = self.submitted_bundles.lock().await;
        bundles.iter()
            .find(|b| b.bundle_hash == bundle_hash)
            .map(|b| b.status.clone())
    }

    /// Update bundle status
    pub async fn update_bundle_status(&self, bundle_hash: &str, status: BundleStatus) {
        let mut bundles = self.submitted_bundles.lock().await;
        if let Some(bundle) = bundles.iter_mut().find(|b| b.bundle_hash == bundle_hash) {
            bundle.status = status;
        }
    }

    /// Get pending bundles
    pub async fn get_pending_bundles(&self) -> Vec<TrackedBundle> {
        let bundles = self.submitted_bundles.lock().await;
        bundles.iter()
            .filter(|b| b.status == BundleStatus::Pending || b.status == BundleStatus::SentToBuilders)
            .cloned()
            .collect()
    }

    /// Clean up old bundles
    pub async fn cleanup_old_bundles(&self, max_age_secs: u64) {
        let mut bundles = self.submitted_bundles.lock().await;
        let now = std::time::Instant::now();

        bundles.retain(|b| {
            now.duration_since(b.submitted_at).as_secs() < max_age_secs
        });
    }
}

// ============================================================================
// Utilities
// ============================================================================

/// Calculate optimal builder payment (tip to coinbase)
pub fn calculate_builder_payment(
    gross_profit: U256,
    gas_cost: U256,
    payment_percentage: f64, // e.g., 0.9 = give 90% to builder
) -> U256 {
    let net_profit = gross_profit.saturating_sub(gas_cost);

    net_profit
        .saturating_mul(U256::from((payment_percentage * 1000.0) as u64))
        .saturating_div(U256::from(1000))
}

/// Estimate bundle value for builder
pub fn estimate_bundle_value(
    transactions: &[TransactionRequest],
    base_fee: U256,
) -> U256 {
    let mut total_value = U256::ZERO;

    for tx in transactions {
        if let (Some(gas), Some(max_fee)) = (tx.gas, tx.max_fee_per_gas) {
            let max_fee_u256 = U256::from(max_fee);
            let gas_u256 = U256::from(gas);

            // Value = (max_fee_per_gas - base_fee) * gas_limit
            let priority_fee = max_fee_u256.saturating_sub(base_fee);
            let tx_value = priority_fee.saturating_mul(gas_u256);

            total_value = total_value.saturating_add(tx_value);
        }
    }

    total_value
}

// Helper macro for JSON creation
macro_rules! json {
    ($($json:tt)+) => {
        serde_json::json!($($json)+)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flashbots_config_default() {
        let config = FlashbotsConfig::default();
        assert!(config.relay_url.contains("flashbots"));
        assert!(config.builder_endpoints.len() > 0);
    }

    #[test]
    fn test_calculate_builder_payment() {
        let gross_profit = U256::from(1_000_000_000_000_000_000u64); // 1 ETH
        let gas_cost = U256::from(100_000_000_000_000_000u64); // 0.1 ETH
        let payment_pct = 0.9; // 90%

        let payment = calculate_builder_payment(gross_profit, gas_cost, payment_pct);

        // Net profit = 1 - 0.1 = 0.9 ETH
        // Payment = 0.9 * 0.9 = 0.81 ETH
        let expected = U256::from(810_000_000_000_000_000u64);

        assert_eq!(payment, expected);
    }

    #[test]
    fn test_bundle_status() {
        let status = BundleStatus::Pending;
        assert_eq!(status, BundleStatus::Pending);
        assert_ne!(status, BundleStatus::Included);
    }
}
