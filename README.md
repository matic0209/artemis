
![](./assets/artemis.png)


[![CI status](https://github.com/paradigmxyz/reth/workflows/ci/badge.svg)][gh-ci]
[![Telegram Chat][tg-badge]][tg-url]

[gh-ci]: https://github.com/paradigmxyz/reth/actions/workflows/rust.yml
[tg-badge]: https://img.shields.io/badge/chat-telegram-blue

## What is Artemis?

Artemis is a framework for writing MEV bots in Rust. It's designed to be simple, modular, and fast.

**🚀 Now powered by Alloy**: Artemis has been fully migrated from ethers-rs to the modern [Alloy](https://github.com/alloy-rs/alloy) Ethereum SDK, providing:
- ⚡ **Better Performance**: Optimized RPC client with connection pooling and batching
- 🔧 **Modern APIs**: Type-safe contract bindings using `sol!` macros
- 📊 **Enhanced Monitoring**: Built-in metrics and backpressure handling
- 🎯 **MEV Integration**: Native support for Flashbots and MEV-Share via [`alloy-mev`](https://github.com/leruaa/alloy-mev)

At its core, Artemis is architected as an event processing pipeline. The library is made up of three main components: 

1. *Collectors*: *Collectors* take in external events (such as pending txs, new blocks, marketplace orders, etc. ) and turn them into an internal *event* representation. 
2. *Strategies*: *Strategies* contain the core logic required for each MEV opportunity. They take in *events* as inputs, and compute whether any opportunities are available (for example, a strategy might listen to a stream of marketplace orders to see if there are any cross-exchange arbs). *Strategies* produce *actions*.
3. *Executors*: *Executors* process *actions*, and are responsible for executing them in different domains (for example, submitting txs, posting off-chain orders, etc.).

## Strategies 

The following strategies have been implemented and migrated to Alloy: 

- [Opensea/Sudoswap NFT Arbitrage](/crates/strategies/opensea-sudo-arb/): ✅ **Fully migrated** - Atomic cross-market NFT arbitrage between Seaport and Sudoswap using Alloy contract bindings and state overrides.
- [MEV-Share Uniswap Arbitrage](/crates/strategies/mev-share-uni-arb/): ✅ **Fully migrated** - Probabilistic Uniswap V3/V2 arbitrage on MEV-Share using Alloy providers and signers.

## Build, Test and Run

First, make sure the following are installed: 
1. [Anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil#installing-from-source)

In order to build, first clone the github repo: 

```sh
git clone https://github.com/paradigmxyz/artemis
cd artemis
```

Next, run tests with cargo:

```sh
cargo test --workspace --all-features
```

In order to run the opensea sudoswap arbitrage strategy, you can run the following command:

```sh
cargo run --bin artemis -- --wss <WSS_ENDPOINT> --opensea-api-key <OPENSEA_API_KEY> \
  --private-key <PRIVATE_KEY> --arb-contract-address <ARB_CONTRACT_ADDRESS> \
  --bid-percentage <BID_PERCENTAGE>
```

## Architecture (Alloy-Powered)

**Collectors** (Data Sources):
- `BlockCollector`: Subscribes to new blocks via Alloy WebSocket with bounded channel backpressure
- `MempoolCollector`: Tracks pending transactions with configurable batching  
- `OpenseaOrderCollector`: Monitors OpenSea order streams
- `MevShareCollector`: Listens to MEV-Share event streams

**Executors** (Action Handlers):
- `MempoolAlloyExecutor`: Submits transactions with gas optimization and caching
- `FlashbotsAlloyExecutor`: Sends bundles to block builders via `alloy-mev`
- `MevshareAlloyExecutor`: Submits MEV-Share bundles with signing

**Performance Features**:
- 🚀 **Bounded Channels**: Prevent memory bloat during high-throughput periods
- ⚡ **Gas Caching**: LRU cache for gas estimates to reduce RPC calls
- 📊 **Metrics**: Prometheus-compatible metrics for monitoring
- 🔄 **Connection Pooling**: Efficient RPC connection reuse

### Environment Configuration

Environment variables are loaded automatically via `just` (see `set dotenv-load := true` in the
`justfile`). Populate the root `.env` file with:

- `ARTEMIS_METRICS_ADDR`: where the Prometheus exporter should bind when the CLI runs.
- `ALLOY_WS_ENDPOINT`: default WebSocket RPC used by Alloy-based examples such as
  `examples/alloy-quickstart`.
- `ETH_MAINNET_HTTP`: HTTPS mainnet RPC endpoint required by Foundry fork tests.
- `ARTEMIS_*` and `MEV_SHARE_*`: optional helpers to store the arguments you pass to the
  `artemis` binary and the MEV-Share example, keeping sensitive keys outside of your shell
  history.
- `ETHERSCAN_API_KEY`, `CHAINBOUND_API_KEY`, `FIBER_TEST_KEY`: API tokens for downloading
  protocol artifacts or enabling the optional Chainbound integrations/tests.

Feature flags:

- Alloy is now the default, so `cargo run --bin artemis -- <ARGS>` spins up the
  Alloy stack out of the box.
- Opt into the legacy ethers backend with:

  ```sh
  cargo run --bin artemis --no-default-features --features sdk-ethers -- <ARGS>
  ```

- If you want to skip compiling any ethers code during workspace builds, add

  ```sh
  cargo run --bin artemis --no-default-features --features sdk-alloy -- <ARGS>
  ```

Alloy MEV (Flashbots / MEV-Share):

- We include `alloy-mev` as an optional dependency. An Alloy-based Flashbots executor is available under `sdk-alloy` (module: `flashbots_alloy_executor`). It will submit bundles via HTTP provider extensions. See `alloy-mev` for API details: https://github.com/leruaa/alloy-mev

  To run the MEV-Share example:

  ```sh
  cargo run --bin mev-share-arb -- \
    --wss <WSS_ENDPOINT> --private-key <BOT_KEY> --flashbots-signer <SIGNER_KEY> \
    --arb-contract-address <ARB_CONTRACT_ADDRESS>
  ```

  Legacy backend:

  ```sh
  cargo run --bin mev-share-arb --no-default-features --features sdk-ethers -- \
    --wss <WSS_ENDPOINT> --private-key <BOT_KEY> --flashbots-signer <SIGNER_KEY> \
    --arb-contract-address <ARB_CONTRACT_ADDRESS>
  ```

Alloy quickstart example:

- A lightweight smoke test lives at `examples/alloy_quickstart.rs`. It connects
  to a WebSocket endpoint (defaults to `ws://localhost:8545`) and fetches the
  latest block via Alloy:

  ```sh
  ALLOY_WS_ENDPOINT=ws://localhost:8545 \
    cargo run -p alloy-quickstart
  ```

  Add `--no-default-features --features sdk-ethers` if you need to compare with
  the legacy stack.

For a consolidated checklist covering prerequisites, build commands, and
runtime instructions, see [docs/alloy_setup.md](docs/alloy_setup.md).

where `ARB_CONTRACT_ADDRESS` is the address to which you deploy the [arb contract](/crates/strategies/opensea-sudo-arb/contracts/src/SudoOpenseaArb.sol).


## Acknowledgements

- [subway](https://github.com/libevm/subway)
- [subway-rs](https://github.com/refcell/subway-rs)
- [cfmms-rs](https://github.com/0xKitsune/cfmms-rs)
- [rusty-sando](https://github.com/mouseless-eth/rusty-sando)
- [bundle-generator](https://github.com/Alcibiades-Capital/mev_bundle_generator/blob/master/Cargo.toml)
- [ethers-rs](https://github.com/gakonst/ethers-rs)
- [ethers-flashbots](https://github.com/onbjerg/ethers-flashbots)
- [alloy-rs](https://github.com/alloy-rs/alloy)
- [alloy-mev](https://github.com/leruaa/alloy-mev)



[tg-url]: https://t.me/artemis_devs
