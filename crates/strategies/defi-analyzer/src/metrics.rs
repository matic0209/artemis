//! DeFi Analyzer Performance Metrics

use std::time::{Duration, Instant};
use std::collections::HashMap;
use tracing::{info, debug};

/// Performance metrics collector
#[derive(Debug, Default)]
pub struct MetricsCollector {
    /// Analysis metrics
    analysis_metrics: HashMap<String, AnalysisMetrics>,
    /// System metrics
    system_metrics: SystemMetrics,
    /// Performance counters
    counters: PerformanceCounters,
}

/// Analysis metrics for a specific contract
#[derive(Debug, Default)]
pub struct AnalysisMetrics {
    /// Total analyses performed
    pub total_analyses: u64,
    /// Successful analyses
    pub successful_analyses: u64,
    /// Failed analyses
    pub failed_analyses: u64,
    /// Average analysis time
    pub avg_analysis_time: Duration,
    /// Total analysis time
    pub total_analysis_time: Duration,
    /// Opportunities found
    pub opportunities_found: u64,
    /// Inconsistencies found
    pub inconsistencies_found: u64,
    /// Cache hit rate
    pub cache_hit_rate: f64,
    /// Memory usage (bytes)
    pub memory_usage: usize,
}

/// System-wide metrics
#[derive(Debug, Default)]
pub struct SystemMetrics {
    /// Total memory usage (bytes)
    pub total_memory_usage: usize,
    /// CPU usage percentage
    pub cpu_usage_percentage: f64,
    /// Active analyses count
    pub active_analyses: u32,
    /// Queue length
    pub queue_length: u32,
    /// Error rate
    pub error_rate: f64,
}

/// Performance counters
#[derive(Debug, Default)]
pub struct PerformanceCounters {
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Symbolic execution calls
    pub symbolic_execution_calls: u64,
    /// Feature extraction calls
    pub feature_extraction_calls: u64,
    /// Documentation comparison calls
    pub documentation_comparison_calls: u64,
    /// Risk assessment calls
    pub risk_assessment_calls: u64,
    /// Network requests
    pub network_requests: u64,
    /// Network errors
    pub network_errors: u64,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self::default()
    }

    /// Record analysis start
    pub fn record_analysis_start(&mut self, contract_address: &str) -> Instant {
        debug!("Starting analysis for contract: {}", contract_address);
        Instant::now()
    }

    /// Record analysis completion
    pub fn record_analysis_completion(
        &mut self,
        contract_address: &str,
        start_time: Instant,
        success: bool,
        opportunities: u64,
        inconsistencies: u64,
    ) {
        let duration = start_time.elapsed();
        let metrics = self.analysis_metrics
            .entry(contract_address.to_string())
            .or_insert_with(AnalysisMetrics::default);

        metrics.total_analyses += 1;
        if success {
            metrics.successful_analyses += 1;
        } else {
            metrics.failed_analyses += 1;
        }

        metrics.total_analysis_time += duration;
        metrics.avg_analysis_time = Duration::from_millis(
            metrics.total_analysis_time.as_millis() as u64 / metrics.total_analyses
        );

        metrics.opportunities_found += opportunities;
        metrics.inconsistencies_found += inconsistencies;

        debug!(
            "Analysis completed for {}: {}ms, opportunities: {}, inconsistencies: {}",
            contract_address,
            duration.as_millis(),
            opportunities,
            inconsistencies
        );
    }

    /// Record cache hit
    pub fn record_cache_hit(&mut self) {
        self.counters.cache_hits += 1;
        self.update_cache_hit_rate();
    }

    /// Record cache miss
    pub fn record_cache_miss(&mut self) {
        self.counters.cache_misses += 1;
        self.update_cache_hit_rate();
    }

    /// Record symbolic execution call
    pub fn record_symbolic_execution(&mut self) {
        self.counters.symbolic_execution_calls += 1;
    }

    /// Record feature extraction call
    pub fn record_feature_extraction(&mut self) {
        self.counters.feature_extraction_calls += 1;
    }

    /// Record documentation comparison call
    pub fn record_documentation_comparison(&mut self) {
        self.counters.documentation_comparison_calls += 1;
    }

    /// Record risk assessment call
    pub fn record_risk_assessment(&mut self) {
        self.counters.risk_assessment_calls += 1;
    }

    /// Record network request
    pub fn record_network_request(&mut self) {
        self.counters.network_requests += 1;
    }

    /// Record network error
    pub fn record_network_error(&mut self) {
        self.counters.network_errors += 1;
    }

    /// Update system metrics
    pub fn update_system_metrics(&mut self, memory_usage: usize, cpu_usage: f64, active_analyses: u32, queue_length: u32) {
        self.system_metrics.total_memory_usage = memory_usage;
        self.system_metrics.cpu_usage_percentage = cpu_usage;
        self.system_metrics.active_analyses = active_analyses;
        self.system_metrics.queue_length = queue_length;

        // Calculate error rate
        let total_analyses: u64 = self.analysis_metrics.values()
            .map(|m| m.total_analyses)
            .sum();
        let failed_analyses: u64 = self.analysis_metrics.values()
            .map(|m| m.failed_analyses)
            .sum();

        if total_analyses > 0 {
            self.system_metrics.error_rate = (failed_analyses as f64 / total_analyses as f64) * 100.0;
        }
    }

    /// Update cache hit rate
    fn update_cache_hit_rate(&mut self) {
        let total_cache_operations = self.counters.cache_hits + self.counters.cache_misses;
        if total_cache_operations > 0 {
            let hit_rate = (self.counters.cache_hits as f64 / total_cache_operations as f64) * 100.0;
            
            // Update all contract metrics with the same hit rate
            for metrics in self.analysis_metrics.values_mut() {
                metrics.cache_hit_rate = hit_rate;
            }
        }
    }

    /// Get metrics for a specific contract
    pub fn get_contract_metrics(&self, contract_address: &str) -> Option<&AnalysisMetrics> {
        self.analysis_metrics.get(contract_address)
    }

    /// Get system metrics
    pub fn get_system_metrics(&self) -> &SystemMetrics {
        &self.system_metrics
    }

    /// Get performance counters
    pub fn get_counters(&self) -> &PerformanceCounters {
        &self.counters
    }

    /// Get summary statistics
    pub fn get_summary(&self) -> MetricsSummary {
        let total_analyses: u64 = self.analysis_metrics.values()
            .map(|m| m.total_analyses)
            .sum();
        let total_successful: u64 = self.analysis_metrics.values()
            .map(|m| m.successful_analyses)
            .sum();
        let total_opportunities: u64 = self.analysis_metrics.values()
            .map(|m| m.opportunities_found)
            .sum();
        let total_inconsistencies: u64 = self.analysis_metrics.values()
            .map(|m| m.inconsistencies_found)
            .sum();

        let success_rate = if total_analyses > 0 {
            (total_successful as f64 / total_analyses as f64) * 100.0
        } else {
            0.0
        };

        MetricsSummary {
            total_analyses,
            success_rate,
            total_opportunities,
            total_inconsistencies,
            cache_hit_rate: self.counters.cache_hits as f64 / 
                (self.counters.cache_hits + self.counters.cache_misses).max(1) as f64 * 100.0,
            error_rate: self.system_metrics.error_rate,
            memory_usage_mb: self.system_metrics.total_memory_usage as f64 / 1024.0 / 1024.0,
            cpu_usage: self.system_metrics.cpu_usage_percentage,
        }
    }

    /// Log performance report
    pub fn log_performance_report(&self) {
        let summary = self.get_summary();
        
        info!("📊 DeFi Analyzer Performance Report");
        info!("├─ Total Analyses: {}", summary.total_analyses);
        info!("├─ Success Rate: {:.2}%", summary.success_rate);
        info!("├─ Opportunities Found: {}", summary.total_opportunities);
        info!("├─ Inconsistencies Found: {}", summary.total_inconsistencies);
        info!("├─ Cache Hit Rate: {:.2}%", summary.cache_hit_rate);
        info!("├─ Error Rate: {:.2}%", summary.error_rate);
        info!("├─ Memory Usage: {:.2} MB", summary.memory_usage_mb);
        info!("└─ CPU Usage: {:.2}%", summary.cpu_usage);
    }
}

/// Metrics summary
#[derive(Debug)]
pub struct MetricsSummary {
    pub total_analyses: u64,
    pub success_rate: f64,
    pub total_opportunities: u64,
    pub total_inconsistencies: u64,
    pub cache_hit_rate: f64,
    pub error_rate: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage: f64,
}
