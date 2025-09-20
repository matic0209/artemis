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

    let block_collector = Box::new(BlockCollector::new(Arc::clone(&provider)));
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

    // TODO: Add Alloy Flashbots executor when Action::SubmitBundle is available

    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            info!("res: {:?}", res);
        }
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
                            .unwrap()
                    } else {
                        Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .body(Body::empty())
                            .unwrap()
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
