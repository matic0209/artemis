# Artemis Performance Test Plan

This document describes the verification strategy for the upcoming performance optimizations across the Artemis pipeline. It focuses on throughput, latency, resource consumption, and functional correctness under load.

## 1. Test Environment
- **Rust toolchain:** stable toolchain defined by `rust-toolchain` (default stable) with `cargo` 1.72+.
- **Runtime:** Tokio multi-threaded runtime; tests run with `--test-threads=1` where deterministic ordering matters.
- **Ethereum backends:**
  - Local Anvil instances (spun up per test module) for deterministic block/tx streams.
  - Optional live RPC/WSS endpoints mocked via `ethers` `Provider` + `mockall` to avoid network variance.
- **Instrumentation:** tracing emits structured JSON, and a Prometheus exporter is mounted at `ARTEMIS_METRICS_ADDR` (defaults to `127.0.0.1:9898/metrics`) with engine/collector queue gauges and executor cache metrics wired through the `metrics` crate.

## 2. Metrics and Acceptance Thresholds
- **Event throughput:** minimum 5k events/sec sustained without message loss for synthetic collectors.
- **Latency (p99):**
  - Collector → Strategy enqueue latency < 50 ms.
  - Strategy → Executor enqueue latency < 30 ms.
- **Memory ceiling:** steady-state RSS growth < 10% versus baseline while processing identical loads.
- **CPU utilisation:** no single task pinned > 90% for >10 s on reference 8-core host.

## 3. Component-Level Test Cases

### 3.1 Engine & Channel Topology
1. **Broadcast Replacement Regression (unit/integration):**
   - Setup: spawn engine with mock collectors publishing 50k lightweight events.
   - Validate: zero event loss, each strategy receives ordered events, and total allocations decrease (tracked via `metrics::counter!`).
   - Tools: `cargo test engine::fanout_smoke -- --nocapture` with `loom`-backed concurrency test to detect deadlocks.
2. **Backpressure Behavior:**
   - Inject slow strategy consumer; assert collectors observe bounded wait times and event queue applies drop/compaction policy (verify via metrics + logs).
   - Ensure configurable thresholds applied correctly via environment overrides.
3. **Cold Start Parallelisation:**
   - Instrument `sync_state` futures; ensure total startup time reduces under 2 s in test harness by concurrently spinning collectors and measuring time to first processed event.

### 3.2 Collectors
1. **MempoolCollector Batched Dispatch:**
   - Use Anvil + synthetic tx flood (10k tx/min) and assert `CollectorMap` receives batches (size histogram recorded).
   - Verify queue never exceeds configured high-water mark; assert dropped-count metric increments when expected.
2. **OpenseaOrderCollector Resilience:**
   - Mock WebSocket stream with intermittent disconnects; ensure exponential backoff reconnects and no event duplication.
   - Validate gzip/deflate compression toggles increase effective throughput via measured bytes read.
3. **MevShareCollector SSE Handling:**
   - Feed SSE events at randomized bursts; confirm parser keeps latency < 20 ms/event and silently retries on malformed payloads.

### 3.3 Strategies
1. **OpenseaSudoArb Data Structures:**
   - Unit tests covering heap-based best-bid maintenance (push/pop/update); assert correctness and O(log n) scaling via benchmark harness (`cargo bench strategy::sudo_heap`).
2. **Concurrent Quote Fetching:**
   - Instrument multi-chunk RPC fetch; ensure concurrency limit respected and total time reduces proportionally (target 70% decrease vs serial baseline).
3. **Order Processing Pipeline:**
   - Integrate mocked OpenSea API returning cached responses; assert deduplication hits > 90% on repeated order hash submissions.
4. **MevShareUniArb Bundle Generation:**
   - Validate concurrent fill & signing reduces total build time < 100 ms for 14 bundle sizes; ensure signed bundles remain deterministic via snapshot tests.

### 3.4 Executors
1. **MempoolExecutor Gas Cache:**
   - Mock provider to count `estimate_gas` invocations; confirm identical template actions hit cache (<=1 RPC per template) and fall back gracefully on cache miss/invalidations.
2. **FlashbotsExecutor Parallel Simulation:**
   - Use stubbed Flashbots client capturing simulation+send timing; ensure concurrency gating respects relay limits and properly aggregates success/failure metrics.
3. **Alloy Executor Wiring:**
   - Add future integration tests once alloy provider support lands; currently placeholder to validate no-ops maintain idempotence.

## 4. Integration & End-to-End Tests
1. **Synthetic Full Pipeline Load:**
   - Compose collectors → strategies → executors with mocks; pump mixed events (mempool, MEV-Share, OpenSea) at scale.
   - Assertions: throughput/latency thresholds met, zero panics, metrics exported for queue depth, success counts.
2. **Realistic Scenario Replay:**
   - Replay captured mainnet traces (scrubbed) via fixture; verify identical action decisions pre/post-optimisation.
3. **Stress & Burn-In:**
   - 1-hour soak test using `cargo nextest run --release --features soak` hitting synthetic loads; monitor for memory leaks.

## 5. Tooling & Automation
- Extend `justfile` with `just perf-smoke`, `just soak`, `just bench` commands.
  - `just perf-smoke` exercises the hot paths under a single test thread.
  - `just bench` compiles and runs Criterion benchmarks when available (gracefully no-ops otherwise).
  - `just soak` runs long-running ignored tests in release mode for burn-in coverage.
- Integrate tests with CI (GitHub Actions) behind opt-in flag `PERF_TESTS=true` to avoid running heavy workloads by default.
- Produce `criterion` benchmark reports stored under `target/criterion` for regression tracking.

## 6. Reporting
- Collect metrics via `metrics-exporter-prometheus` (local) and summarise into Markdown after each run.
- Track baseline numbers before optimisation merge and ensure regression guardrails in CI (`cargo bench` compares against JSON baseline).
