use anyhow::{Context, Result};
use artemis_core::collectors::block_collector::BlockCollector;
use artemis_core::collectors::opensea_order_collector::OpenseaOrderCollector;
use artemis_core::engine::Engine;
use artemis_core::eth::Address;
use artemis_core::types::{CollectorMap, ExecutorMap};
use clap::Parser;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Response, Server, StatusCode};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use opensea_sudo_arb::strategy::OpenseaSudoArb;
use opensea_sudo_arb::types::{Action, Config, Event};
use opensea_v2::client::{OpenSeaApiConfig, OpenSeaV2Client};
use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use alloy_provider::ProviderBuilder;
use alloy_transport_ws::WsConnect;
use artemis_core::eth::{helpers, LocalWallet};
use artemis_core::executors::mempool_alloy_executor::MempoolAlloyExecutor;
use artemis_core::executors::flashbots_alloy_executor::FlashbotsAlloyExecutor;

/// CLI Options.
#[derive(Parser, Debug)]
pub struct Args {
    /// Ethereum node WS endpoint.
    #[arg(long)]
    pub wss: String,

    /// Key for the OpenSea API.
    #[arg(long)]
    pub opensea_api_key: String,

    /// Private key for sending txs.
    #[arg(long)]
    pub private_key: String,

    /// Address of the arb contract.
    #[arg(long)]
    pub arb_contract_address: String,

    /// Percentage of profit to pay in gas.
    #[arg(long)]
    pub bid_percentage: u64,

    /// Block collector buffer size (default: 1024)
    #[arg(long, default_value = "1024")]
    pub block_buffer_size: usize,

    /// Mempool collector buffer size (default: 2048)
    #[arg(long, default_value = "2048")]
    pub mempool_buffer_size: usize,

    /// Minimum transaction value to process in wei (default: 0)
    #[arg(long, default_value = "0")]
    pub min_tx_value: u128,

    /// Enable rbuilder integration for advanced block building
    #[arg(long)]
    pub enable_rbuilder: bool,

    /// rbuilder JSON-RPC endpoint (default: http://localhost:8645)
    #[arg(long, default_value = "http://localhost:8645")]
    pub rbuilder_url: String,

    /// Block building algorithm (max-profit, mev-gas-price, type-max-profit)
    #[arg(long, default_value = "max-profit")]
    pub rbuilder_algorithm: String,

    /// Run performance benchmark instead of normal operation
    #[arg(long)]
    pub benchmark: bool,

    /// Number of benchmark iterations (default: 100)
    #[arg(long, default_value = "100")]
    pub benchmark_iterations: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing and parse args.
    let filter = filter::Targets::new()
        .with_target("opensea_sudo_arb", Level::INFO)
        .with_target("artemis_core", Level::INFO);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let metrics_handle = install_metrics_recorder()?;
    let metrics_addr = metrics_bind_addr()?;
    tokio::spawn(run_metrics_server(metrics_handle, metrics_addr));
    info!("metrics_server" = %metrics_addr, "metrics server started");

    let args = Args::parse();
    
    // Run benchmark if requested
    if args.benchmark {
        return run_benchmark(args).await;
    }
    
    run_cli(args).await
}

async fn run_cli(args: Args) -> Result<()> {
    let wallet: LocalWallet = helpers::parse_local_wallet(&args.private_key)?;
    let connect = WsConnect::new(&args.wss);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_ws(connect)
        .await
        .context("failed to connect alloy provider")?;
    let provider = Arc::new(provider);

    let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {
        api_key: args.opensea_api_key.clone(),
    });

    let mut engine: Engine<Event, Action> = Engine::default();

    // Configure block collector with custom buffer size
    let block_collector = BlockCollector::new(Arc::clone(&provider))
        .with_buffer_size(args.block_buffer_size);
    let block_collector = Box::new(block_collector);
    let block_collector = CollectorMap::new(block_collector, Event::NewBlock);
    engine.add_collector(Box::new(block_collector));

    let opensea_collector = Box::new(OpenseaOrderCollector::new(args.opensea_api_key));
    let opensea_collector =
        CollectorMap::new(opensea_collector, |e| Event::OpenseaOrder(Box::new(e)));
    engine.add_collector(Box::new(opensea_collector));

    let config = Config {
        arb_contract_address: Address::from_str(&args.arb_contract_address)?,
        bid_percentage: args.bid_percentage,
    };
    let strategy = OpenseaSudoArb::new(Arc::clone(&provider), opensea_client, config);
    engine.add_strategy(Box::new(strategy));

    let executor = Box::new(MempoolAlloyExecutor::new(Arc::clone(&provider)));
    let executor = ExecutorMap::new(executor, |action| match action {
        Action::SubmitTx(tx) => Some(tx),
    });
    engine.add_executor(Box::new(executor));

    // Add rbuilder executor if enabled
    #[cfg(feature = "rbuilder-integration")]
    if args.enable_rbuilder {
        use artemis_core::executors::rbuilder_executor::{RbuilderExecutor, RbuilderConfig, RbuilderBundle};
        
        let rbuilder_config = RbuilderConfig {
            rpc_url: args.rbuilder_url,
            sorting_algorithm: args.rbuilder_algorithm,
            relay_urls: vec!["https://boost-relay.flashbots.net".to_string()],
            enable_optimization: true,
        };
        
        let rbuilder_executor = Box::new(RbuilderExecutor::new(rbuilder_config));
        let rbuilder_executor = ExecutorMap::new(rbuilder_executor, |action| match action {
            Action::SubmitTx(tx) => {
                // Convert single tx to rbuilder bundle format
                Some(RbuilderBundle {
                    txs: vec!["0x".to_string()], // TODO: Convert tx to hex
                    target_block: None,
                    min_timestamp: None,
                    max_timestamp: None,
                    reverting_tx_hashes: vec![],
                    replacement_uuid: None,
                })
            }
        });
        engine.add_executor(Box::new(rbuilder_executor));
        info!("rbuilder executor enabled with algorithm: {}", args.rbuilder_algorithm);
    }

    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            info!("res: {:?}", res);
        }
    }

    Ok(())
}

async fn run_benchmark(args: Args) -> Result<()> {
    use artemis_core::benchmarks::PerformanceBenchmark;
    
    println!("🚀 Artemis Performance Benchmark Mode");
    println!("Setting up provider connection...");
    
    // Create provider for benchmarking
    let wallet: LocalWallet = helpers::parse_local_wallet(&args.private_key)?;
    let connect = WsConnect::new(&args.wss);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_ws(connect)
        .await
        .context("failed to connect provider for benchmark")?;
    let provider = Arc::new(provider);
    
    // Run benchmark
    let benchmark = PerformanceBenchmark::new(provider, args.benchmark_iterations);
    
    println!("Running RPC performance test...");
    let rpc_latency = benchmark.benchmark_rpc_calls().await?;
    
    println!("Running engine comparison...");
    let results = benchmark.run_comparison().await?;
    
    // Print comprehensive report
    benchmark.print_report(&results);
    
    println!("\n🌐 RPC Performance: {:.2}ms for 10 concurrent calls", rpc_latency.as_millis());
    
    // Recommendations based on results
    if results.improvement_percentage > 30.0 {
        println!("\n🎯 RECOMMENDATION: Enable optimizations for production use!");
        println!("Add --features rbuilder-integration to your build command");
    }
    
    Ok(())
}

fn install_metrics_recorder() -> Result<PrometheusHandle> {
    PrometheusBuilder::new()
        .install_recorder()
        .context("failed to install prometheus recorder")
}

fn metrics_bind_addr() -> Result<SocketAddr> {
    let bind = env::var("ARTEMIS_METRICS_ADDR").unwrap_or_else(|_| "127.0.0.1:9898".to_string());
    bind.parse::<SocketAddr>()
        .context("invalid ARTEMIS_METRICS_ADDR value")
}

async fn run_metrics_server(handle: PrometheusHandle, addr: SocketAddr) {
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

    if let Err(err) = Server::bind(&addr).serve(make_svc).await {
        tracing::error!(error = %err, "metrics server failed");
    }
}
