# Alloy Setup Guide

This guide covers the current Alloy-first workflow for running Artemis.
It captures the required toolchain, feature flags, and the commands you
can use to exercise the Alloy-enabled binaries and examples.

> **Scope**: The instructions below assume you are running in an isolated
> environment (no RPC credentials are embedded in the repo). Real runs still
> require access to Ethereum JSON-RPC, API keys, and funded wallets.

## Requirements
- Rust toolchain (stable) with `cargo`
- An Ethereum WebSocket endpoint (`wss://…`)
- Private key(s) for the bot account(s)
- (Optional) OpenSea API key for the NFT arbitrage strategy
- (Optional) Flashbots signer key for MEV-Share submissions
- Populate the workspace `.env` with the values above so `just`/binaries pick them up.

## Feature Flags
At the workspace root we ship two mutually-exclusive feature flags:

- `sdk-alloy` – enables the Alloy provider/transport stack (preferred).
- `sdk-ethers` – keeps the legacy ethers-rs path available for parity
  checking during migration.

Most binaries/examples now default to `sdk-alloy`, so additional flags are
only required when you need to pin a specific backend. Pass
`--no-default-features --features sdk-alloy` to skip compiling the legacy
ethers crates during workspace builds, or swap in `sdk-ethers` to exercise the
fallback path.

## Building & Testing
```sh
# Full workspace build pinned to Alloy (skips compiling legacy deps)
cargo check --no-default-features --features sdk-alloy

# Run tests for the MEV share example (includes conversion unit tests)
cargo test -p mev-share-arb
```

If you see `Invalid cross-device link` errors while testing inside a
restricted environment, rerun with a temporary `CARGO_TARGET_DIR` that
lives on the same filesystem:
```sh
CARGO_TARGET_DIR=/tmp/artemis-target \
  cargo test -p mev-share-arb --no-default-features --features sdk-alloy
```

## Running `bin/artemis`
The `artemis` binary wires collectors, strategies, and executors for the
OpenSea ↔️ Sudoswap arbitrage flow.

```sh
cargo run --bin artemis \
  --wss wss://your-node.example --opensea-api-key <API_KEY> \
  --private-key 0xbotprivatekey --arb-contract-address 0xArbContract \
  --bid-percentage 10
```

Switch to the legacy backend with

```sh
cargo run --bin artemis \
  --no-default-features --features sdk-ethers -- \
  --wss wss://your-node.example --opensea-api-key <API_KEY> \
  --private-key 0xbotprivatekey --arb-contract-address 0xArbContract \
  --bid-percentage 10
```

## Quick Alloy Smoke Test

Need a fast sanity check? The repository ships an example that connects to a
WebSocket endpoint and prints the latest block number via Alloy helpers:

```sh
ALLOY_WS_ENDPOINT=ws://localhost:8545 \
  cargo run -p alloy-quickstart
```

`ALLOY_WS_ENDPOINT` defaults to `ws://localhost:8545`, making it easy to point
the example at a local Anvil instance. Set it to your hosted endpoint (Infura,
Alchemy, etc.) as needed. A template is available at
`examples/alloy-quickstart/.env.example`.

## Running the MEV-Share Example
The Alloy feature gate activates both the strategy and the Alloy-native
executor (bundle submission via `alloy-mev`).

```sh
cargo run --bin mev-share-arb \
  --wss wss://your-node.example --private-key 0xBotKey \
  --flashbots-signer 0xFbKey --arb-contract-address 0xArbContract
```

Fallback command:

```sh
cargo run --bin mev-share-arb \
  --no-default-features --features sdk-ethers -- \
  --wss wss://your-node.example --private-key 0xBotKey \
  --flashbots-signer 0xFbKey --arb-contract-address 0xArbContract
```

Under the hood the example converts the legacy `mev_share::rpc` bundle
structure to `alloy_rpc_types_mev::MevSendBundle` before handing work to
`MevshareAlloyExecutor`.

## Development Notes
- The Alloy collectors currently poll (`tokio::time::interval`) until
  native subscriptions become available via `alloy-provider`.
- When wiring new strategies, prefer `artemis_core::eth::alloy_support::*`
  for helpers, type aliases, and provider builders.
- During migration you can still build both SDKs together for parity
  testing (`cargo check --all-features`), but we encourage developing new
  codepaths behind `sdk-alloy` only.

If you spot gaps or run into issues, update `docs/ethers_to_alloy_todo.md`
so we keep the migration tracker accurate.
