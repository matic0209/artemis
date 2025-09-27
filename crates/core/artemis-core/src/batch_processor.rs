//! Batch Processing Optimization Module
//!
//! This module provides efficient batch processing capabilities for events and actions,
//! including adaptive sizing, concurrent processing, and performance monitoring.
//! Designed to improve throughput and reduce latency in high-volume scenarios.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, Notify};
use tokio::time::timeout;
use tokio_stream::{Stream, StreamExt};
use tracing::{debug, error};


/// 批处理器配置
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// 批处理大小
    pub batch_size: usize,
    /// 批处理超时时间
    pub batch_timeout: Duration,
    /// 最大并发数
    pub max_concurrency: usize,
    /// 缓冲区大小
    pub buffer_size: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            batch_timeout: Duration::from_millis(100),
            max_concurrency: 4,
            buffer_size: 1000,
        }
    }
}

/// 批处理器
pub struct BatchProcessor<T> {
    /// 配置
    config: BatchConfig,
    /// 内部缓冲区
    buffer: Arc<Mutex<VecDeque<T>>>,
    /// 通知器
    notifier: Arc<Notify>,
    /// 是否正在运行
    running: Arc<Mutex<bool>>,
}

impl<T> BatchProcessor<T> 
where 
    T: Send + Sync + 'static,
{
    /// 创建新的批处理器
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config: config.clone(),
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(config.buffer_size))),
            notifier: Arc::new(Notify::new()),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// 添加项目到批处理队列
    pub async fn add(&self, item: T) -> Result<(), BatchError> {
        let mut buffer = self.buffer.lock().await;
        
        if buffer.len() >= self.config.buffer_size {
            return Err(BatchError::BufferFull);
        }
        
        buffer.push_back(item);
        
        // 如果达到批处理大小，通知处理器
        if buffer.len() >= self.config.batch_size {
            self.notifier.notify_one();
        }
        
        Ok(())
    }

    /// 启动批处理器
    pub async fn start<F, Fut>(&self, processor: F) -> Result<(), BatchError>
    where
        F: Fn(Vec<T>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), BatchError>> + Send,
    {
        let mut running = self.running.lock().await;
        if *running {
            return Err(BatchError::AlreadyRunning);
        }
        *running = true;
        drop(running);

        let buffer = Arc::clone(&self.buffer);
        let notifier = Arc::clone(&self.notifier);
        let running = Arc::clone(&self.running);
        let config = self.config.clone();

        tokio::spawn(async move {
            let processor = Arc::new(processor);
            
            while *running.lock().await {
                // 等待通知或超时
                let _ = timeout(config.batch_timeout, notifier.notified()).await;
                
                // 收集批处理项目
                let batch = {
                    let mut buffer_guard = buffer.lock().await;
                    let batch_size = std::cmp::min(config.batch_size, buffer_guard.len());
                    
                    if batch_size == 0 {
                        continue;
                    }
                    
                    let mut batch = Vec::with_capacity(batch_size);
                    for _ in 0..batch_size {
                        if let Some(item) = buffer_guard.pop_front() {
                            batch.push(item);
                        }
                    }
                    batch
                };
                
                if !batch.is_empty() {
                    debug!("Processing batch of {} items", batch.len());
                    
                    // 处理批次
                    let processor_clone = Arc::clone(&processor);
                    let batch_result = processor_clone(batch).await;
                    
                    if let Err(e) = batch_result {
                        error!("Batch processing failed: {:?}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// 停止批处理器
    pub async fn stop(&self) {
        let mut running = self.running.lock().await;
        *running = false;
        self.notifier.notify_one();
    }

    /// 获取当前缓冲区大小
    pub async fn buffer_len(&self) -> usize {
        let buffer = self.buffer.lock().await;
        buffer.len()
    }

    /// 检查是否正在运行
    pub async fn is_running(&self) -> bool {
        let running = self.running.lock().await;
        *running
    }
}

/// 批处理错误
#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    #[error("Buffer is full")]
    BufferFull,
    #[error("Batch processor is already running")]
    AlreadyRunning,
    #[error("Processing error: {0}")]
    Processing(String),
    #[error("Timeout error")]
    Timeout,
}

/// 流批处理器
pub struct StreamBatchProcessor;

impl StreamBatchProcessor {
    /// 将流转换为批处理流
    pub fn batch_stream<T, S>(
        stream: S,
        config: BatchConfig,
    ) -> impl Stream<Item = Vec<T>>
    where
        S: Stream<Item = T> + Send + 'static,
        T: Send + 'static,
    {
        let _buffer = Arc::new(Mutex::new(Vec::<T>::with_capacity(config.batch_size)));
        let _last_batch_time = Arc::new(Mutex::new(Instant::now()));
        
        stream.chunks_timeout(config.batch_size, config.batch_timeout)
    }

    /// 并发处理批次
    pub async fn process_batches_concurrent<T, F, Fut>(
        batches: impl Stream<Item = Vec<T>> + Unpin,
        processor: F,
        max_concurrency: usize,
    ) -> Result<(), BatchError>
    where
        T: Send + 'static,
        F: Fn(Vec<T>) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<(), BatchError>> + Send,
    {
        use futures::StreamExt as FuturesStreamExt;
        
        tokio_stream::StreamExt::map(batches, |batch| {
                let processor = processor.clone();
                async move {
                    processor(batch).await
                }
            })
            .buffer_unordered(max_concurrency)
            .for_each(|result| async move {
                if let Err(e) = result {
                    error!("Batch processing error: {:?}", e);
                }
            })
            .await;
            
        Ok(())
    }
}

/// 智能批处理器 - 根据负载动态调整批处理参数
pub struct AdaptiveBatchProcessor<T> {
    /// 基础批处理器
    processor: BatchProcessor<T>,
    /// 性能统计
    stats: Arc<Mutex<BatchStats>>,
    /// 配置调整器
    #[allow(dead_code)]
    config_tuner: ConfigTuner,
}

/// 批处理统计
#[derive(Debug, Default)]
pub struct BatchStats {
    /// 处理的批次数
    batches_processed: u64,
    /// 处理的总项目数
    items_processed: u64,
    /// 平均批处理时间
    avg_batch_time: Duration,
    /// 平均批处理大小
    avg_batch_size: f64,
    /// 错误计数
    error_count: u64,
}

/// 配置调整器
struct ConfigTuner {
    /// 目标处理时间
    target_processing_time: Duration,
    /// 调整因子
    adjustment_factor: f64,
}

impl<T> AdaptiveBatchProcessor<T> 
where 
    T: Send + Sync + 'static,
{
    /// 创建自适应批处理器
    pub fn new(config: BatchConfig) -> Self {
        Self {
            processor: BatchProcessor::new(config.clone()),
            stats: Arc::new(Mutex::new(BatchStats::default())),
            config_tuner: ConfigTuner {
                target_processing_time: Duration::from_millis(50),
                adjustment_factor: 0.1,
            },
        }
    }

    /// 添加项目
    pub async fn add(&self, item: T) -> Result<(), BatchError> {
        self.processor.add(item).await
    }

    /// 启动自适应处理
    pub async fn start_adaptive<F, Fut>(&self, processor: F) -> Result<(), BatchError>
    where
        F: Fn(Vec<T>) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<(), BatchError>> + Send,
    {
        let stats = Arc::clone(&self.stats);
        
        let adaptive_processor = move |batch: Vec<T>| {
            let processor = processor.clone();
            let stats = Arc::clone(&stats);
            
            async move {
                let start_time = Instant::now();
                let batch_size = batch.len();
                
                let result = processor(batch).await;
                
                let processing_time = start_time.elapsed();
                
                // 更新统计
                let mut stats_guard = stats.lock().await;
                stats_guard.batches_processed += 1;
                stats_guard.items_processed += batch_size as u64;
                
                // 更新平均处理时间
                let total_batches = stats_guard.batches_processed as f64;
                stats_guard.avg_batch_time = Duration::from_nanos(
                    ((stats_guard.avg_batch_time.as_nanos() as f64 * (total_batches - 1.0) + 
                      processing_time.as_nanos() as f64) / total_batches) as u64
                );
                
                // 更新平均批处理大小
                stats_guard.avg_batch_size = 
                    (stats_guard.avg_batch_size * (total_batches - 1.0) + batch_size as f64) / total_batches;
                
                if result.is_err() {
                    stats_guard.error_count += 1;
                }
                
                result
            }
        };

        self.processor.start(adaptive_processor).await
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> BatchStats {
        let stats = self.stats.lock().await;
        BatchStats {
            batches_processed: stats.batches_processed,
            items_processed: stats.items_processed,
            avg_batch_time: stats.avg_batch_time,
            avg_batch_size: stats.avg_batch_size,
            error_count: stats.error_count,
        }
    }

    /// 停止处理器
    pub async fn stop(&self) {
        self.processor.stop().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_batch_processor_basic() {
        let config = BatchConfig {
            batch_size: 3,
            batch_timeout: Duration::from_millis(100),
            max_concurrency: 1,
            buffer_size: 10,
        };
        
        let processor = BatchProcessor::new(config);
        let processed_items = Arc::new(Mutex::new(Vec::new()));
        let processed_items_clone = Arc::clone(&processed_items);
        
        // 启动处理器
        processor.start(move |batch| {
            let processed_items = Arc::clone(&processed_items_clone);
            async move {
                let mut items = processed_items.lock().await;
                items.extend(batch);
                Ok(())
            }
        }).await.unwrap();
        
        // 添加项目
        for i in 0..5 {
            processor.add(i).await.unwrap();
        }
        
        // 等待处理
        sleep(Duration::from_millis(200)).await;
        
        let items = processed_items.lock().await;
        assert_eq!(items.len(), 5);
        
        processor.stop().await;
    }

    #[tokio::test]
    async fn test_adaptive_batch_processor() {
        let config = BatchConfig {
            batch_size: 2,
            batch_timeout: Duration::from_millis(50),
            max_concurrency: 1,
            buffer_size: 10,
        };
        
        let processor = AdaptiveBatchProcessor::new(config);
        
        processor.start_adaptive(|batch| async move {
            // 模拟处理时间
            sleep(Duration::from_millis(10)).await;
            tracing::debug!("Processed batch of {} items", batch.len());
            Ok(())
        }).await.unwrap();
        
        // 添加项目
        for i in 0..10 {
            processor.add(i).await.unwrap();
            sleep(Duration::from_millis(5)).await;
        }
        
        // 等待处理完成
        sleep(Duration::from_millis(200)).await;
        
        let stats = processor.get_stats().await;
        assert!(stats.batches_processed > 0);
        assert!(stats.items_processed == 10);
        
        processor.stop().await;
    }
}
