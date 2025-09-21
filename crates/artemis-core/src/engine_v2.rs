use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use tokio_stream::StreamExt;
use tracing::{error, info, warn};

use crate::types::{Collector, Executor, Strategy};

/// Event routing configuration
#[derive(Debug, Clone)]
pub struct EventRoutingConfig {
    /// Buffer size for each event type
    pub buffer_sizes: HashMap<String, usize>,
    /// Priority levels for different event types
    pub event_priorities: HashMap<String, u8>,
    /// Enable parallel strategy execution
    pub parallel_strategies: bool,
}

impl Default for EventRoutingConfig {
    fn default() -> Self {
        let mut buffer_sizes = HashMap::new();
        buffer_sizes.insert("NewBlock".to_string(), 512);
        buffer_sizes.insert("Transaction".to_string(), 2048);
        buffer_sizes.insert("OpenseaOrder".to_string(), 1024);
        
        let mut event_priorities = HashMap::new();
        event_priorities.insert("NewBlock".to_string(), 255); // Highest priority
        event_priorities.insert("OpenseaOrder".to_string(), 200);
        event_priorities.insert("Transaction".to_string(), 100);
        
        Self {
            buffer_sizes,
            event_priorities,
            parallel_strategies: true,
        }
    }
}

/// Enhanced engine with optimized event processing
pub struct EngineV2<E, A> {
    /// Event collectors
    collectors: Vec<Box<dyn Collector<E>>>,
    /// Strategy processors  
    strategies: Vec<Box<dyn Strategy<E, A>>>,
    /// Action executors
    executors: Vec<Box<dyn Executor<A>>>,
    /// Routing configuration
    config: EventRoutingConfig,
}

impl<E, A> EngineV2<E, A>
where
    E: Send + Sync + Clone + 'static,
    A: Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            collectors: Vec::new(),
            strategies: Vec::new(), 
            executors: Vec::new(),
            config: EventRoutingConfig::default(),
        }
    }

    pub fn with_config(mut self, config: EventRoutingConfig) -> Self {
        self.config = config;
        self
    }

    pub fn add_collector(&mut self, collector: Box<dyn Collector<E>>) {
        self.collectors.push(collector);
    }

    pub fn add_strategy(&mut self, strategy: Box<dyn Strategy<E, A>>) {
        self.strategies.push(strategy);
    }

    pub fn add_executor(&mut self, executor: Box<dyn Executor<A>>) {
        self.executors.push(executor);
    }

    /// Run the optimized engine with parallel processing
    pub async fn run(self) -> Result<JoinSet<()>, Box<dyn std::error::Error>> {
        let (event_sender, _) = broadcast::channel::<E>(4096);
        let (action_sender, _) = broadcast::channel::<A>(4096);
        
        let mut set = JoinSet::new();

        // Start executors with batch processing
        for executor in self.executors {
            let mut action_receiver = action_sender.subscribe();
            set.spawn(async move {
                info!("Starting optimized executor...");
                let mut batch = Vec::new();
                let mut batch_timer = tokio::time::interval(std::time::Duration::from_millis(10));
                
                loop {
                    tokio::select! {
                        // Collect actions into batches
                        action_result = action_receiver.recv() => {
                            match action_result {
                                Ok(action) => {
                                    batch.push(action);
                                    // Process batch when it reaches optimal size
                                    if batch.len() >= 10 {
                                        Self::process_action_batch(&executor, &mut batch).await;
                                    }
                                }
                                Err(e) => error!("Error receiving action: {}", e),
                            }
                        }
                        // Process remaining actions on timer
                        _ = batch_timer.tick() => {
                            if !batch.is_empty() {
                                Self::process_action_batch(&executor, &mut batch).await;
                            }
                        }
                    }
                }
            });
        }

        // Start strategies with parallel processing
        if self.config.parallel_strategies {
            for mut strategy in self.strategies {
                let mut event_receiver = event_sender.subscribe();
                let action_sender = action_sender.clone();
                
                set.spawn(async move {
                    info!("Starting parallel strategy...");
                    if let Err(err) = strategy.sync_state().await {
                        error!("Strategy failed to sync state: {}", err);
                        return;
                    }
                    
                    loop {
                        match event_receiver.recv().await {
                            Ok(event) => {
                                let start = std::time::Instant::now();
                                
                                // Process event with timeout protection
                                let actions = tokio::time::timeout(
                                    std::time::Duration::from_millis(500),
                                    strategy.process_event(event)
                                ).await;
                                
                                match actions {
                                    Ok(actions) => {
                                        for action in actions {
                                            if let Err(e) = action_sender.send(action) {
                                                error!("Failed to send action: {}", e);
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        warn!("Strategy processing timeout");
                                        metrics::counter!("artemis.engine.strategy_timeouts").increment(1);
                                    }
                                }
                                
                                let elapsed = start.elapsed();
                                metrics::histogram!("artemis.engine.strategy_process_time_v2")
                                    .record(elapsed.as_millis() as f64);
                            }
                            Err(e) => error!("Error receiving event: {}", e),
                        }
                    }
                });
            }
        }

        // Start collectors with smart buffering
        for collector in self.collectors {
            let event_sender = event_sender.clone();
            set.spawn(async move {
                info!("Starting optimized collector...");
                match collector.get_event_stream().await {
                    Ok(mut stream) => {
                        while let Some(event) = stream.next().await {
                            if let Err(e) = event_sender.send(event) {
                                if event_sender.receiver_count() == 0 {
                                    break; // No more receivers
                                }
                                warn!("Failed to send event: {}", e);
                            }
                        }
                    }
                    Err(e) => error!("Failed to get event stream: {}", e),
                }
            });
        }

        Ok(set)
    }

    async fn process_action_batch(
        executor: &Box<dyn Executor<A>>,
        batch: &mut Vec<A>,
    ) {
        if batch.is_empty() {
            return;
        }

        let batch_size = batch.len();
        let start = std::time::Instant::now();

        // Process actions concurrently within the batch
        let futures: Vec<_> = batch.drain(..)
            .map(|action| executor.execute(action))
            .collect();

        let results = futures::future::join_all(futures).await;
        
        let mut success_count = 0;
        for result in results {
            match result {
                Ok(_) => success_count += 1,
                Err(e) => error!("Batch execution error: {}", e),
            }
        }

        let elapsed = start.elapsed();
        metrics::histogram!("artemis.engine.batch_execution_time")
            .record(elapsed.as_millis() as f64);
        metrics::counter!("artemis.engine.batch_actions_processed")
            .increment(batch_size as u64);
        metrics::counter!("artemis.engine.batch_actions_succeeded")
            .increment(success_count);
    }
}
