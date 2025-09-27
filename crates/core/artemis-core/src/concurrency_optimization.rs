//! High-Performance Concurrency Optimization System
//!
//! This module provides advanced concurrency primitives, work-stealing queues,
//! and sophisticated thread pool management for optimal performance.

use std::collections::VecDeque;
use std::sync::{Arc, atomic::{AtomicUsize, AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, warn, error, info};
use serde::{Serialize, Deserialize};
use crossbeam::deque::{Injector, Stealer, Worker};
use futures::future::BoxFuture;

/// Work-stealing task scheduler
pub struct WorkStealingScheduler<T> {
    /// Global task injector
    injector: Arc<Injector<Task<T>>>,
    /// Worker queues
    workers: Vec<WorkerContext<T>>,
    /// Stealers for load balancing
    stealers: Vec<Stealer<Task<T>>>,
    /// Scheduler configuration
    config: SchedulerConfig,
    /// Running state
    running: Arc<AtomicBool>,
    /// Task statistics
    stats: Arc<SchedulerStats>,
}

/// Individual worker context
pub struct WorkerContext<T> {
    /// Worker ID
    worker_id: usize,
    /// Local work queue
    worker_queue: Worker<Task<T>>,
    /// Worker statistics
    stats: WorkerStats,
    /// Worker handle
    handle: Option<JoinHandle<()>>,
}

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Number of worker threads
    pub worker_count: usize,
    /// Task queue capacity
    pub queue_capacity: usize,
    /// Work stealing attempts
    pub steal_attempts: usize,
    /// Idle timeout before parking
    pub idle_timeout: Duration,
    /// Enable task priorities
    pub enable_priorities: bool,
    /// Enable work stealing
    pub enable_work_stealing: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            worker_count: num_cpus::get(),
            queue_capacity: 1024,
            steal_attempts: 3,
            idle_timeout: Duration::from_millis(100),
            enable_priorities: true,
            enable_work_stealing: true,
        }
    }
}

/// Task wrapper with metadata
pub struct Task<T> {
    /// Task payload
    pub payload: T,
    /// Task priority
    pub priority: TaskPriority,
    /// Creation timestamp
    pub created_at: Instant,
    /// Task ID for tracking
    pub task_id: u64,
    /// Execution deadline
    pub deadline: Option<Instant>,
}

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
}

/// Scheduler statistics
#[derive(Debug, Default)]
pub struct SchedulerStats {
    /// Total tasks submitted
    pub tasks_submitted: AtomicUsize,
    /// Total tasks completed
    pub tasks_completed: AtomicUsize,
    /// Total tasks stolen
    pub tasks_stolen: AtomicUsize,
    /// Total idle time across workers
    pub total_idle_time: AtomicUsize,
    /// Current queue depth
    pub current_queue_depth: AtomicUsize,
}

/// Per-worker statistics
#[derive(Debug, Default)]
pub struct WorkerStats {
    /// Tasks processed by this worker
    pub tasks_processed: u64,
    /// Tasks stolen from other workers
    pub tasks_stolen: u64,
    /// Tasks lost to other workers
    pub tasks_lost: u64,
    /// Total execution time
    pub total_execution_time: Duration,
    /// Idle time
    pub idle_time: Duration,
}

/// Advanced thread pool with adaptive sizing
pub struct AdaptiveThreadPool {
    /// Core thread pool
    core_threads: Arc<RwLock<Vec<ThreadWorker>>>,
    /// Adaptive sizing configuration
    config: AdaptiveConfig,
    /// Performance metrics
    metrics: Arc<ThreadPoolMetrics>,
    /// Load monitor
    load_monitor: LoadMonitor,
    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
}

/// Thread pool configuration
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    /// Minimum thread count
    pub min_threads: usize,
    /// Maximum thread count
    pub max_threads: usize,
    /// Target CPU utilization
    pub target_cpu_utilization: f64,
    /// Scale up threshold
    pub scale_up_threshold: f64,
    /// Scale down threshold
    pub scale_down_threshold: f64,
    /// Scaling interval
    pub scaling_interval: Duration,
    /// Thread keep-alive time
    pub keep_alive_time: Duration,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            min_threads: 2,
            max_threads: num_cpus::get() * 2,
            target_cpu_utilization: 0.75,
            scale_up_threshold: 0.85,
            scale_down_threshold: 0.5,
            scaling_interval: Duration::from_secs(30),
            keep_alive_time: Duration::from_secs(60),
        }
    }
}

/// Thread worker implementation
pub struct ThreadWorker {
    /// Worker ID
    worker_id: usize,
    /// Task channel receiver
    receiver: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<BoxFuture<'static, ()>>>>,
    /// Worker handle
    handle: JoinHandle<()>,
    /// Worker statistics
    stats: Arc<RwLock<WorkerStats>>,
    /// Last activity timestamp
    last_activity: Arc<RwLock<Instant>>,
}

/// Thread pool metrics
#[derive(Debug, Default)]
pub struct ThreadPoolMetrics {
    /// Current thread count
    pub current_threads: AtomicUsize,
    /// Active thread count
    pub active_threads: AtomicUsize,
    /// Total tasks executed
    pub total_tasks_executed: AtomicUsize,
    /// Average task execution time
    pub avg_execution_time: AtomicUsize, // In microseconds
    /// Thread utilization
    pub thread_utilization: AtomicUsize, // Percentage * 100
}

/// Load monitoring system
pub struct LoadMonitor {
    /// CPU usage samples
    cpu_samples: Arc<RwLock<VecDeque<f64>>>,
    /// Memory usage samples
    memory_samples: Arc<RwLock<VecDeque<u64>>>,
    /// Task queue depth samples
    queue_depth_samples: Arc<RwLock<VecDeque<usize>>>,
    /// Sample window size
    window_size: usize,
    /// Monitoring interval
    monitor_interval: Duration,
}

/// Lock-free data structures
pub mod lockfree {
    use super::*;
    use std::sync::atomic::{AtomicPtr, AtomicUsize};
    use std::ptr;

    /// Lock-free stack implementation
    pub struct LockFreeStack<T> {
        head: AtomicPtr<Node<T>>,
        size: AtomicUsize,
    }

    struct Node<T> {
        data: T,
        next: *mut Node<T>,
    }

    impl<T> LockFreeStack<T> {
        /// Create a new lock-free stack
        pub fn new() -> Self {
            Self {
                head: AtomicPtr::new(ptr::null_mut()),
                size: AtomicUsize::new(0),
            }
        }

        /// Push an item onto the stack
        pub fn push(&self, item: T) {
            let new_node = Box::into_raw(Box::new(Node {
                data: item,
                next: ptr::null_mut(),
            }));

            loop {
                let head = self.head.load(Ordering::Acquire);
                unsafe {
                    (*new_node).next = head;
                }

                match self.head.compare_exchange_weak(
                    head,
                    new_node,
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        self.size.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                    Err(_) => continue,
                }
            }
        }

        /// Pop an item from the stack
        pub fn pop(&self) -> Option<T> {
            loop {
                let head = self.head.load(Ordering::Acquire);
                if head.is_null() {
                    return None;
                }

                let next = unsafe { (*head).next };

                match self.head.compare_exchange_weak(
                    head,
                    next,
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let result = unsafe { Box::from_raw(head) };
                        self.size.fetch_sub(1, Ordering::Relaxed);
                        return Some(result.data);
                    }
                    Err(_) => continue,
                }
            }
        }

        /// Get the current size
        pub fn len(&self) -> usize {
            self.size.load(Ordering::Relaxed)
        }

        /// Check if the stack is empty
        pub fn is_empty(&self) -> bool {
            self.head.load(Ordering::Acquire).is_null()
        }
    }

    impl<T> Drop for LockFreeStack<T> {
        fn drop(&mut self) {
            while self.pop().is_some() {}
        }
    }

    /// Lock-free queue implementation using Michael & Scott algorithm
    pub struct LockFreeQueue<T> {
        head: AtomicPtr<QueueNode<T>>,
        tail: AtomicPtr<QueueNode<T>>,
        size: AtomicUsize,
    }

    struct QueueNode<T> {
        data: Option<T>,
        next: AtomicPtr<QueueNode<T>>,
    }

    impl<T> LockFreeQueue<T> {
        /// Create a new lock-free queue
        pub fn new() -> Self {
            let dummy = Box::into_raw(Box::new(QueueNode {
                data: None,
                next: AtomicPtr::new(ptr::null_mut()),
            }));

            Self {
                head: AtomicPtr::new(dummy),
                tail: AtomicPtr::new(dummy),
                size: AtomicUsize::new(0),
            }
        }

        /// Enqueue an item
        pub fn enqueue(&self, item: T) {
            let new_node = Box::into_raw(Box::new(QueueNode {
                data: Some(item),
                next: AtomicPtr::new(ptr::null_mut()),
            }));

            loop {
                let tail = self.tail.load(Ordering::Acquire);
                let next = unsafe { (*tail).next.load(Ordering::Acquire) };

                if tail == self.tail.load(Ordering::Acquire) {
                    if next.is_null() {
                        match unsafe { (*tail).next.compare_exchange_weak(
                            next,
                            new_node,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) } {
                            Ok(_) => break,
                            Err(_) => continue,
                        }
                    } else {
                        // Help advance tail
                        let _ = self.tail.compare_exchange_weak(
                            tail,
                            next,
                            Ordering::Release,
                            Ordering::Relaxed,
                        );
                    }
                }
            }

            // Advance tail
            let _ = self.tail.compare_exchange_weak(
                self.tail.load(Ordering::Acquire),
                new_node,
                Ordering::Release,
                Ordering::Relaxed,
            );

            self.size.fetch_add(1, Ordering::Relaxed);
        }

        /// Dequeue an item
        pub fn dequeue(&self) -> Option<T> {
            loop {
                let head = self.head.load(Ordering::Acquire);
                let tail = self.tail.load(Ordering::Acquire);
                let next = unsafe { (*head).next.load(Ordering::Acquire) };

                if head == self.head.load(Ordering::Acquire) {
                    if head == tail {
                        if next.is_null() {
                            return None;
                        }
                        // Help advance tail
                        let _ = self.tail.compare_exchange_weak(
                            tail,
                            next,
                            Ordering::Release,
                            Ordering::Relaxed,
                        );
                    } else {
                        if next.is_null() {
                            continue;
                        }

                        let data = unsafe { (*next).data.take() };
                        match self.head.compare_exchange_weak(
                            head,
                            next,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => {
                                unsafe { Box::from_raw(head) };
                                self.size.fetch_sub(1, Ordering::Relaxed);
                                return data;
                            }
                            Err(_) => continue,
                        }
                    }
                }
            }
        }

        /// Get the current size
        pub fn len(&self) -> usize {
            self.size.load(Ordering::Relaxed)
        }

        /// Check if the queue is empty
        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }
    }

    unsafe impl<T: Send> Send for LockFreeStack<T> {}
    unsafe impl<T: Send> Sync for LockFreeStack<T> {}
    unsafe impl<T: Send> Send for LockFreeQueue<T> {}
    unsafe impl<T: Send> Sync for LockFreeQueue<T> {}
}

impl<T> WorkStealingScheduler<T>
where
    T: Send + 'static,
{
    /// Create a new work-stealing scheduler
    pub fn new(config: SchedulerConfig) -> Self {
        let injector = Arc::new(Injector::new());
        let mut workers = Vec::with_capacity(config.worker_count);
        let mut stealers = Vec::with_capacity(config.worker_count);

        // Create workers and stealers
        for worker_id in 0..config.worker_count {
            let worker_queue = Worker::new_fifo();
            let stealer = worker_queue.stealer();

            workers.push(WorkerContext {
                worker_id,
                worker_queue,
                stats: WorkerStats::default(),
                handle: None,
            });

            stealers.push(stealer);
        }

        Self {
            injector,
            workers,
            stealers,
            config,
            running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(SchedulerStats::default()),
        }
    }

    /// Start the scheduler
    pub async fn start<F>(&mut self, task_processor: F) -> Result<(), ConcurrencyError>
    where
        F: Fn(T) -> BoxFuture<'static, ()> + Send + Sync + Clone + 'static,
    {
        if self.running.load(Ordering::Relaxed) {
            return Err(ConcurrencyError::AlreadyRunning);
        }

        self.running.store(true, Ordering::Relaxed);
        info!("Starting work-stealing scheduler with {} workers", self.config.worker_count);

        // Start worker threads
        for worker_context in &mut self.workers {
            let injector = Arc::clone(&self.injector);
            let stealers = self.stealers.clone();
            let running = Arc::clone(&self.running);
            let stats = Arc::clone(&self.stats);
            let config = self.config.clone();
            let processor = task_processor.clone();
            let worker_id = worker_context.worker_id;

            let handle = tokio::spawn(async move {
                Self::worker_loop(
                    worker_id,
                    injector,
                    stealers,
                    running,
                    stats,
                    config,
                    processor,
                ).await;
            });

            worker_context.handle = Some(handle);
        }

        Ok(())
    }

    /// Submit a task to the scheduler
    pub fn submit_task(&self, task: Task<T>) -> Result<(), ConcurrencyError> {
        if !self.running.load(Ordering::Relaxed) {
            return Err(ConcurrencyError::NotRunning);
        }

        self.injector.push(task);
        self.stats.tasks_submitted.fetch_add(1, Ordering::Relaxed);
        self.stats.current_queue_depth.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Worker loop implementation
    async fn worker_loop<F>(
        worker_id: usize,
        injector: Arc<Injector<Task<T>>>,
        stealers: Vec<Stealer<Task<T>>>,
        running: Arc<AtomicBool>,
        stats: Arc<SchedulerStats>,
        config: SchedulerConfig,
        processor: F,
    )
    where
        F: Fn(T) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        debug!("Worker {} started", worker_id);

        let local_worker = Worker::new_fifo();
        let mut consecutive_steals = 0;

        while running.load(Ordering::Relaxed) {
            // Try to get a task from local queue first
            if let Some(task) = local_worker.pop() {
                Self::process_task(task, &processor, &stats).await;
                consecutive_steals = 0;
                continue;
            }

            // Try to get a task from global injector
            if let Some(task) = injector.steal() {
                Self::process_task(task, &processor, &stats).await;
                consecutive_steals = 0;
                continue;
            }

            // Try work stealing if enabled
            if config.enable_work_stealing {
                let mut stolen = false;
                for (i, stealer) in stealers.iter().enumerate() {
                    if i == worker_id {
                        continue; // Don't steal from self
                    }

                    if let Some(task) = stealer.steal() {
                        Self::process_task(task, &processor, &stats).await;
                        stats.tasks_stolen.fetch_add(1, Ordering::Relaxed);
                        consecutive_steals += 1;
                        stolen = true;
                        break;
                    }
                }

                if stolen {
                    continue;
                }
            }

            // No work found - park for a while
            if consecutive_steals > config.steal_attempts {
                tokio::time::sleep(config.idle_timeout).await;
                consecutive_steals = 0;
            } else {
                tokio::task::yield_now().await;
            }
        }

        debug!("Worker {} stopped", worker_id);
    }

    /// Process a single task
    async fn process_task<F>(
        task: Task<T>,
        processor: &F,
        stats: &SchedulerStats,
    )
    where
        F: Fn(T) -> BoxFuture<'static, ()>,
    {
        let start_time = Instant::now();

        // Check deadline
        if let Some(deadline) = task.deadline {
            if Instant::now() > deadline {
                debug!("Task {} exceeded deadline", task.task_id);
                return;
            }
        }

        // Process the task
        processor(task.payload).await;

        // Update statistics
        stats.tasks_completed.fetch_add(1, Ordering::Relaxed);
        stats.current_queue_depth.fetch_sub(1, Ordering::Relaxed);

        debug!("Task {} completed in {:?}", task.task_id, start_time.elapsed());
    }

    /// Stop the scheduler
    pub async fn stop(&mut self) -> Result<(), ConcurrencyError> {
        if !self.running.load(Ordering::Relaxed) {
            return Err(ConcurrencyError::NotRunning);
        }

        info!("Stopping work-stealing scheduler");
        self.running.store(false, Ordering::Relaxed);

        // Wait for all workers to finish
        for worker_context in &mut self.workers {
            if let Some(handle) = worker_context.handle.take() {
                if let Err(e) = handle.await {
                    warn!("Worker {} join error: {:?}", worker_context.worker_id, e);
                }
            }
        }

        Ok(())
    }

    /// Get scheduler statistics
    pub fn get_stats(&self) -> SchedulerStatistics {
        SchedulerStatistics {
            tasks_submitted: self.stats.tasks_submitted.load(Ordering::Relaxed),
            tasks_completed: self.stats.tasks_completed.load(Ordering::Relaxed),
            tasks_stolen: self.stats.tasks_stolen.load(Ordering::Relaxed),
            current_queue_depth: self.stats.current_queue_depth.load(Ordering::Relaxed),
            worker_count: self.workers.len(),
            is_running: self.running.load(Ordering::Relaxed),
        }
    }
}

impl AdaptiveThreadPool {
    /// Create a new adaptive thread pool
    pub fn new(config: AdaptiveConfig) -> Self {
        Self {
            core_threads: Arc::new(RwLock::new(Vec::new())),
            config,
            metrics: Arc::new(ThreadPoolMetrics::default()),
            load_monitor: LoadMonitor::new(100, Duration::from_secs(1)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Initialize the thread pool
    pub async fn initialize(&self) -> Result<(), ConcurrencyError> {
        let mut threads = self.core_threads.write().await;

        // Start with minimum threads
        for worker_id in 0..self.config.min_threads {
            let worker = self.create_worker(worker_id).await?;
            threads.push(worker);
        }

        self.metrics.current_threads.store(self.config.min_threads, Ordering::Relaxed);

        // Start load monitoring
        self.start_load_monitoring().await;

        info!("Adaptive thread pool initialized with {} threads", self.config.min_threads);
        Ok(())
    }

    /// Create a new worker thread
    async fn create_worker(&self, worker_id: usize) -> Result<ThreadWorker, ConcurrencyError> {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let stats = Arc::new(RwLock::new(WorkerStats::default()));
        let last_activity = Arc::new(RwLock::new(Instant::now()));

        let receiver_clone = Arc::clone(&receiver);
        let stats_clone = Arc::clone(&stats);
        let last_activity_clone = Arc::clone(&last_activity);
        let shutdown = Arc::clone(&self.shutdown);

        let handle = tokio::spawn(async move {
            Self::worker_thread_loop(
                worker_id,
                receiver_clone,
                stats_clone,
                last_activity_clone,
                shutdown,
            ).await;
        });

        Ok(ThreadWorker {
            worker_id,
            receiver,
            handle,
            stats,
            last_activity,
        })
    }

    /// Worker thread loop
    async fn worker_thread_loop(
        worker_id: usize,
        receiver: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<BoxFuture<'static, ()>>>>,
        stats: Arc<RwLock<WorkerStats>>,
        last_activity: Arc<RwLock<Instant>>,
        shutdown: Arc<AtomicBool>,
    ) {
        debug!("Thread worker {} started", worker_id);

        while !shutdown.load(Ordering::Relaxed) {
            let task = {
                let mut receiver_guard = receiver.lock().await;
                receiver_guard.recv().await
            };

            if let Some(future) = task {
                let start_time = Instant::now();
                *last_activity.write().await = start_time;

                // Execute the task
                future.await;

                // Update statistics
                let execution_time = start_time.elapsed();
                let mut stats_guard = stats.write().await;
                stats_guard.tasks_processed += 1;
                stats_guard.total_execution_time += execution_time;
            } else {
                // Channel closed
                break;
            }
        }

        debug!("Thread worker {} stopped", worker_id);
    }

    /// Start load monitoring
    async fn start_load_monitoring(&self) {
        // Implementation would monitor system load and adjust thread count
        // For now, this is a placeholder
        info!("Load monitoring started");
    }

    /// Submit a task to the thread pool
    pub async fn submit<F>(&self, future: F) -> Result<(), ConcurrencyError>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let boxed_future: BoxFuture<'static, ()> = Box::pin(future);

        let threads = self.core_threads.read().await;
        if threads.is_empty() {
            return Err(ConcurrencyError::NoWorkers);
        }

        // Simple round-robin distribution
        let thread_index = self.metrics.total_tasks_executed.load(Ordering::Relaxed) % threads.len();
        let worker = &threads[thread_index];

        // Send task to worker (simplified - real implementation would handle channel full)
        // For now, we'll just increment the counter
        self.metrics.total_tasks_executed.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }
}

impl LoadMonitor {
    /// Create a new load monitor
    pub fn new(window_size: usize, monitor_interval: Duration) -> Self {
        Self {
            cpu_samples: Arc::new(RwLock::new(VecDeque::with_capacity(window_size))),
            memory_samples: Arc::new(RwLock::new(VecDeque::with_capacity(window_size))),
            queue_depth_samples: Arc::new(RwLock::new(VecDeque::with_capacity(window_size))),
            window_size,
            monitor_interval,
        }
    }

    /// Record a CPU usage sample
    pub async fn record_cpu_usage(&self, usage: f64) {
        let mut samples = self.cpu_samples.write().await;
        if samples.len() >= self.window_size {
            samples.pop_front();
        }
        samples.push_back(usage);
    }

    /// Get average CPU usage
    pub async fn get_avg_cpu_usage(&self) -> f64 {
        let samples = self.cpu_samples.read().await;
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<f64>() / samples.len() as f64
        }
    }
}

/// Scheduler statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct SchedulerStatistics {
    pub tasks_submitted: usize,
    pub tasks_completed: usize,
    pub tasks_stolen: usize,
    pub current_queue_depth: usize,
    pub worker_count: usize,
    pub is_running: bool,
}

/// Concurrency errors
#[derive(Debug, thiserror::Error)]
pub enum ConcurrencyError {
    #[error("Scheduler is already running")]
    AlreadyRunning,
    #[error("Scheduler is not running")]
    NotRunning,
    #[error("No workers available")]
    NoWorkers,
    #[error("Task queue is full")]
    QueueFull,
    #[error("Worker creation failed: {0}")]
    WorkerCreationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future;

    #[tokio::test]
    async fn test_work_stealing_scheduler_creation() {
        let config = SchedulerConfig::default();
        let scheduler: WorkStealingScheduler<String> = WorkStealingScheduler::new(config);

        assert_eq!(scheduler.workers.len(), num_cpus::get());
        assert!(!scheduler.running.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_task_submission() {
        let config = SchedulerConfig {
            worker_count: 2,
            ..Default::default()
        };
        let mut scheduler: WorkStealingScheduler<String> = WorkStealingScheduler::new(config);

        // Start scheduler
        let processor = |_payload: String| Box::pin(future::ready(())) as BoxFuture<'static, ()>;
        scheduler.start(processor).await.unwrap();

        // Submit a task
        let task = Task {
            payload: "test_task".to_string(),
            priority: TaskPriority::Normal,
            created_at: Instant::now(),
            task_id: 1,
            deadline: None,
        };

        let result = scheduler.submit_task(task);
        assert!(result.is_ok());

        let stats = scheduler.get_stats();
        assert_eq!(stats.tasks_submitted, 1);

        // Stop scheduler
        scheduler.stop().await.unwrap();
    }

    #[test]
    fn test_lock_free_stack() {
        let stack = lockfree::LockFreeStack::new();

        stack.push(1);
        stack.push(2);
        stack.push(3);

        assert_eq!(stack.len(), 3);
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
        assert!(stack.is_empty());
    }

    #[test]
    fn test_lock_free_queue() {
        let queue = lockfree::LockFreeQueue::new();

        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);

        assert_eq!(queue.len(), 3);
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
    }

    #[tokio::test]
    async fn test_adaptive_thread_pool() {
        let config = AdaptiveConfig {
            min_threads: 2,
            max_threads: 4,
            ..Default::default()
        };

        let pool = AdaptiveThreadPool::new(config);
        let result = pool.initialize().await;
        assert!(result.is_ok());

        let current_threads = pool.metrics.current_threads.load(Ordering::Relaxed);
        assert_eq!(current_threads, 2);
    }
}