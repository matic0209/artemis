# DeFi Analyzer Strategy

A comprehensive DeFi analysis strategy for Artemis that integrates symbolic execution capabilities with MEV framework to analyze DeFi protocols for inconsistencies and potential arbitrage opportunities.

## Features

- **Symbolic Execution**: Complete EVM interpreter with Z3 integration for symbolic analysis
- **Negative Cycle Arbitrage**: Advanced graph-based arbitrage detection using Bellman-Ford algorithm
- **ABI Parsing**: Comprehensive ABI parsing and parameter encoding/decoding
- **Path Exploration**: DFS-based execution path exploration with loop detection
- **DeFi Feature Extraction**: Specialized DeFi feature detection and analysis
- **Artemis Integration**: Seamless integration with Artemis collectors and executors
- **Multi-Protocol Support**: Uniswap V2/V3, SushiSwap, Curve, and more
- **Real-time Analysis**: High-performance real-time arbitrage opportunity detection
- **Observability**: Built-in monitoring and observability capabilities

## Architecture

### Core Components

- **`evm_interpreter`**: Complete EVM symbolic interpreter with all opcodes
- **`abi_parser`**: ABI parsing and parameter handling
- **`defi_feature_extractor`**: DeFi-specific feature detection
- **`path_explorer`**: Execution path exploration and analysis
- **`arbitrage_detector`**: Basic arbitrage opportunity detection algorithms
- **`negative_cycle_arbitrage`**: Advanced negative cycle arbitrage engine using graph theory
- **`collectors`**: Artemis collector integration for data sources
- **`executors`**: Artemis executor integration for action execution
- **`integration_examples`**: Complete integration examples and use cases
- **`observability`**: Monitoring and observability infrastructure

### Key Features

1. **Symbolic Execution Engine**
   - Full EVM opcode support
   - Z3 constraint solver integration
   - Cross-contract call handling
   - Memory and storage management

2. **Negative Cycle Arbitrage Algorithm**
   - Graph-based arbitrage detection using Bellman-Ford algorithm
   - Multi-protocol arbitrage path discovery
   - Dynamic state-aware price modeling
   - Optimal investment parameter search
   - Cross-protocol cycle detection

3. **Traditional Arbitrage Detection**
   - Price arbitrage detection
   - Liquidity arbitrage detection
   - Cross-protocol arbitrage
   - Risk assessment and profit calculation

4. **ABI Integration**
   - Multi-source ABI fetching (Etherscan, Sourcify, 4byte, OpenChain)
   - Parameter encoding/decoding
   - Function signature analysis

5. **Path Exploration**
   - DFS-based execution path exploration
   - Loop detection and path pruning
   - Path deduplication and optimization

6. **Artemis Integration**
   - Seamless collector integration (blocks, logs, mempool)
   - Executor integration (mempool, Flashbots)
   - Event stream processing
   - Action execution pipeline

## Usage

### Basic Usage

```rust
use defi_analyzer::{DeFiAnalyzer, AnalyzerConfig, AnalysisEvent};

// Create analyzer with configuration
let config = AnalyzerConfig::default();
let mut analyzer = DeFiAnalyzer::new(config);

// Initialize the analyzer
analyzer.initialize().await?;

// Analyze a transaction
let event = AnalysisEvent {
    block_number: 12345,
    transaction_hash: [0u8; 32],
    contract_address: Address::ZERO,
    transaction_data: vec![0x60, 0x01, 0x60, 0x02, 0x01, 0x00],
    event_type: "transfer".to_string(),
    event_data: vec![],
    timestamp: 1234567890,
};

let result = analyzer.analyze_event(&event).await?;
```

### Negative Cycle Arbitrage Integration

```rust
use defi_analyzer::{
    DeFiAnalyzerStrategy, 
    NegativeCycleArbitrageEngine,
    NegativeCycleConfig,
    StateSnapshot,
    integration_examples::run_all_examples,
};

// Create strategy with negative cycle arbitrage
let mut strategy = DeFiAnalyzerStrategy::new(config);
strategy.initialize().await?;

// The strategy automatically integrates negative cycle arbitrage
// and will detect arbitrage opportunities using graph algorithms
let actions = strategy.process_event(analysis_event).await;
```

### Advanced Integration Examples

```rust
// Run comprehensive integration examples
run_all_examples().await?;

// Create custom arbitrage configuration
let arbitrage_config = NegativeCycleConfig {
    target_revenue: U256::from(5_000_000_000_000_000u64), // 0.005 ETH
    max_cycles_per_iteration: 10,
    max_path_length: 6,
    supported_protocols: vec![
        "uniswap_v2".to_string(),
        "uniswap_v3".to_string(),
        "sushiswap".to_string(),
        "curve".to_string(),
    ],
    ..Default::default()
};

// Manual arbitrage engine usage
let mut arbitrage_engine = NegativeCycleArbitrageEngine::new(arbitrage_config);
let state_snapshot = StateSnapshot::from_analysis_event(&event);
let total_revenue = arbitrage_engine.execute_arbitrage_algorithm(&state_snapshot).await?;
```

### Artemis Integration

```rust
use defi_analyzer::{
    DeFiAnalyzerStrategy, 
    DeFiCollectorConfig, 
    DeFiExecutorConfig,
    DeFiExecutorFactory
};

// Create strategy with Artemis integration
let mut strategy = DeFiAnalyzerStrategy::new(config);
strategy.initialize().await?;
    
// Configure collectors
let collector_config = DeFiCollectorConfig {
    enable_block_analysis: true,
    enable_log_analysis: true,
    enable_mempool_analysis: true,
    monitored_protocols: vec![
        "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".to_string(), // Uniswap V2
        "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(), // Uniswap V3
    ],
    ..Default::default()
};

// Configure executors
let executor_config = DeFiExecutorConfig {
    enable_mempool_execution: true,
    enable_flashbots_execution: true,
    min_profit_threshold: 1_000_000_000_000_000, // 0.001 ETH
    ..Default::default()
};

// Create executors
let executors = DeFiExecutorFactory::create_all_executors(executor_config)?;
for executor in executors {
    strategy.add_executor(executor);
}
```

## Configuration

### Analyzer Configuration

The analyzer can be configured through the `AnalyzerConfig` struct:

```rust
let config = AnalyzerConfig {
    max_execution_paths: 1000,
    max_depth: 50,
    enable_arbitrage_detection: true,
    enable_consistency_checking: true,
    min_profit_threshold: U256::from(1000),
    // ... other options
};
```

### Collector Configuration

Configure data sources through `DeFiCollectorConfig`:

```rust
let collector_config = DeFiCollectorConfig {
    enable_block_analysis: true,
    enable_log_analysis: true,
    enable_mempool_analysis: true,
    min_gas_price: 20_000_000_000, // 20 gwei
    monitored_protocols: vec![
        "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".to_string(), // Uniswap V2
        "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(), // Uniswap V3
        "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".to_string(), // SushiSwap
    ],
};
```

### Executor Configuration

Configure action execution through `DeFiExecutorConfig`:

```rust
let executor_config = DeFiExecutorConfig {
    enable_mempool_execution: true,
    enable_flashbots_execution: true,
    max_gas_price: 100_000_000_000, // 100 gwei
    min_profit_threshold: 1_000_000_000_000_000, // 0.001 ETH
    flashbots_timeout: 12, // 12 seconds
};
```

## Testing

The project includes comprehensive tests covering:

- Basic arithmetic operations
- Memory and storage operations
- Control flow operations
- Cross-contract calls
- Arbitrage detection
- ABI parsing
- Path exploration
- Edge cases

Run tests with:

```bash
cargo test --package defi-analyzer
```

## Dependencies

- **Z3**: Symbolic execution and constraint solving
- **Alloy**: Ethereum primitives and types
- **Tokio**: Async runtime
- **Tracing**: Logging and observability
- **Serde**: Serialization

## Performance

The analyzer is optimized for performance with:

- Efficient path exploration algorithms
- Smart caching mechanisms
- Parallel processing capabilities
- Memory optimization
- Gas estimation
- Artemis integration for high-throughput event processing
- Optimized collector and executor pipelines

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.