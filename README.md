
![](./assets/artemis.png)


[![CI status](https://github.com/paradigmxyz/reth/workflows/ci/badge.svg)][gh-ci]
[![Telegram Chat][tg-badge]][tg-url]

[gh-ci]: https://github.com/paradigmxyz/reth/actions/workflows/rust.yml
[tg-badge]: https://img.shields.io/badge/chat-telegram-blue

## What is Artemis?

Artemis is a framework for writing MEV bots in Rust. It's designed to be simple, modular, and fast.

Key updates (Alloy migration):
- SDK adapter layer with feature flags: default enables both `sdk-alloy` and `sdk-ethers` for a smooth transition. New code paths prefer Alloy; ethers remains available for fallback modules.
- Optional Alloy MEV integration: `alloy-mev` is wired behind `sdk-alloy` with a new executor skeleton (`flashbots_alloy_executor`).
- Backpressure: collectors now use bounded channels to handle bursts without blocking.

At its core, Artemis is architected as an event processing pipeline. The library is made up of three main components: 

1. *Collectors*: *Collectors* take in external events (such as pending txs, new blocks, marketplace orders, etc. ) and turn them into an internal *event* representation. 
2. *Strategies*: *Strategies* contain the core logic required for each MEV opportunity. They take in *events* as inputs, and compute whether any opportunities are available (for example, a strategy might listen to a stream of marketplace orders to see if there are any cross-exchange arbs). *Strategies* produce *actions*.
3. *Executors*: *Executors* process *actions*, and are responsible for executing them in different domains (for example, submitting txs, posting off-chain orders, etc.).

## Strategies 

The following strategies have been implemented: 

- [Opensea/Sudoswap NFT Arbitrage](/crates/strategies/opensea-sudo-arb/): A strategy implementing atomic, cross-market NFT arbitrage between Seaport and Sudoswap.

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
cargo run -- --wss <WSS_ENDPOINT> --opensea-api-key <OPENSEA_API_KEY> --private-key <PRIVATE_KEY> --arb-contract-address <ARB_CONTRACT_ADDRESS> --bid-percentage <BID_PERCENTAGE>

Feature flags:
- Enable Alloy only:
```
cargo run --features sdk-alloy
```
- Enable ethers fallback only:
```
cargo run --no-default-features --features sdk-ethers
```

Alloy MEV (Flashbots/MEV-Share):
- We include `alloy-mev` as an optional dependency. An Alloy-based Flashbots executor is available under `sdk-alloy` (module: `flashbots_alloy_executor`). It will submit bundles via HTTP provider extensions. See `alloy-mev` for API details:
  - https://github.com/leruaa/alloy-mev
```

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
