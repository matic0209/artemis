# Ethers ➜ Alloy Migration TODO

This checklist tracks the multi-stage migration plan from `ethers-rs` to the Alloy SDK. Each
item should be completed and validated (compilation + relevant tests) before moving to the next.

## Phase 0 – Preparation
- [x] Confirm target Alloy crate versions / features (`alloy-provider`, `alloy-transport`, `alloy-mev`, etc.). *(workspace crates pinned to Alloy `1.x`, core features cover `ws`, `reqwest`, local signer, MEV APIs; tracked in `artemis-core` + strategy manifests).* 
- [x] Decide on binding generation approach (e.g. `sol!` macros) and verify basic RPC/WebSocket connectivity. *(`sol!`-generated bindings land in each strategy crate; `artemis_core::eth::alloy_support::helpers` smoke test WebSocket provider + local wallet in `bin/artemis`.)*
- [x] Define migration success criteria (builds, tests, binaries, demos). *(Success = `sdk-alloy` only builds/tests green on CI, CLI/examples run end-to-end with Alloy stack, docs updated, and `sdk-ethers`/legacy code deleted.)*

## Phase 1 – Core Infrastructure
- [x] Implement Alloy-based types/adapters in `artemis-core/src/eth/mod.rs` (provider, signer, tx, log, etc.).
- [x] Expose unified traits so upper layers do not depend on `ethers` directly.
- [x] Wire Alloy crates in `Cargo.toml`, retain `sdk-ethers` fallback during migration.
- [x] Add smoke test for Alloy provider + signer.
- [x] Maintain `cfg`-guarded `ethers` path for regression parity.

## Phase 2 – Shared Utilities & Middleware
- [x] Port `artemis-core/src/types.rs` and `utilities` modules to Alloy types. *(state override middleware now exposes an Alloy helper; ethers variant still compiled under `sdk-ethers`)*
- [x] Update helpers and engine logic (state override, hashing, type aliases). *(Alloy helpers now cover keccak + action aliases; state override remains feature-gated parity)*
- [x] Rework Flashbots/MEV middleware to Alloy RPC primitives. *(Flashbots, mempool, and MEV-Share executors now offer Alloy implementations)*
- [x] Update collectors (block, log, mempool, MEV-share, OpenSea) to Alloy streaming APIs. *(block/log/mempool now polling via Alloy; MEV-Share uses shared SSE client; OpenSea remains WebSocket client without ethers dependencies)*
- [x] Adjust unit/mocking infrastructure (Anvil, SSE) for Alloy compatibility. *(Ethers-based tests gated; Alloy smoke tests remain)*

## Phase 3 – Contracts & Strategies
- [ ] Regenerate contract bindings with Alloy tooling; replace `ethers`-generated code under `bindings/`. *(Remaining: convert residual `ethers` `abigen!` modules to `sol!` variants or delete once the legacy feature flag is gone.)*
    - [x] `mev-share-uni-arb` bindings now use `sol!`/Alloy; legacy `ethers` code generated at build time.
    - [x] `opensea-sudo-arb` bindings now expose `sol!`-based definitions for the Sudo contracts and helpers (Alloy ABI parity pending widened coverage).
- [ ] Update strategies (`opensea-sudo-arb`, `mev-share-uni-arb`, etc.) to use Alloy bindings and types. *(Outstanding: finish the Alloy implementation for `opensea-sudo-arb` and untangle shared utilities from `ethers`.)*
    - [x] `mev-share-uni-arb` compiles on both SDKs via split `cfg` modules.
    - [x] `opensea-sudo-arb` strategy skeleton compiles under `sdk-alloy` (logic parity TBD). *(Alloy build now passes; next step is wiring signer/provider ownership and reconciling touched-pool refresh logic for runtime parity.)*
- [ ] Rewrite transaction/bundle construction to Alloy encoding/signing APIs. *(Remaining focus area: finish `opensea-sudo-arb` signing path + wallet sourcing; `mev-share-uni-arb` already Alloy-native.)*
    - [x] `mev-share-uni-arb` Alloy path signs via `EthereumWallet` and encodes bundles with `TxEnvelope`.
- [ ] Validate strategy state sync & event processing with Alloy paths (add tests/snapshots as needed). *(Need Alloy-driven integration/smoke coverage for strategies + collectors.)*

## Phase 4 – Applications & Examples
- [ ] Migrate CLI binaries (`bin/artemis`, `examples/*`) to Alloy initialization flows. *(Replace `ethers` helpers with `alloy_support::helpers`, ensure strategies/executors load under Alloy feature only.)*
- [ ] Update documentation/tutorials for new setup (`README`, docs/ guides). *(Document Alloy-first setup, env vars, and build flags.)*
- [ ] Provide Alloy-based end-to-end demos (collector→strategy→executor). *(Publish runnable scripts or walkthroughs proving the Alloy stack end-to-end.)*

## Phase 5 – Cleanup & Validation
- [ ] Remove `ethers` dependencies, `sdk-ethers` feature flags, and legacy code paths. *(Requires successful Alloy regression + crate-by-crate dependency sweep.)*
- [ ] Delete temporary conversion helpers once Alloy is the single backend. *(Can drop `to_alloy_*` / `from_alloy_*` shims and state-override bridges post cutover.)*
- [ ] Ensure CI builds/tests run purely on Alloy. *(Update default feature set and CI matrix to Alloy-only.)*
- [ ] Perform full regression (perf tests, soak, integration labs). *(Schedule perf + soak campaigns against Alloy-backed binaries.)*

Keep this checklist updated as tasks complete.
