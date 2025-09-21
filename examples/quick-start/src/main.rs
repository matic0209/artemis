use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

use artemis_core::{
    collectors::block_collector::BlockCollector,
    collectors::opensea_order_collector::OpenseaOrderCollector,
    engine::Engine,
    executors::mempool_alloy_executor::MempoolAlloyExecutor,
    types::{CollectorMap, ExecutorMap},
    eth::{helpers, LocalWallet, Address},
};
use opensea_sudo_arb::{
    strategy::OpenseaSudoArb,
    types::{Action, Config, Event},
};
use opensea_v2::client::{OpenSeaApiConfig, OpenSeaV2Client};
use alloy_provider::ProviderBuilder;
use alloy_transport_ws::WsConnect;

/// 简化的 CLI 参数
#[derive(Parser, Debug)]
pub struct Args {
    /// WebSocket RPC 端点
    #[arg(long)]
    pub wss: String,

    /// OpenSea API Key
    #[arg(long)]
    pub opensea_api_key: String,

    /// 私钥
    #[arg(long)]
    pub private_key: String,

    /// 套利合约地址
    #[arg(long)]
    pub arb_contract_address: String,

    /// 出价百分比 (0-100)
    #[arg(long, default_value = "90")]
    pub bid_percentage: u64,

    /// 运行性能测试
    #[arg(long)]
    pub benchmark: bool,

    /// 启用 rbuilder 优化
    #[arg(long)]
    pub enable_rbuilder: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 设置日志
    let filter = filter::Targets::new()
        .with_target("artemis_quick_start", Level::INFO)
        .with_target("artemis_core", Level::INFO)
        .with_target("opensea_sudo_arb", Level::INFO);
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let args = Args::parse();

    // 性能测试模式
    if args.benchmark {
        return run_performance_test(&args).await;
    }

    // 正常运行模式
    run_optimized_artemis(args).await
}

/// 运行优化版本的 Artemis
async fn run_optimized_artemis(args: Args) -> Result<()> {
    info!("🚀 启动 Artemis 优化版本");

    // 1. 创建优化的 Provider（带自动填充）
    let wallet: LocalWallet = helpers::parse_local_wallet(&args.private_key)?;
    let connect = WsConnect::new(&args.wss);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_ws(connect)
        .await?;
    let provider = Arc::new(provider);

    info!("✅ Provider 连接成功");

    // 2. 创建 OpenSea 客户端
    let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {
        api_key: args.opensea_api_key.clone(),
    });

    // 3. 设置优化引擎
    let mut engine: Engine<Event, Action> = Engine::default()
        .with_event_channel_capacity(2048)  // 优化的缓冲区大小
        .with_action_channel_capacity(1024);

    info!("🔧 引擎配置完成");

    // 4. 添加优化的收集器
    // 区块收集器（带背压控制）
    let block_collector = BlockCollector::new(Arc::clone(&provider))
        .with_buffer_size(2048);
    let block_collector = CollectorMap::new(Box::new(block_collector), Event::NewBlock);
    engine.add_collector(Box::new(block_collector));

    // OpenSea 订单收集器
    let opensea_collector = Box::new(OpenseaOrderCollector::new(args.opensea_api_key));
    let opensea_collector = CollectorMap::new(opensea_collector, |e| Event::OpenseaOrder(Box::new(e)));
    engine.add_collector(Box::new(opensea_collector));

    info!("📡 收集器设置完成");

    // 5. 添加优化策略
    let config = Config {
        arb_contract_address: Address::from_str(&args.arb_contract_address)?,
        bid_percentage: args.bid_percentage,
    };
    let strategy = OpenseaSudoArb::new(Arc::clone(&provider), opensea_client, config);
    engine.add_strategy(Box::new(strategy));

    info!("🎯 策略配置完成");

    // 6. 添加优化执行器
    let executor = Box::new(MempoolAlloyExecutor::new(Arc::clone(&provider)));
    let executor = ExecutorMap::new(executor, |action| match action {
        Action::SubmitTx(tx) => Some(tx),
    });
    engine.add_executor(Box::new(executor));

    // 7. 可选：启用 rbuilder 高级优化
    #[cfg(feature = "rbuilder-integration")]
    if args.enable_rbuilder {
        use artemis_core::executors::rbuilder_executor::{RbuilderExecutor, RbuilderConfig, RbuilderBundle};
        
        info!("🔥 启用 rbuilder 高级优化");
        
        let rbuilder_config = RbuilderConfig {
            rpc_url: "http://localhost:8645".to_string(),
            sorting_algorithm: "max-profit".to_string(),
            relay_urls: vec!["https://boost-relay.flashbots.net".to_string()],
            enable_optimization: true,
        };
        
        let rbuilder_executor = Box::new(RbuilderExecutor::new(rbuilder_config));
        let rbuilder_executor = ExecutorMap::new(rbuilder_executor, |action| match action {
            Action::SubmitTx(_) => {
                // 转换为 rbuilder bundle 格式
                Some(RbuilderBundle {
                    txs: vec![], // TODO: 实际转换
                    target_block: None,
                    min_timestamp: None,
                    max_timestamp: None,
                    reverting_tx_hashes: vec![],
                    replacement_uuid: None,
                })
            }
        });
        engine.add_executor(Box::new(rbuilder_executor));
    }

    info!("⚡ 执行器配置完成");

    // 8. 启动系统
    info!("🚀 启动 Artemis...");
    
    if let Ok(mut set) = engine.run().await {
        info!("✅ Artemis 运行中，监听 MEV 机会...");
        
        while let Some(res) = set.join_next().await {
            if let Err(e) = res {
                tracing::error!("任务错误: {:?}", e);
            }
        }
    }

    Ok(())
}

/// 运行性能测试
async fn run_performance_test(args: &Args) -> Result<()> {
    use artemis_core::benchmarks::PerformanceBenchmark;
    
    println!("🎯 Artemis 性能基准测试");
    
    // 创建测试用 Provider
    let wallet: LocalWallet = helpers::parse_local_wallet(&args.private_key)?;
    let connect = WsConnect::new(&args.wss);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_ws(connect)
        .await?;
    let provider = Arc::new(provider);
    
    // 运行基准测试 (转换 provider 类型)
    let root_provider = Arc::new(helpers::create_ws_provider(&args.wss).await?);
    let benchmark = PerformanceBenchmark::new(root_provider, 1000);
    
    println!("📊 测试 RPC 性能...");
    let rpc_latency = benchmark.benchmark_rpc_calls().await?;
    
    println!("🔥 测试引擎性能...");
    let results = benchmark.run_comparison().await?;
    
    // 显示结果
    benchmark.print_report(&results);
    
    println!("\n🌐 RPC 性能: {:.2}ms (10 并发调用)", rpc_latency.as_millis());
    
    if results.improvement_percentage > 30.0 {
        println!("\n🎉 建议：启用优化功能获得更好性能！");
        println!("使用: --enable-rbuilder 启用高级优化");
    }
    
    Ok(())
}

use std::str::FromStr;
