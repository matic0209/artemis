//! Local Node MEV Arbitrage Bot
//!
//! 连接到本地 Reth + Lighthouse 节点的完整 MEV 套利机器人
//!
//! ## 使用方法
//!
//! ```bash
//! # 1. 确保 Reth 和 Lighthouse 正在运行
//! # 2. 创建配置文件: config/local_node.toml
//! # 3. 设置环境变量: export PRIVATE_KEY="0x..."
//! # 4. 运行:
//! cargo run --bin artemis-local-node-bot -- --config config/local_node.toml
//! ```

use anyhow::{Result, Context};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, error, debug};
use clap::Parser;

use mev_arbitrage::{
    // Core components
    UnifiedArbitrageManager,
    ManagerConfig,
    event_coordinator::{EventCoordinator, CoordinatorConfig},

    // Event system
    MEVEvent,

    // Abstractions
    DetectionContext,
    MarketData,
    DetectionParams,
};

use mev_arbitrage_graph::StateSnapshot;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_transport_ws::WsConnect;
use alloy_rpc_types_eth::{Block, Transaction};
use alloy_primitives::{Address, U256};

#[derive(Parser)]
#[command(name = "artemis-local-node-bot")]
#[command(about = "MEV Arbitrage Bot for Local Reth + Lighthouse Node")]
#[command(version = "0.1.0")]
struct Args {
    /// Config file path
    #[arg(short, long, default_value = "config/local_node.toml")]
    config: String,

    /// Enable dry run (no real transactions)
    #[arg(long)]
    dry_run: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Enable metrics server
    #[arg(long)]
    metrics: bool,

    /// Metrics port
    #[arg(long, default_value = "9090")]
    metrics_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    init_logging(&args.log_level)?;

    info!("🚀 Starting Local Node MEV Arbitrage Bot");
    info!("📁 Config: {}", args.config);
    info!("🔧 Dry run: {}", args.dry_run);

    // Load configuration
    let config = Config::load(&args.config)
        .context("Failed to load configuration")?;

    info!("✅ Configuration loaded successfully");
    info!("   Node: {}", config.node.ws_url);
    info!("   Chain ID: {}", config.node.chain_id);
    info!("   Strategy: {}", config.strategy.strategy_type);

    // Connect to local node
    info!("🔌 Connecting to local node...");
    let provider = connect_to_node(&config.node.ws_url).await
        .context("Failed to connect to local node")?;

    // Verify connection
    let block_number = provider.get_block_number().await?;
    let chain_id = provider.get_chain_id().await?;

    info!("✅ Connected to local node");
    info!("   Current block: {}", block_number);
    info!("   Chain ID: {}", chain_id);

    if chain_id != config.node.chain_id {
        warn!("⚠️  Chain ID mismatch! Config: {}, Node: {}",
              config.node.chain_id, chain_id);
    }

    // Initialize MEV arbitrage system
    info!("🏗️  Initializing MEV arbitrage system...");

    let manager_config = ManagerConfig {
        enable_fast_detector: config.detection.enable_fast_detector,
        enable_deep_detector: config.detection.enable_deep_detector,
        enable_parallel_detection: true,
        detection_timeout: std::time::Duration::from_secs(config.detection.fast.timeout),
        validation_timeout: std::time::Duration::from_secs(config.validation.validation_timeout),
        max_concurrent_detections: 10,
    };

    let manager = UnifiedArbitrageManager::new(manager_config);
    let manager_arc = Arc::new(RwLock::new(manager));

    info!("✅ MEV arbitrage manager created");

    // Create event coordinator
    let coordinator_config = CoordinatorConfig {
        max_buffer_size: 1000,
        event_timeout: std::time::Duration::from_secs(5),
        enable_batching: true,
        batch_size: 10,
        batch_timeout: std::time::Duration::from_millis(100),
        enable_metrics: args.metrics,
    };

    let (coordinator, event_sender) = EventCoordinator::new(
        coordinator_config,
        manager_arc.clone()
    );

    info!("✅ Event coordinator created");

    // Start event coordinator in background
    tokio::spawn(async move {
        info!("🔄 Event coordinator started");
        if let Err(e) = coordinator.run().await {
            error!("❌ Event coordinator error: {}", e);
        }
    });

    // Start metrics server if enabled
    if args.metrics {
        let metrics_port = args.metrics_port;
        tokio::spawn(async move {
            if let Err(e) = start_metrics_server(metrics_port).await {
                error!("❌ Metrics server error: {}", e);
            }
        });
        info!("📊 Metrics server started on port {}", args.metrics_port);
    }

    // Create and start block listener
    let listener = BlockListener::new(
        provider,
        event_sender,
        config.clone(),
        args.dry_run,
    );

    info!("🎯 Starting block listener...");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🤖 MEV Arbitrage Bot is now running!");
    info!("   Listening for new blocks...");
    info!("   Press Ctrl+C to stop");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Run the listener (this blocks until error or shutdown)
    listener.start().await?;

    Ok(())
}

/// Block listener - monitors new blocks and sends events to the coordinator
struct BlockListener<P> {
    provider: Arc<P>,
    event_sender: mpsc::Sender<MEVEvent>,
    config: Config,
    dry_run: bool,
    stats: Arc<RwLock<Stats>>,
}

#[derive(Default)]
struct Stats {
    blocks_processed: u64,
    transactions_processed: u64,
    opportunities_found: u64,
    errors: u64,
}

impl<P: Provider> BlockListener<P> {
    fn new(
        provider: Arc<P>,
        event_sender: mpsc::Sender<MEVEvent>,
        config: Config,
        dry_run: bool,
    ) -> Self {
        Self {
            provider,
            event_sender,
            config,
            dry_run,
            stats: Arc::new(RwLock::new(Stats::default())),
        }
    }

    async fn start(&self) -> Result<()> {
        // Subscribe to new blocks via WebSocket
        let sub = self.provider.subscribe_blocks().await
            .context("Failed to subscribe to blocks")?;

        let mut stream = sub.into_stream();

        info!("✅ Subscribed to new blocks");

        // Start stats reporter
        let stats_clone = self.stats.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                let stats = stats_clone.read().await;
                info!("📊 Stats (last 60s): blocks={}, txs={}, opportunities={}, errors={}",
                      stats.blocks_processed,
                      stats.transactions_processed,
                      stats.opportunities_found,
                      stats.errors);
            }
        });

        // Process incoming blocks
        while let Some(block) = stream.next().await {
            if let Err(e) = self.process_block(&block).await {
                error!("❌ Error processing block: {}", e);
                let mut stats = self.stats.write().await;
                stats.errors += 1;
            }
        }

        warn!("⚠️  Block stream ended");
        Ok(())
    }

    async fn process_block(&self, block: &Block) -> Result<()> {
        let block_number = block.header.number
            .context("Block number missing")?;

        let timestamp = block.header.timestamp;

        let base_fee = block.header.base_fee_per_gas
            .unwrap_or(U256::ZERO);

        debug!("📦 New block: #{} (timestamp: {}, base_fee: {} gwei)",
               block_number,
               timestamp,
               base_fee / U256::from(1_000_000_000u64));

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.blocks_processed += 1;
        }

        // Send NewBlock event to coordinator
        let block_hash = block.header.hash
            .context("Block hash missing")?;

        let event = MEVEvent::NewBlock {
            block_number,
            block_hash,
            timestamp,
            base_fee,
        };

        if let Err(e) = self.event_sender.send(event).await {
            error!("❌ Failed to send block event: {}", e);
        }

        // Process transactions in the block
        let tx_count = block.transactions.len();
        if tx_count > 0 {
            debug!("   Processing {} transactions", tx_count);

            for tx in &block.transactions {
                if let Err(e) = self.process_transaction(tx, block_number).await {
                    debug!("   Error processing transaction: {}", e);
                }
            }

            let mut stats = self.stats.write().await;
            stats.transactions_processed += tx_count as u64;
        }

        Ok(())
    }

    async fn process_transaction(&self, tx: &Transaction, block_number: u64) -> Result<()> {
        // Extract transaction details
        let tx_hash = tx.hash;
        let from = tx.from;
        let to = tx.to;
        let value = tx.value;
        let gas_price = tx.gas_price.unwrap_or(U256::ZERO);

        // Filter: only process transactions to known DEX contracts
        if let Some(to_addr) = to {
            if !self.is_interesting_transaction(&to_addr, &value) {
                return Ok(());
            }

            debug!("   🔍 Interesting tx: {} (to: {:?}, value: {} ETH)",
                   tx_hash,
                   to_addr,
                   value.to_string().parse::<f64>().unwrap_or(0.0) / 1e18);

            // Send NewTransaction event
            let event = MEVEvent::NewTransaction {
                tx_hash,
                from,
                to,
                value,
                gas_price,
                block_number,
            };

            self.event_sender.send(event).await?;
        }

        Ok(())
    }

    /// Check if transaction is interesting (involves DEXes)
    fn is_interesting_transaction(&self, to: &Address, value: &U256) -> bool {
        // Filter 1: Must have significant value (> 0.01 ETH)
        if *value < U256::from(10_000_000_000_000_000u64) {
            return false;
        }

        // Filter 2: Check if it's a known DEX contract
        let to_str = format!("{:?}", to).to_lowercase();

        // Known DEX routers (add more as needed)
        let known_dexes = [
            &self.config.pools.uniswap_v2_router,
            &self.config.pools.uniswap_v3_router,
            &self.config.pools.sushiswap_router,
        ];

        known_dexes.iter().any(|dex| {
            let dex_str = format!("{:?}", dex).to_lowercase();
            to_str.contains(&dex_str) || dex_str.contains(&to_str)
        })
    }
}

/// Configuration structures
#[derive(Debug, Clone, serde::Deserialize)]
struct Config {
    node: NodeConfig,
    wallet: WalletConfig,
    strategy: StrategyConfig,
    detection: DetectionConfig,
    validation: ValidationConfig,
    execution: ExecutionConfig,
    monitoring: MonitoringConfig,
    safety: SafetyConfig,
    pools: PoolsConfig,
    tokens: TokensConfig,
}

impl Config {
    fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context(format!("Failed to read config file: {}", path))?;

        let config: Config = toml::from_str(&content)
            .context("Failed to parse config file")?;

        Ok(config)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct NodeConfig {
    http_url: String,
    ws_url: String,
    beacon_url: Option<String>,
    chain_id: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct WalletConfig {
    private_key_env: String,
    address: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StrategyConfig {
    strategy_type: String,
    min_profit_threshold: String,
    max_investment: String,
    max_gas_price: u64,
    gas_price_multiplier: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct DetectionConfig {
    enable_fast_detector: bool,
    enable_deep_detector: bool,
    fast: FastDetectorConfig,
    deep: DeepDetectorConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FastDetectorConfig {
    min_profit_threshold: String,
    max_gas_cost: u64,
    confidence_threshold: f64,
    max_opportunities: usize,
    timeout: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct DeepDetectorConfig {
    min_profit_threshold: String,
    max_path_length: usize,
    confidence_threshold: f64,
    max_opportunities: usize,
    timeout: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ValidationConfig {
    enable_symbolic: bool,
    enable_revm: bool,
    validation_timeout: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ExecutionConfig {
    execution_mode: String,
    slippage_tolerance: f64,
    max_retries: u32,
    execution_timeout: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct MonitoringConfig {
    enable_metrics: bool,
    metrics_port: u16,
    enable_health_check: bool,
    health_check_port: u16,
    log_level: String,
    log_format: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SafetyConfig {
    enable_safety_mode: bool,
    dry_run: bool,
    max_daily_loss: String,
    max_single_trade: String,
    enable_emergency_stop: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PoolsConfig {
    dexes: Vec<String>,
    uniswap_v2_router: String,
    uniswap_v2_factory: String,
    uniswap_v3_router: String,
    uniswap_v3_factory: String,
    sushiswap_router: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TokensConfig {
    watched_tokens: Vec<String>,
    symbols: HashMap<String, String>,
}

/// Initialize logging with colored output
fn init_logging(log_level: &str) -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            EnvFilter::new(format!(
                "{},mev_arbitrage=debug,local_node_bot=debug",
                log_level
            ))
        });

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .pretty()
        )
        .init();

    Ok(())
}

/// Connect to local node via WebSocket
async fn connect_to_node(ws_url: &str) -> Result<Arc<impl Provider>> {
    info!("   Connecting to: {}", ws_url);

    let ws = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new()
        .on_ws(ws)
        .await
        .context("Failed to create WebSocket provider")?;

    Ok(Arc::new(provider))
}

/// Start metrics server (simple HTTP server)
async fn start_metrics_server(port: u16) -> Result<()> {
    use std::net::SocketAddr;
    use tokio::net::TcpListener;

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    info!("📊 Metrics server listening on http://{}", addr);

    loop {
        let (socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            // Simple HTTP response with metrics
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n\
                           # MEV Arbitrage Metrics\n\
                           mev_blocks_processed 0\n\
                           mev_opportunities_found 0\n\
                           mev_transactions_executed 0\n";

            if let Err(e) = socket.try_write(response.as_bytes()) {
                debug!("Failed to write metrics response: {}", e);
            }
        });
    }
}