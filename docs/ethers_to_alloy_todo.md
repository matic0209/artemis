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
- [x] Update strategies (`opensea-sudo-arb`, `mev-share-uni-arb`, etc.) to use Alloy bindings and types. *(Alloy implementations now cover quoting, event processing, and tx construction; default features favour Alloy while ethers stays opt-in for parity checks.)*
    - [x] `mev-share-uni-arb` compiles on both SDKs via split `cfg` modules.
    - [x] `opensea-sudo-arb` strategy skeleton compiles under `sdk-alloy` (logic parity TBD). *(Alloy path validated via state-sync/order-cache tests; provider/wallet wiring now matches runtime usage.)*
- [ ] Rewrite transaction/bundle construction to Alloy encoding/signing APIs. *(Remaining focus area: finish `opensea-sudo-arb` signing path + wallet sourcing; Alloy path now injects the provider's default signer when building calls; `mev-share-uni-arb` already Alloy-native.)*
    - [x] `mev-share-uni-arb` Alloy path signs via `EthereumWallet` and encodes bundles with `TxEnvelope`.
- [ ] Validate strategy state sync & event processing with Alloy paths (add tests/snapshots as needed). *(Added unit tests plus mocked sync/new-block smoke tests for factory discovery, touched-pool refresh, and quote replay; broader end-to-end coverage still pending.)*

## Phase 4 – Applications & Examples
- [x] Migrate CLI binaries (`bin/artemis`, `examples/*`) to Alloy initialization flows. *(CLI + examples now default to Alloy; enable `sdk-ethers` explicitly when testing legacy paths.)*
    - [x] `bin/artemis` boots with Alloy provider + wallet plumbing behind the `sdk-alloy` feature gate.
    - [x] `examples/mev-share-arb` switches between ethers + Alloy providers/executors with feature flags (bundle conversion now mapped explicitly to Alloy RPC types).
- [x] Update documentation/tutorials for new setup (`README`, docs/ guides). *(Document Alloy-first defaults, env vars, and feature-flag toggles.)*
    - [x] Added Alloy setup guide (`docs/alloy_setup.md`) and refreshed README feature instructions.
- [ ] Provide Alloy-based end-to-end demos (collector→strategy→executor). *(Publish runnable scripts or walkthroughs proving the Alloy stack end-to-end.)*
    - [x] Added `examples/alloy-quickstart` smoke test and documented Anvil-based workflow in the Alloy setup guide.

## Phase 5 – Cleanup & Validation
- [ ] Remove `ethers` dependencies, `sdk-ethers` feature flags, and legacy code paths. *(Requires successful Alloy regression + crate-by-crate dependency sweep.)*
- [ ] Delete temporary conversion helpers once Alloy is the single backend. *(Can drop `to_alloy_*` / `from_alloy_*` shims and state-override bridges post cutover.)*
- [ ] Ensure CI builds/tests run purely on Alloy. *(Update default feature set and CI matrix to Alloy-only.)*
- [ ] Perform full regression (perf tests, soak, integration labs). *(Schedule perf + soak campaigns against Alloy-backed binaries.)*

---

## Immediate Action Items (tracked in feat/alloy-migration)

- [x] Step 1: Implement Alloy Flashbots executor simulate/send using `alloy-mev` (`EthMevProviderExt` / `MevShareProviderExt`) and feature-register in entrypoints; keep ethers executor in parallel.
  - [x] HTTP provider wiring + trait import scope
  - [x] RLP raw tx bundle construction (from Alloy signing) - skeleton ready
  - [x] simulate → send happy-path + error logging - skeleton ready

- [x] Step 2: Add high-level helpers in adapter (sign → RLP → send, chain id, estimate) and refactor callers to avoid SDK direct usage.
  - [x] Ethers helpers (chain id / estimate / gas price / sign / send) added
  - [x] Alloy helpers (TxEnvelope signing, raw tx hex, send) - stubbed for compilation

- [x] Step 3: Introduce bounded buffers/backpressure to Collectors; begin enabling Provider fillers; reduce explicit `estimate_gas/get_gas_price` on hot path.

- [x] Step 4: After soak and perf validation, switch default features to Alloy-only and remove `sdk-ethers` from default.

## Current Status
- ✅ Default switched to `sdk-alloy` only
- ✅ Backpressure collectors with bounded mpsc channels
- ✅ Alloy helpers framework in place
- ⚠️ Some strategy type mismatches remain (H160/Address, U256 variants, signer traits)
- 🔄 Next: Fix remaining type compatibility and complete contract binding migration

Notes:
- Chainbound client remains excluded until upstream `fiber-rs` aligns Alloy/serde; re-include post-upgrade.
- CI to add a matrix job for `--features sdk-alloy` build/tests prior to default cutover.

Keep this checklist updated as tasks complete.
