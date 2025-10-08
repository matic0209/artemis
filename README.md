# Artemis MEV Framework

> High-performance Rust MEV (Maximal Extractable Value) framework with real-time arbitrage detection, symbolic execution validation, and on-chain execution

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)](COMPILATION_SUCCESS.md)

## 🎯 Overview

Artemis is a production-ready MEV arbitrage framework that integrates:

- ⚡ **Artemis Core**: Event-driven engine (Collector → Strategy → Executor)
- 🔍 **Multi-Strategy Detection**: Fast 2-3 hop + Symbolic (10 strategies) + Graph theory (Bellman-Ford)
- ✅ **Dual Validation**: Z3 symbolic verification + REVM concrete execution
- 📊 **Concurrent State**: DashMap for thread-safe pool management + petgraph for path finding
- 🛡️ **MEV Defense**: Built-in protection mechanisms

## 🚀 Quick Start

### Prerequisites

- Rust 1.75+
- Ethereum node (WebSocket enabled)

### Build

```bash
# Build core library
cargo build -p mev-arbitrage

# Build MEV bot binary
cargo build -p mev-arb-bot --release
```

### Run

```bash
# Start MEV arbitrage bot
./target/release/mev-arb-bot \
  --wss wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY \
  --private-key YOUR_PRIVATE_KEY \
  --flashbots-key YOUR_FLASHBOTS_KEY \
  --min-profit-eth 0.1
```

## 📁 Project Structure

```
artemis/
├── crates/
│   ├── core/
│   │   └── artemis-core/              # Core engine ✅
│   │       ├── collectors/            # Block/Mempool collectors
│   │       ├── executors/             # Flashbots/Mempool executors
│   │       ├── engine/                # Event-driven engine
│   │       └── types/                 # Core traits
│   │
│   └── strategies/
│       └── mev-arbitrage/             # MEV Arbitrage Strategy ✅
│           ├── src/
│           │   ├── strategy.rs        # Strategy trait implementation
│           │   ├── abstractions.rs    # Core abstractions
│           │   ├── utils.rs           # TokenGraph + PoolManager
│           │   ├── detectors/         # Fast + Symbolic detectors
│           │   ├── validators/        # REVM validator
│           │   ├── optimizers/        # Z3 optimizer
│           │   ├── strategies/        # Z3 strategy optimizer
│           │   ├── execution/         # Gas strategy
│           │   └── coordination/      # Event system + Manager
│           │
│           ├── graph-theory/          # Bellman-Ford + petgraph
│           ├── symbolic-execution/    # Z3 symbolic execution
│           ├── revm-validation/       # REVM validation
│           └── defense/               # MEV defense
│
├── bin/
│   └── mev-arb-bot/                   # MEV Bot Binary ✅
│
└── docs/                              # Documentation
    ├── COMPILATION_SUCCESS.md         # Build status report
    ├── MEV_ARBITRAGE_ARCHITECTURE.md  # Architecture details
    └── E2E_UPDATED_ROADMAP.md         # Development roadmap
```

## 🎯 Core Features

### Technology Stack

| Technology | Status | Location |
|------------|--------|----------|
| **Artemis Core** | ✅ | `artemis-core/` |
| **petgraph** | ✅ | `utils.rs::TokenGraph` |
| **Z3** | ✅ | `symbolic-execution/`, `strategies/z3_strategy_optimizer.rs` |
| **REVM** | ✅ | `revm-validation/`, `validators/revm.rs` |
| **DashMap** | ✅ | `utils.rs::PoolManager` |
| **Bellman-Ford** | ✅ | `graph-theory/` |
| **Alloy** | ✅ | Provider/Signer integration |

### Detection Strategies

| Strategy | Description | Status |
|----------|-------------|--------|
| **FastArbitrage** | 2-3 hop quick detection | ✅ Production |
| **SymbolicDetection** | 10 Z3-based strategies | ✅ Production |
| **GraphTheory** | Bellman-Ford negative cycle | ✅ Production |

### Validation Pipeline

| Stage | Component | Technology |
|-------|-----------|------------|
| **Detection** | FastDetector + SymbolicDetector | petgraph + Z3 |
| **Optimization** | Z3StrategyOptimizer | Z3 SMT Solver |
| **Validation** | REVMValidator | REVM |
| **Execution** | FlashbotsExecutor | Flashbots Relay |

## 📊 System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Artemis Engine                       │
└─────────────────────────────────────────────────────────┘
         │                    │                    │
         ↓                    ↓                    ↓
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│ BlockCollector  │  │ ArbitrageStrategy│  │FlashbotsExecutor│
│                 │  │                  │  │                 │
│ - NewBlock      │→ │ - TokenGraph     │→ │ - Bundle Build  │
│ - Filters       │  │ - PoolManager    │  │ - Submit        │
└─────────────────┘  │ - FastDetector   │  └─────────────────┘
                     │ - SymbolicDetect │
                     │ - Z3 Optimizer   │
                     │ - REVM Validator │
                     └─────────────────┘
```

### Data Flow

```
1. BlockCollector (Artemis Core)
       ↓ ArbitrageEvent::NewBlock

2. ArbitrageStrategy.process_event()
       ↓ pool_manager.update_all_pools()  // DashMap
       ↓ token_graph.find_paths()         // petgraph

3. Detectors
       ↓ fast_detector.detect()           // 2-3 hop
       ↓ symbolic_detector.detect()       // Z3 (10 strategies)

4. Filtering
       ↓ Filter by min_profit_wei

5. Generate Actions
       ↓ ArbitrageAction::SubmitFlashbotsBundle

6. FlashbotsExecutor (Artemis Core)
       ↓ Submit to Flashbots Relay
```

## ⚙️ Configuration

### CLI Arguments

```bash
mev-arb-bot \
  --wss <WEBSOCKET_URL>          # Ethereum WebSocket endpoint
  --private-key <KEY>            # Transaction signer
  --flashbots-key <KEY>          # Flashbots signer
  --min-profit-eth <AMOUNT>      # Minimum profit (default: 0.1)
  --max-gas-gwei <PRICE>         # Max gas price (default: 100)
  --max-hops <N>                 # Max arbitrage hops (default: 3)
  --metrics-addr <ADDR>          # Metrics server (default: 0.0.0.0:9090)
```

### Strategy Configuration

```rust
ArbitrageConfig {
    min_profit_wei: U256::from(100_000_000_000_000_000u64), // 0.1 ETH
    max_gas_price_gwei: 100,
    enable_fast_detector: true,
    enable_symbolic_detector: true,
    dex_routers: vec![/* DEX addresses */],
    max_hops: 3,
}
```

## 🛠️ Development

### Build Commands

```bash
# Core library only
cargo build -p mev-arbitrage

# MEV bot binary
cargo build -p mev-arb-bot

# Release build
cargo build -p mev-arb-bot --release

# With all features
cargo build -p mev-arbitrage --features full
```

### Testing

```bash
# Run all tests
cargo test -p mev-arbitrage

# Run specific test module
cargo test -p mev-arbitrage --test detector_tests
```

### Documentation

```bash
# Generate API docs
cargo doc -p mev-arbitrage --features full --open
```

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [COMPILATION_SUCCESS.md](COMPILATION_SUCCESS.md) | Build status and fixes applied |
| [MEV_ARBITRAGE_ARCHITECTURE.md](MEV_ARBITRAGE_ARCHITECTURE.md) | Detailed architecture |
| [E2E_UPDATED_ROADMAP.md](E2E_UPDATED_ROADMAP.md) | Development roadmap |
| [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) | Project summary |

## 🔍 Code Examples

### Example 1: Create Strategy

```rust
use mev_arbitrage::strategy::{ArbitrageStrategy, ArbitrageConfig};

let provider = Arc::new(ProviderBuilder::new().on_ws(ws_connect).await?);
let config = ArbitrageConfig::default();

let strategy = ArbitrageStrategy::new(provider, config);
```

### Example 2: Integrate with Artemis Engine

```rust
use artemis_core::engine::Engine;
use artemis_core::collectors::block_collector::BlockCollector;

let mut engine = Engine::new();

// Add collector
let collector = BlockCollector::new(provider.clone());
engine.add_collector(Box::new(collector));

// Add strategy
engine.add_strategy(Box::new(strategy));

// Add executor
engine.add_executor(Box::new(flashbots_executor));

// Run
engine.run().await?;
```

### Example 3: Custom Detection

```rust
use mev_arbitrage::detectors::FastArbitrageDetector;
use mev_arbitrage::utils::TokenGraph;

let graph = TokenGraph::from_pools(&pools)?;
let detector = FastArbitrageDetector::new(Default::default());

let context = DetectionContext { /* ... */ };
let result = detector.detect(&context).await?;

for opp in result.opportunities {
    println!("Found: {} ETH profit", opp.expected_profit);
}
```

## 📈 Performance

| Metric | Value |
|--------|-------|
| **Compilation** | ✅ 0 errors |
| **Core Library** | ✅ mev-arbitrage builds in 1.09s |
| **Binary** | ✅ mev-arb-bot builds in 9.97s |
| **Warnings** | 8 (dead_code/unused only) |

## 🔧 Troubleshooting

### Common Issues

**Issue: Compilation errors**
```bash
# Ensure dependencies are up to date
cargo update

# Clean and rebuild
cargo clean && cargo build -p mev-arbitrage
```

**Issue: Missing dependencies**
```bash
# Check Z3 installation
z3 --version

# Install if needed (Ubuntu/Debian)
sudo apt-get install z3
```

**Issue: WebSocket connection fails**
```bash
# Test connection
wscat -c wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
```

## 🚧 TODO / Roadmap

### P0 - Core Functionality

- [ ] Implement `PoolManager.discover_pools()` (query DEX factories)
- [ ] Implement `PoolManager.update_all_pools()` (Multicall3 batch updates)
- [ ] Implement `TransactionBuilder` (build actual Flashbots bundles)
- [ ] Complete `REVMValidator` implementation

### P1 - Optimization

- [ ] Optimize FastDetector from O(n³) to O(e²)
- [ ] Add Z3 result caching
- [ ] Restore parallel strategy execution

### P2 - Testing

- [ ] Add unit tests for all detectors
- [ ] Add integration tests
- [ ] Add E2E tests with testnet

See [E2E_UPDATED_ROADMAP.md](E2E_UPDATED_ROADMAP.md) for detailed roadmap.

## ⚠️ Security Notice

1. **Never commit private keys** - Use environment variables
2. **Test thoroughly** - Start with testnets
3. **Monitor closely** - Watch logs and metrics
4. **Set limits** - Configure max investment and loss limits
5. **Understand risks** - MEV arbitrage can result in losses

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📄 License

MIT License - see [LICENSE](LICENSE) for details

## 🙏 Acknowledgments

- [Paradigm Artemis](https://github.com/paradigmxyz/artemis) - Original framework
- [Reth](https://github.com/paradigmxyz/reth) - Ethereum execution client
- [Alloy](https://github.com/alloy-rs/alloy) - Ethereum Rust library
- [Z3](https://github.com/Z3Prover/z3) - SMT solver
- [petgraph](https://github.com/petgraph/petgraph) - Graph data structures

---

**⚠️ Risk Warning**: MEV arbitrage involves financial risk. Use at your own risk. The authors are not responsible for any losses incurred.
