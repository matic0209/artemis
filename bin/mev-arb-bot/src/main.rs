//! MEV Arbitrage Bot
//!
//! Complete bot that integrates:
//! - Artemis core framework (Collectors, Engine, Executors)
//! - MEV arbitrage strategy (detectors, validators, optimizers)
//! - Z3 symbolic execution
//! - REVM validation
//! - petgraph for graph theory
//! - Flashbots integration

use std::sync::Arc;
use std::str::FromStr;
use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn, Level};
use tracing_subscriber::{filter, prelude::*};

use alloy_primitives::Address;
use alloy_provider::ProviderBuilder;
use alloy_transport_ws::WsConnect;

use artemis_core::{
    collectors::block_collector::BlockCollector,
    engine::Engine,
    executors::flashbots_alloy_executor::FlashbotsAlloyExecutor,
    eth::{helpers, LocalWallet},
    types::{CollectorMap, ExecutorMap},
};

use mev_arbitrage::strategy::{ArbitrageStrategy, ArbitrageConfig, ArbitrageEvent, ArbitrageAction};

/// CLI arguments
#[derive(Parser, Debug)]
#[command(name = "mev-arb-bot")]
#[command(about = "MEV Arbitrage Bot with Z3, REVM, and Graph Theory", long_about = None)]
pub struct Args {
    /// Ethereum node WebSocket endpoint
    #[arg(long)]
    pub wss: String,

    /// Private key for signing transactions
    #[arg(long)]
    pub private_key: String,

    /// Flashbots signer private key
    #[arg(long)]
    pub flashbots_key: String,

    /// Minimum profit threshold in ETH
    #[arg(long, default_value = "0.1")]
    pub min_profit_eth: f64,

    /// Maximum gas price in gwei
    #[arg(long, default_value = "100")]
    pub max_gas_gwei: u64,

    /// Enable fast detector
    #[arg(long, default_value = "true")]
    pub enable_fast_detector: bool,

    /// Enable symbolic detector (Z3)
    #[arg(long, default_value = "true")]
    pub enable_symbolic_detector: bool,

    /// Maximum arbitrage path hops
    #[arg(long, default_value = "3")]
    pub max_hops: usize,

    /// Metrics server address
    #[arg(long, default_value = "127.0.0.1:9898")]
    pub metrics_addr: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let filter = filter::Targets::new()
        .with_target("mev_arb_bot", Level::INFO)
        .with_target("mev_arbitrage", Level::INFO)
        .with_target("artemis_core", Level::INFO);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    info!("🚀 MEV Arbitrage Bot starting...");

    // Parse CLI args
    let args = Args::parse();

    // Display configuration
    info!("Configuration:");
    info!("  Min profit: {} ETH", args.min_profit_eth);
    info!("  Max gas: {} gwei", args.max_gas_gwei);
    info!("  Fast detector: {}", args.enable_fast_detector);
    info!("  Symbolic detector (Z3): {}", args.enable_symbolic_detector);
    info!("  Max hops: {}", args.max_hops);

    // Create provider
    info!("📡 Connecting to Ethereum node: {}", args.wss);
    let wallet: LocalWallet = helpers::parse_local_wallet(&args.private_key)?;
    let connect = WsConnect::new(&args.wss);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_ws(connect)
        .await
        .context("failed to connect to Ethereum node")?;
    let provider = Arc::new(provider);

    info!("✅ Connected to Ethereum");

    // Create arbitrage strategy config
    let strategy_config = ArbitrageConfig {
        min_profit_wei: alloy_primitives::U256::from((args.min_profit_eth * 1e18) as u64),
        max_gas_price_gwei: args.max_gas_gwei,
        enable_fast_detector: args.enable_fast_detector,
        enable_symbolic_detector: args.enable_symbolic_detector,
        dex_routers: vec![
            // TODO: Add actual DEX router addresses
            // Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")?, // Uniswap V2 Router
            // Address::from_str("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F")?, // Sushiswap Router
        ],
        max_hops: args.max_hops,
    };

    // Create strategy
    info!("🔧 Initializing arbitrage strategy...");
    let mut strategy = ArbitrageStrategy::new(provider.clone(), strategy_config);

    // Sync strategy state (loads pools, builds graph, etc.)
    info!("🔄 Syncing strategy state (this may take a minute)...");
    // TODO: sync_state is a Strategy trait method, will be called by Engine
    // strategy.sync_state().await?;

    // Create Engine
    info!("⚙️  Building Artemis Engine...");
    let mut engine: Engine<ArbitrageEvent, ArbitrageAction> = Engine::new()
        .with_event_channel_capacity(1024)
        .with_action_channel_capacity(512);

    // Add Block Collector
    info!("📊 Adding block collector...");
    let block_collector = BlockCollector::new(provider.clone()).with_buffer_size(1024);
    let block_collector = Box::new(block_collector);
    let block_collector = CollectorMap::new(block_collector, ArbitrageEvent::NewBlock);
    engine.add_collector(Box::new(block_collector));

    // TODO: Add Mempool Collector for sandwich detection
    // let mempool_collector = MempoolCollector::new(provider.clone());
    // engine.add_collector(Box::new(mempool_collector));

    // Add Strategy
    info!("🧠 Adding arbitrage strategy...");
    engine.add_strategy(Box::new(strategy));

    // Add Flashbots Executor
    info!("⚡ Adding Flashbots executor...");
    use alloy_mev::Endpoints;

    // Create Flashbots endpoints
    let flashbots_endpoints = Endpoints::default();

    let flashbots_executor = Box::new(FlashbotsAlloyExecutor::new(
        provider.clone(),
        flashbots_endpoints,
    ));
    let flashbots_executor = ExecutorMap::new(flashbots_executor, |action| match action {
        ArbitrageAction::SubmitFlashbotsBundle {
            txs,
            target_block,
            min_timestamp,
            max_timestamp,
        } => {
            // Convert to Flashbots bundle format
            Some(artemis_core::executors::flashbots_alloy_executor::FlashbotsAlloyBundle {
                txs: vec![], // TODO: Convert Vec<u8> to Vec<TxRequest>
                target_block: Some(target_block),
                min_timestamp,
                max_timestamp,
                replacement_uuid: None,
                reverting_hashes: vec![],
            })
        }
        ArbitrageAction::SubmitToMempool { .. } => {
            // TODO: Add mempool executor
            None
        }
    });
    engine.add_executor(Box::new(flashbots_executor));

    // Start metrics server
    info!("📈 Starting metrics server on {}", args.metrics_addr);
    tokio::spawn(run_metrics_server(args.metrics_addr));

    // Run the engine
    info!("🎯 Starting MEV bot engine...");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("Bot is now running and monitoring for opportunities!");
    info!("Press Ctrl+C to stop");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            match res {
                Ok(_) => {}
                Err(e) => {
                    warn!("Task failed: {:?}", e);
                }
            }
        }
    }

    Ok(())
}

/// Run Prometheus metrics server
async fn run_metrics_server(addr: String) {
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Response, Server, StatusCode};
    use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
    use std::convert::Infallible;
    use std::net::SocketAddr;

    let handle = match PrometheusBuilder::new().install_recorder() {
        Ok(h) => h,
        Err(e) => {
            warn!("Failed to install metrics recorder: {}", e);
            return;
        }
    };

    let addr: SocketAddr = match addr.parse() {
        Ok(a) => a,
        Err(e) => {
            warn!("Invalid metrics address: {}", e);
            return;
        }
    };

    let make_svc = make_service_fn(move |_conn| {
        let handle = handle.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let handle = handle.clone();
                async move {
                    let response = if req.uri().path() == "/metrics" {
                        let body = handle.render();
                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/plain; version=0.0.4")
                            .body(Body::from(body))
                            .expect("Failed to build metrics response")
                    } else {
                        Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .body(Body::empty())
                            .expect("Failed to build 404 response")
                    };
                    Ok::<_, Infallible>(response)
                }
            }))
        }
    });

    if let Err(e) = Server::bind(&addr).serve(make_svc).await {
        warn!("Metrics server error: {}", e);
    }
}
