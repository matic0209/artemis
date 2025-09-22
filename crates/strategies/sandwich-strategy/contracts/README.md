# Sandwich Strategy Huff Contracts

High-performance Huff implementation for MEV sandwich attacks with gas optimizations.

## Gas Savings

| Operation | Solidity | Huff | Savings |
|-----------|----------|------|---------|
| Sandwich V2 | ~180,000 | ~175,000 | ~5,000 |
| Sandwich V3 | ~200,000 | ~195,000 | ~5,000 |
| Emergency Withdraw | ~30,000 | ~28,000 | ~2,000 |

## Usage

The contracts are precompiled and can be deployed directly:

```rust
use sandwich_strategy::contracts::SandwichContractCompiled;

// Deploy the contract
let contract_address = SandwichContractCompiled::deploy(&provider).await?;
```

## Features

- **Gas Optimized**: 5,000+ gas savings per sandwich
- **MEV Protection**: Built-in anti-MEV measures
- **Multi-DEX Support**: Uniswap V2/V3, Sushiswap
- **Emergency Functions**: Safe withdrawal mechanisms
- **Access Control**: Owner-only critical functions

## Compilation

```bash
# Install huffc
cargo install huffc

# Compile contracts
huffc --bytecode src/huff/SandwichContract.huff
```
