//! Event Coordinator
//!
//! Coordinates events between the artemis-core event bus and the MEV arbitrage manager

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, error, debug};
use std::time::{Duration, Instant};

use crate::coordination::event_system::MEVEvent;
use crate::coordination::unified_manager::{UnifiedArbitrageManager, ArbitrageResult};

/// Event coordinator configuration
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// Maximum events to buffer
    pub max_buffer_size: usize,
    /// Processing timeout per event
    pub event_timeout: Duration,
    /// Enable event batching
    pub enable_batching: bool,
    /// Batch size for processing
    pub batch_size: usize,
    /// Batch timeout
    pub batch_timeout: Duration,
    /// Enable metrics collection
    pub enable_metrics: bool,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: 1000,
            event_timeout: Duration::from_secs(5),
            enable_batching: true,
            batch_size: 10,
            batch_timeout: Duration::from_millis(100),
            enable_metrics: true,
        }
    }
}

/// Event coordinator statistics
#[derive(Debug, Clone, Default)]
pub struct CoordinatorStats {
    pub total_events_received: u64,
    pub total_events_processed: u64,
    pub total_opportunities_found: u64,
    pub total_successful_executions: u64,
    pub total_failed_executions: u64,
    pub avg_processing_time_ms: f64,
    pub last_event_timestamp: Option<Instant>,
}

/// Event coordinator for MEV arbitrage
///
/// This coordinator bridges MEVEvent streams with the UnifiedArbitrageManager,
/// handling event buffering, batching, and result collection.
pub struct EventCoordinator {
    /// Configuration
    config: CoordinatorConfig,
    /// Arbitrage manager
    manager: Arc<RwLock<UnifiedArbitrageManager>>,
    /// Event receiver channel
    event_receiver: mpsc::UnboundedReceiver<MEVEvent>,
    /// Event sender (for external use)
    event_sender: mpsc::UnboundedSender<MEVEvent>,
    /// Result sender (for outputting results)
    result_sender: Option<mpsc::UnboundedSender<ArbitrageResult>>,
    /// Statistics
    stats: Arc<RwLock<CoordinatorStats>>,
}

impl EventCoordinator {
    /// Create new event coordinator
    pub fn new(
        config: CoordinatorConfig,
        manager: Arc<RwLock<UnifiedArbitrageManager>>,
    ) -> (Self, mpsc::UnboundedSender<MEVEvent>) {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let coordinator = Self {
            config,
            manager,
            event_receiver,
            event_sender: event_sender.clone(),
            result_sender: None,
            stats: Arc::new(RwLock::new(CoordinatorStats::default())),
        };

        (coordinator, event_sender)
    }

    /// Set result output channel
    pub fn with_result_sender(mut self, sender: mpsc::UnboundedSender<ArbitrageResult>) -> Self {
        self.result_sender = Some(sender);
        self
    }

    /// Get event sender for external use
    pub fn event_sender(&self) -> mpsc::UnboundedSender<MEVEvent> {
        self.event_sender.clone()
    }

    /// Get statistics
    pub async fn stats(&self) -> CoordinatorStats {
        self.stats.read().await.clone()
    }

    /// Run the event coordinator
    pub async fn run(mut self) -> Result<()> {
        info!("EventCoordinator: Starting event processing loop");

        let mut event_batch = Vec::with_capacity(self.config.batch_size);
        let mut last_batch_time = Instant::now();

        loop {
            // Try to receive event with timeout for batching
            let event_opt = if self.config.enable_batching {
                tokio::time::timeout(self.config.batch_timeout, self.event_receiver.recv()).await.ok().flatten()
            } else {
                self.event_receiver.recv().await
            };

            match event_opt {
                Some(event) => {
                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.total_events_received += 1;
                        stats.last_event_timestamp = Some(Instant::now());
                    }

                    if self.config.enable_batching {
                        event_batch.push(event);

                        // Process batch if full or timeout reached
                        let should_process = event_batch.len() >= self.config.batch_size
                            || last_batch_time.elapsed() >= self.config.batch_timeout;

                        if should_process && !event_batch.is_empty() {
                            self.process_event_batch(&mut event_batch).await?;
                            last_batch_time = Instant::now();
                        }
                    } else {
                        // Process immediately
                        self.process_single_event(event).await?;
                    }
                }
                None => {
                    // Channel closed or timeout - process any remaining batch
                    if !event_batch.is_empty() {
                        self.process_event_batch(&mut event_batch).await?;
                    }

                    if event_opt.is_none() && !self.config.enable_batching {
                        info!("EventCoordinator: Event channel closed, shutting down");
                        break;
                    }
                }
            }
        }

        info!("EventCoordinator: Stopped");
        Ok(())
    }

    /// Process a single event
    async fn process_single_event(&mut self, event: MEVEvent) -> Result<()> {
        let processing_start = Instant::now();

        debug!("Processing single event: {:?}", event);

        // Convert event to detection context
        let context = event.to_detection_context();

        // Process with manager
        let results = {
            let mut manager = self.manager.write().await;
            tokio::time::timeout(
                self.config.event_timeout,
                manager.process_arbitrage_cycle(&context)
            ).await??
        }; // Lock released here

        // Update stats and send results
        self.update_stats_and_send_results(results, processing_start).await;

        Ok(())
    }

    /// Process a batch of events
    async fn process_event_batch(&mut self, batch: &mut Vec<MEVEvent>) -> Result<()> {
        let batch_start = Instant::now();
        let batch_size = batch.len();

        info!("Processing event batch of size {}", batch_size);

        // Process each event in the batch
        // For now, we process sequentially; could be optimized for parallel processing
        for event in batch.drain(..) {
            let context = event.to_detection_context();

            let results_opt = {
                let mut manager = self.manager.write().await;
                tokio::time::timeout(
                    self.config.event_timeout,
                    manager.process_arbitrage_cycle(&context)
                ).await
            }; // Lock released here

            match results_opt {
                Ok(Ok(results)) => {
                    self.update_stats_and_send_results(results, batch_start).await;
                }
                Ok(Err(e)) => {
                    error!("Error processing event in batch: {}", e);
                }
                Err(_) => {
                    warn!("Event processing timeout in batch");
                }
            }
        }

        info!("Completed batch processing in {:?}", batch_start.elapsed());
        Ok(())
    }

    /// Update statistics and send results
    async fn update_stats_and_send_results(&mut self, results: Vec<ArbitrageResult>, start_time: Instant) {
        let processing_time = start_time.elapsed();

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_events_processed += 1;

            for result in &results {
                if result.success {
                    stats.total_successful_executions += 1;
                } else if result.execution_result.is_some() {
                    stats.total_failed_executions += 1;
                }

                // Count as opportunity if it has meaningful profit
                if result.opportunity.expected_profit > alloy_primitives::U256::ZERO {
                    stats.total_opportunities_found += 1;
                }
            }

            // Update rolling average
            let n = stats.total_events_processed as f64;
            stats.avg_processing_time_ms =
                (stats.avg_processing_time_ms * (n - 1.0) + processing_time.as_secs_f64() * 1000.0) / n;
        }

        // Send results if output channel configured
        if let Some(sender) = &self.result_sender {
            for result in results {
                if let Err(e) = sender.send(result) {
                    error!("Failed to send arbitrage result: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::unified_manager::ManagerConfig;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let manager_config = ManagerConfig::default();
        let manager = Arc::new(RwLock::new(
            UnifiedArbitrageManager::new(manager_config).await.unwrap()
        ));

        let config = CoordinatorConfig::default();
        let (coordinator, _sender) = EventCoordinator::new(config, manager);

        let stats = coordinator.stats().await;
        assert_eq!(stats.total_events_received, 0);
    }
}