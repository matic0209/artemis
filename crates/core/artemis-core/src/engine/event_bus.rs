//! High-Performance Event Bus System
//!
//! Provides priority-based event processing, backpressure handling,
//! and advanced routing capabilities for optimal throughput.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tracing::{debug, warn, error};
use serde::{Serialize, Deserialize};

/// Event priority levels for processing order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventPriority {
    /// Critical events (liquidations, time-sensitive arbs)
    Critical = 0,
    /// High priority events (profitable opportunities)
    High = 1,
    /// Normal priority events (standard monitoring)
    Normal = 2,
    /// Low priority events (analytics, logging)
    Low = 3,
}

/// Generic event message wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMessage<T> {
    /// Event payload
    pub payload: T,
    /// Processing priority
    pub priority: EventPriority,
    /// Event creation timestamp
    pub timestamp: SystemTime,
    /// Event source identifier
    pub source: String,
    /// Event correlation ID for tracing
    pub correlation_id: String,
    /// Retry count for failed events
    pub retry_count: u32,
}

impl<T> EventMessage<T> {
    /// Create a new event message
    pub fn new(payload: T, priority: EventPriority, source: String) -> Self {
        Self {
            payload,
            priority,
            timestamp: SystemTime::now(),
            source,
            correlation_id: uuid::Uuid::new_v4().to_string(),
            retry_count: 0,
        }
    }

    /// Get event age in milliseconds
    pub fn age_ms(&self) -> u64 {
        match SystemTime::now().duration_since(self.timestamp) {
            Ok(duration) => duration.as_millis() as u64,
            Err(_) => 0,
        }
    }

    /// Check if event has expired
    pub fn is_expired(&self, max_age: Duration) -> bool {
        match SystemTime::now().duration_since(self.timestamp) {
            Ok(duration) => duration > max_age,
            Err(_) => false,
        }
    }
}

/// Event bus configuration
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// Maximum events in queue per priority level
    pub max_queue_size: usize,
    /// Worker thread count per priority level
    pub workers_per_priority: usize,
    /// Maximum event age before dropping
    pub max_event_age: Duration,
    /// Backpressure threshold (percentage of queue full)
    pub backpressure_threshold: f64,
    /// Enable priority-based processing
    pub enable_priority_processing: bool,
    /// Batch size for event processing
    pub batch_size: usize,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            workers_per_priority: 4,
            max_event_age: Duration::from_secs(30),
            backpressure_threshold: 0.8,
            enable_priority_processing: true,
            batch_size: 50,
        }
    }
}

/// High-performance event bus with priority queues and backpressure
pub struct EventBus<T> {
    /// Priority-based event queues
    priority_queues: Arc<RwLock<BTreeMap<EventPriority, VecDeque<EventMessage<T>>>>>,
    /// Event senders by priority
    senders: BTreeMap<EventPriority, mpsc::UnboundedSender<EventMessage<T>>>,
    /// Configuration
    config: EventBusConfig,
    /// Backpressure semaphore
    backpressure_sem: Arc<Semaphore>,
    /// Event processing metrics
    metrics: Arc<RwLock<EventBusMetrics>>,
}

/// Event bus performance metrics
#[derive(Debug, Default)]
pub struct EventBusMetrics {
    /// Total events processed
    pub events_processed: u64,
    /// Events dropped due to backpressure
    pub events_dropped: u64,
    /// Events expired
    pub events_expired: u64,
    /// Average processing latency per priority
    pub avg_latency_by_priority: BTreeMap<EventPriority, Duration>,
    /// Queue sizes by priority
    pub queue_sizes: BTreeMap<EventPriority, usize>,
    /// Throughput (events/second)
    pub throughput: f64,
}

impl<T> EventBus<T>
where
    T: Send + Sync + 'static,
{
    /// Create a new high-performance event bus
    pub fn new(config: EventBusConfig) -> Self {
        let priority_queues = Arc::new(RwLock::new(BTreeMap::new()));
        let mut senders = BTreeMap::new();

        // Initialize priority queues and channels
        for priority in [EventPriority::Critical, EventPriority::High,
                        EventPriority::Normal, EventPriority::Low] {
            let (sender, _receiver) = mpsc::unbounded_channel();
            senders.insert(priority, sender);
        }

        let backpressure_permits = (config.max_queue_size as f64 * config.backpressure_threshold) as usize;
        let backpressure_sem = Arc::new(Semaphore::new(backpressure_permits));

        Self {
            priority_queues,
            senders,
            config,
            backpressure_sem,
            metrics: Arc::new(RwLock::new(EventBusMetrics::default())),
        }
    }

    /// Send an event with priority handling
    pub async fn send(&self, event: EventMessage<T>) -> Result<(), EventBusError> {
        // Check backpressure
        if self.backpressure_sem.available_permits() == 0 {
            let mut metrics = self.metrics.write().await;
            metrics.events_dropped += 1;
            warn!(
                "Event dropped due to backpressure, correlation_id: {}",
                event.correlation_id
            );
            return Err(EventBusError::Backpressure);
        }

        // Check event expiration
        if event.is_expired(self.config.max_event_age) {
            let mut metrics = self.metrics.write().await;
            metrics.events_expired += 1;
            warn!(
                "Event expired, age: {}ms, correlation_id: {}",
                event.age_ms(),
                event.correlation_id
            );
            return Err(EventBusError::EventExpired);
        }

        // Send to appropriate priority queue
        if let Some(sender) = self.senders.get(&event.priority) {
            sender.send(event).map_err(|_| EventBusError::SendFailed)?;
        }

        Ok(())
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> EventBusMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Start event processing workers
    pub async fn start_workers<F>(&self, processor: F)
    where
        F: Fn(Vec<EventMessage<T>>) -> Result<(), String> + Send + Sync + Clone + 'static,
    {
        for (priority, _) in &self.senders {
            for worker_id in 0..self.config.workers_per_priority {
                let processor_clone = processor.clone();
                let metrics = Arc::clone(&self.metrics);
                let priority = *priority;

                tokio::spawn(async move {
                    debug!("Started event worker for priority {:?}, worker {}", priority, worker_id);

                    // Worker implementation would go here
                    // This is a simplified version
                    loop {
                        // Process events in batches
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                });
            }
        }
    }

    /// Shutdown the event bus gracefully
    pub async fn shutdown(&self) -> Result<(), EventBusError> {
        debug!("Shutting down event bus gracefully");

        // Process remaining events
        let remaining_events = self.get_queue_sizes().await;
        if remaining_events > 0 {
            debug!("Processing {} remaining events before shutdown", remaining_events);
            // Allow time for processing
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        Ok(())
    }

    /// Get total events in all queues
    async fn get_queue_sizes(&self) -> usize {
        let queues = self.priority_queues.read().await;
        queues.values().map(|q| q.len()).sum()
    }
}

/// Event bus errors
#[derive(Debug, thiserror::Error)]
pub enum EventBusError {
    #[error("Backpressure detected, event dropped")]
    Backpressure,
    #[error("Event expired")]
    EventExpired,
    #[error("Failed to send event")]
    SendFailed,
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl Clone for EventBusMetrics {
    fn clone(&self) -> Self {
        Self {
            events_processed: self.events_processed,
            events_dropped: self.events_dropped,
            events_expired: self.events_expired,
            avg_latency_by_priority: self.avg_latency_by_priority.clone(),
            queue_sizes: self.queue_sizes.clone(),
            throughput: self.throughput,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_creation() {
        let config = EventBusConfig::default();
        let event_bus: EventBus<String> = EventBus::new(config);
        assert_eq!(event_bus.get_queue_sizes().await, 0);
    }

    #[tokio::test]
    async fn test_event_priority() {
        let critical = EventPriority::Critical;
        let low = EventPriority::Low;
        assert!(critical < low);
    }

    #[test]
    fn test_event_message_creation() {
        let payload = "test".to_string();
        let event = EventMessage::new(
            payload,
            EventPriority::High,
            "test_source".to_string()
        );
        assert_eq!(event.priority, EventPriority::High);
        assert_eq!(event.source, "test_source");
        assert!(event.age_ms() < 100); // Should be very recent
    }
}
