# Quick Start Guide

Get started with Artemis MEV Framework in 5 minutes.

## Prerequisites

- Rust 1.75+
- Ethereum node with WebSocket (Alchemy, Infura, or local Reth)

## Installation

```bash
# Clone repository
git clone <repo-url>
cd artemis

# Build
cargo build -p mev-arb-bot --release
```

## Configuration

```bash
# Set environment variables
export ETH_WS_URL="wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY"
export PRIVATE_KEY="0x..."
export FLASHBOTS_KEY="0x..."
```

## Run

```bash
./target/release/mev-arb-bot \
  --wss $ETH_WS_URL \
  --private-key $PRIVATE_KEY \
  --flashbots-key $FLASHBOTS_KEY \
  --min-profit-eth 0.1
```

## What's Next?

- Read [README.md](README.md) for full documentation
- Check [MEV_ARBITRAGE_ARCHITECTURE.md](MEV_ARBITRAGE_ARCHITECTURE.md) for architecture
- Review [E2E_UPDATED_ROADMAP.md](E2E_UPDATED_ROADMAP.md) for roadmap
