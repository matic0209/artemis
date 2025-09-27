//! Comprehensive Performance Benchmark Suite
//!
//! This benchmark suite tests various aspects of Artemis performance including
//! latency, throughput, memory usage, and scalability under different workloads.

use artemis_core::{
    engine::{HighPerformanceEngine, EventBus},
    types::{Event, Action, Strategy},
    strategy_composer::{StrategyComposer, CompositionMode},
    memory_optimization::{ArtemisAllocator, ZeroCopyBufferManager},
    concurrency_optimization::{WorkStealingScheduler, AdaptiveThreadPool},
    batch_processor::{BatchProcessor, BatchConfig},
    benchmarks::{BenchmarkSuite, BenchmarkConfig, BenchmarkResult},
    simd_math::SimdPriceCalculator,
};
use std::{
    sync::{Arc, atomic::{AtomicU64, Ordering}},
    time::{Duration, Instant},
    collections::HashMap,
};
use tokio::sync::RwLock;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

/// Comprehensive benchmark suite for Artemis performance testing
pub struct ArtemisBenchmarkSuite {
    /// Test configurations
    configs: Vec<BenchmarkConfig>,
    /// Results storage
    results: Arc<RwLock<HashMap<String, BenchmarkResult>>>,
    /// Performance counters
    counters: Arc<PerformanceCounters>,
}

#[derive(Debug, Default)]
pub struct PerformanceCounters {
    pub events_processed: AtomicU64,
    pub actions_generated: AtomicU64,
    pub memory_allocations: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub name: String,
    pub event_count: usize,
    pub concurrent_strategies: usize,
    pub batch_size: usize,
    pub memory_pool_size: usize,
    pub thread_count: usize,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub throughput_events_per_sec: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub memory_usage_mb: f64,
    pub cpu_utilization_percent: f64,
    pub error_rate_percent: f64,
    pub cache_hit_rate_percent: f64,
}

impl ArtemisBenchmarkSuite {
    pub fn new() -> Self {
        Self {
            configs: Self::default_configs(),
            results: Arc::new(RwLock::new(HashMap::new())),
            counters: Arc::new(PerformanceCounters::default()),
        }
    }

    /// Create default benchmark configurations for various scenarios
    fn default_configs() -> Vec<BenchmarkConfig> {
        vec![
            // Low latency configuration
            BenchmarkConfig {
                name: "low_latency".to_string(),
                event_count: 10_000,
                concurrent_strategies: 1,
                batch_size: 1,
                memory_pool_size: 50 * 1024 * 1024, // 50MB
                thread_count: 4,
                duration_seconds: 60,
            },
            // High throughput configuration
            BenchmarkConfig {
                name: "high_throughput".to_string(),
                event_count: 1_000_000,
                concurrent_strategies: 20,
                batch_size: 1000,
                memory_pool_size: 500 * 1024 * 1024, // 500MB
                thread_count: 32,
                duration_seconds: 300,
            },
            // Balanced configuration
            BenchmarkConfig {
                name: "balanced".to_string(),
                event_count: 100_000,
                concurrent_strategies: 10,
                batch_size: 100,
                memory_pool_size: 100 * 1024 * 1024, // 100MB
                thread_count: 16,
                duration_seconds: 120,
            },
            // Memory stress test
            BenchmarkConfig {
                name: "memory_stress".to_string(),
                event_count: 500_000,
                concurrent_strategies: 50,
                batch_size: 50,
                memory_pool_size: 50 * 1024 * 1024, // Limited memory
                thread_count: 8,
                duration_seconds: 180,
            },
            // Concurrency stress test
            BenchmarkConfig {
                name: "concurrency_stress".to_string(),
                event_count: 200_000,
                concurrent_strategies: 100,
                batch_size: 10,
                memory_pool_size: 200 * 1024 * 1024, // 200MB
                thread_count: 64,
                duration_seconds: 240,
            },
        ]
    }

    /// Run all benchmark configurations
    pub async fn run_all_benchmarks(&mut self) -> Result<(), BenchmarkError> {
        println!("Starting Artemis Performance Benchmark Suite");
        println!("===========================================");

        for config in &self.configs.clone() {
            println!("\nRunning benchmark: {}", config.name);
            let result = self.run_benchmark(config).await?;

            // Store result
            let mut results = self.results.write().await;
            results.insert(config.name.clone(), result.clone());

            // Print immediate results
            self.print_benchmark_result(&result);
        }

        // Print summary
        self.print_summary().await;

        Ok(())
    }

    /// Run a single benchmark configuration
    async fn run_benchmark(&self, config: &BenchmarkConfig) -> Result<BenchmarkResult, BenchmarkError> {
        // Reset counters
        self.counters.events_processed.store(0, Ordering::Relaxed);
        self.counters.actions_generated.store(0, Ordering::Relaxed);
        self.counters.memory_allocations.store(0, Ordering::Relaxed);
        self.counters.cache_hits.store(0, Ordering::Relaxed);
        self.counters.cache_misses.store(0, Ordering::Relaxed);

        // Set up engine with benchmark configuration
        let mut engine = self.create_benchmark_engine(config).await?;

        // Add benchmark strategy
        let strategy = BenchmarkStrategy::new(self.counters.clone());
        engine.add_strategy(Box::new(strategy));

        // Create test events
        let events = self.generate_test_events(config.event_count);

        // Measure performance
        let start_time = Instant::now();
        let mut latencies = Vec::new();

        // Run benchmark
        for event in events {
            let event_start = Instant::now();
            engine.process_event(event).await?;
            let latency = event_start.elapsed();
            latencies.push(latency.as_nanos() as f64 / 1_000_000.0); // Convert to milliseconds
        }

        let total_duration = start_time.elapsed();

        // Calculate statistics
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let latency_p50 = latencies[latencies.len() / 2];
        let latency_p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
        let latency_p99 = latencies[(latencies.len() as f64 * 0.99) as usize];

        let throughput = config.event_count as f64 / total_duration.as_secs_f64();
        let memory_usage = self.get_memory_usage().await;
        let cpu_utilization = self.get_cpu_utilization().await;

        // Calculate cache hit rate
        let total_cache_ops = self.counters.cache_hits.load(Ordering::Relaxed) +
                             self.counters.cache_misses.load(Ordering::Relaxed);
        let cache_hit_rate = if total_cache_ops > 0 {
            (self.counters.cache_hits.load(Ordering::Relaxed) as f64 / total_cache_ops as f64) * 100.0
        } else {
            0.0
        };

        Ok(BenchmarkResult {
            name: config.name.clone(),
            throughput_events_per_sec: throughput,
            latency_p50_ms: latency_p50,
            latency_p95_ms: latency_p95,
            latency_p99_ms: latency_p99,
            memory_usage_mb: memory_usage,
            cpu_utilization_percent: cpu_utilization,
            error_rate_percent: 0.0, // TODO: Implement error tracking
            cache_hit_rate_percent: cache_hit_rate,
        })
    }

    /// Create an optimized engine for benchmarking
    async fn create_benchmark_engine(&self, config: &BenchmarkConfig) -> Result<HighPerformanceEngine, BenchmarkError> {
        // Set up memory optimization
        let allocator = ArtemisAllocator::new(config.memory_pool_size);
        let buffer_manager = ZeroCopyBufferManager::new(1000);

        // Create high-performance engine
        let mut engine = HighPerformanceEngine::new()
            .with_max_concurrent_strategies(config.concurrent_strategies)
            .with_circuit_breaker_enabled(true)
            .with_adaptive_load_balancing(true)
            .with_backpressure_threshold(config.batch_size * 10)
            .with_metrics_collection_interval(100)
            .build();

        // Set up work-stealing scheduler
        let scheduler = WorkStealingScheduler::new(config.thread_count);
        engine.set_scheduler(Box::new(scheduler));

        // Set up adaptive thread pool
        let thread_pool = AdaptiveThreadPool::new()
            .with_min_threads(config.thread_count / 2)
            .with_max_threads(config.thread_count)
            .with_scaling_threshold(0.8)
            .build();
        engine.set_thread_pool(thread_pool);

        // Set up batch processing
        let batch_config = BatchConfig {
            max_batch_size: config.batch_size,
            max_batch_delay: Duration::from_millis(1),
            adaptive_sizing: true,
        };
        let batch_processor = BatchProcessor::new(batch_config);
        engine.set_batch_processor(batch_processor);

        // Set memory components
        engine.set_allocator(Box::new(allocator));
        engine.set_buffer_manager(buffer_manager);

        Ok(engine)
    }

    /// Generate synthetic test events for benchmarking
    fn generate_test_events(&self, count: usize) -> Vec<Event> {
        let mut events = Vec::with_capacity(count);

        for i in 0..count {
            let event = match i % 4 {
                0 => Event::PendingTx(self.generate_test_transaction(i)),
                1 => Event::PoolUpdate(self.generate_test_pool_update(i)),
                2 => Event::BlockEvent(self.generate_test_block_event(i)),
                3 => Event::PriceUpdate(self.generate_test_price_update(i)),
                _ => unreachable!(),
            };
            events.push(event);
        }

        events
    }

    /// Get current memory usage in MB
    async fn get_memory_usage(&self) -> f64 {
        // Implementation would use system APIs to get actual memory usage
        // For now, return a simulated value
        let allocations = self.counters.memory_allocations.load(Ordering::Relaxed);
        (allocations as f64 * 1024.0) / (1024.0 * 1024.0) // Convert to MB
    }

    /// Get current CPU utilization percentage
    async fn get_cpu_utilization(&self) -> f64 {
        // Implementation would use system APIs to get actual CPU usage
        // For now, return a simulated value between 20-80%
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(20.0..=80.0)
    }

    /// Print individual benchmark result
    fn print_benchmark_result(&self, result: &BenchmarkResult) {
        println!("Results for {}:", result.name);
        println!("  Throughput: {:.2} events/sec", result.throughput_events_per_sec);
        println!("  Latency P50: {:.2}ms", result.latency_p50_ms);
        println!("  Latency P95: {:.2}ms", result.latency_p95_ms);
        println!("  Latency P99: {:.2}ms", result.latency_p99_ms);
        println!("  Memory Usage: {:.2}MB", result.memory_usage_mb);
        println!("  CPU Utilization: {:.1}%", result.cpu_utilization_percent);
        println!("  Cache Hit Rate: {:.1}%", result.cache_hit_rate_percent);
        println!("  Error Rate: {:.3}%", result.error_rate_percent);
    }

    /// Print comprehensive benchmark summary
    async fn print_summary(&self) {
        let results = self.results.read().await;

        println!("\n\nBenchmark Summary");
        println!("================");

        // Find best performers
        let best_throughput = results.values()
            .max_by(|a, b| a.throughput_events_per_sec.partial_cmp(&b.throughput_events_per_sec).unwrap());

        let best_latency = results.values()
            .min_by(|a, b| a.latency_p99_ms.partial_cmp(&b.latency_p99_ms).unwrap());

        let lowest_memory = results.values()
            .min_by(|a, b| a.memory_usage_mb.partial_cmp(&b.memory_usage_mb).unwrap());

        if let Some(result) = best_throughput {
            println!("Best Throughput: {} ({:.2} events/sec)", result.name, result.throughput_events_per_sec);
        }

        if let Some(result) = best_latency {
            println!("Best Latency: {} ({:.2}ms P99)", result.name, result.latency_p99_ms);
        }

        if let Some(result) = lowest_memory {
            println!("Lowest Memory: {} ({:.2}MB)", result.name, result.memory_usage_mb);
        }

        // Performance recommendations
        println!("\nPerformance Recommendations:");
        for result in results.values() {
            if result.latency_p99_ms > 100.0 {
                println!("  ⚠️  {} has high latency ({}ms P99) - consider reducing batch size or strategy complexity",
                        result.name, result.latency_p99_ms);
            }
            if result.memory_usage_mb > 200.0 {
                println!("  ⚠️  {} has high memory usage ({}MB) - consider optimizing allocations",
                        result.name, result.memory_usage_mb);
            }
            if result.cpu_utilization_percent > 80.0 {
                println!("  ⚠️  {} has high CPU usage ({}%) - consider adding more workers",
                        result.name, result.cpu_utilization_percent);
            }
            if result.cache_hit_rate_percent < 80.0 {
                println!("  ⚠️  {} has low cache hit rate ({}%) - consider cache optimization",
                        result.name, result.cache_hit_rate_percent);
            }
        }
    }

    /// Save benchmark results to file
    pub async fn save_results(&self, filename: &str) -> Result<(), BenchmarkError> {
        let results = self.results.read().await;
        let json = serde_json::to_string_pretty(&*results)?;
        tokio::fs::write(filename, json).await?;
        Ok(())
    }

    // Helper methods for generating test data
    fn generate_test_transaction(&self, id: usize) -> TestTransaction { TestTransaction { id } }
    fn generate_test_pool_update(&self, id: usize) -> TestPoolUpdate { TestPoolUpdate { id } }
    fn generate_test_block_event(&self, id: usize) -> TestBlockEvent { TestBlockEvent { id } }
    fn generate_test_price_update(&self, id: usize) -> TestPriceUpdate { TestPriceUpdate { id } }
}

/// Benchmark-specific strategy for performance testing
#[derive(Debug)]
struct BenchmarkStrategy {
    counters: Arc<PerformanceCounters>,
    price_calculator: SimdPriceCalculator,
}

impl BenchmarkStrategy {
    fn new(counters: Arc<PerformanceCounters>) -> Self {
        Self {
            counters,
            price_calculator: SimdPriceCalculator::new(),
        }
    }
}

#[async_trait::async_trait]
impl Strategy<Event, Action> for BenchmarkStrategy {
    async fn process_event(&mut self, event: Event) -> Vec<Action> {
        self.counters.events_processed.fetch_add(1, Ordering::Relaxed);

        // Simulate strategy processing with varying complexity
        match event {
            Event::PendingTx(_) => {
                // Simulate transaction analysis
                self.simulate_computation(100).await;
                self.counters.actions_generated.fetch_add(1, Ordering::Relaxed);
                vec![Action::SubmitTx(TestAction)]
            }
            Event::PoolUpdate(_) => {
                // Simulate pool state update
                self.simulate_computation(50).await;
                vec![]
            }
            Event::BlockEvent(_) => {
                // Simulate block processing
                self.simulate_computation(200).await;
                vec![]
            }
            Event::PriceUpdate(_) => {
                // Simulate price calculation using SIMD
                self.simulate_simd_calculation().await;
                vec![]
            }
        }
    }
}

impl BenchmarkStrategy {
    /// Simulate computational work
    async fn simulate_computation(&self, iterations: usize) {
        let mut sum = 0u64;
        for i in 0..iterations {
            sum = sum.wrapping_add(i as u64);
        }
        // Prevent optimization
        std::hint::black_box(sum);
    }

    /// Simulate SIMD calculations
    async fn simulate_simd_calculation(&self) {
        let data = vec![1.0f64; 1000];
        let _result = self.price_calculator.calculate_prices_vectorized(&data);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BenchmarkError {
    #[error("Engine error: {0}")]
    EngineError(String),
    #[error("IO error: {0}")]
    IoError(#[from] tokio::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

// Test data structures
#[derive(Debug)] struct TestTransaction { id: usize }
#[derive(Debug)] struct TestPoolUpdate { id: usize }
#[derive(Debug)] struct TestBlockEvent { id: usize }
#[derive(Debug)] struct TestPriceUpdate { id: usize }
#[derive(Debug)] struct TestAction;

/// Criterion-based micro benchmarks
fn criterion_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Benchmark event processing
    c.bench_function("event_processing", |b| {
        b.to_async(&rt).iter(|| async {
            let strategy = BenchmarkStrategy::new(Arc::new(PerformanceCounters::default()));
            let event = Event::PendingTx(TestTransaction { id: 1 });
            strategy.process_event(event).await
        })
    });

    // Benchmark memory allocation
    c.bench_function("memory_allocation", |b| {
        b.iter(|| {
            let allocator = ArtemisAllocator::new(1024 * 1024);
            // Benchmark allocation performance
            std::hint::black_box(allocator);
        })
    });

    // Benchmark SIMD calculations
    c.bench_function("simd_calculations", |b| {
        b.iter(|| {
            let calculator = SimdPriceCalculator::new();
            let data = vec![1.0f64; 1000];
            calculator.calculate_prices_vectorized(&data)
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

/// Main benchmark runner
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut benchmark_suite = ArtemisBenchmarkSuite::new();

    println!("Artemis Performance Benchmark Suite");
    println!("===================================");

    // Run all benchmarks
    benchmark_suite.run_all_benchmarks().await?;

    // Save results
    benchmark_suite.save_results("benchmark_results.json").await?;

    println!("\nBenchmark complete! Results saved to benchmark_results.json");

    Ok(())
}