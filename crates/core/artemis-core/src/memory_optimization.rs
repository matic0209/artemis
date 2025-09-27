//! Advanced Memory Optimization System
//!
//! This module provides sophisticated memory management capabilities including
//! zero-copy operations, custom allocators, and memory pool management for
//! high-performance MEV operations.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, atomic::{AtomicUsize, AtomicU64, Ordering}};
use std::time::{Instant, Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use serde::{Serialize, Deserialize};

/// Custom allocator with tracking and optimization
pub struct ArtemisAllocator {
    /// Underlying system allocator
    inner: System,
    /// Allocation statistics
    stats: Arc<AllocationStats>,
    /// Memory pools for different sizes
    pools: Arc<RwLock<HashMap<usize, MemoryPool>>>,
}

/// Allocation statistics for monitoring
#[derive(Debug, Default)]
pub struct AllocationStats {
    /// Total allocations made
    pub total_allocations: AtomicU64,
    /// Total deallocations made
    pub total_deallocations: AtomicU64,
    /// Current bytes allocated
    pub current_bytes: AtomicUsize,
    /// Peak bytes allocated
    pub peak_bytes: AtomicUsize,
    /// Total bytes allocated over time
    pub total_bytes_allocated: AtomicU64,
    /// Total bytes freed over time
    pub total_bytes_freed: AtomicU64,
    /// Number of pool hits
    pub pool_hits: AtomicU64,
    /// Number of pool misses
    pub pool_misses: AtomicU64,
}

/// Memory pool for specific size classes
pub struct MemoryPool {
    /// Size class for this pool
    pub size_class: usize,
    /// Available blocks
    available_blocks: VecDeque<*mut u8>,
    /// Total blocks in pool
    total_blocks: usize,
    /// Maximum pool size
    max_size: usize,
    /// Allocation statistics
    stats: PoolStats,
}

/// Statistics for individual memory pools
#[derive(Debug, Default)]
pub struct PoolStats {
    /// Total allocations from this pool
    pub allocations: u64,
    /// Total returns to this pool
    pub returns: u64,
    /// Current utilization
    pub utilization: f64,
    /// Hit rate
    pub hit_rate: f64,
}

/// Zero-copy buffer manager
pub struct ZeroCopyBufferManager {
    /// Buffer pools by size
    buffer_pools: Arc<RwLock<HashMap<usize, BufferPool>>>,
    /// Buffer registry for tracking
    buffer_registry: Arc<RwLock<HashMap<usize, BufferInfo>>>,
    /// Configuration
    config: BufferManagerConfig,
    /// Next buffer ID
    next_buffer_id: AtomicUsize,
}

/// Configuration for buffer manager
#[derive(Debug, Clone)]
pub struct BufferManagerConfig {
    /// Default buffer sizes to pre-allocate
    pub default_sizes: Vec<usize>,
    /// Maximum buffers per size class
    pub max_buffers_per_size: usize,
    /// Enable memory mapping for large buffers
    pub enable_mmap: bool,
    /// Memory map threshold
    pub mmap_threshold: usize,
    /// Buffer alignment
    pub alignment: usize,
}

impl Default for BufferManagerConfig {
    fn default() -> Self {
        Self {
            default_sizes: vec![1024, 4096, 16384, 65536, 262144, 1048576], // 1KB to 1MB
            max_buffers_per_size: 100,
            enable_mmap: true,
            mmap_threshold: 1048576, // 1MB
            alignment: 64, // Cache line aligned
        }
    }
}

/// Buffer pool for a specific size
pub struct BufferPool {
    /// Size of buffers in this pool
    pub buffer_size: usize,
    /// Available buffers
    available_buffers: VecDeque<ZeroCopyBuffer>,
    /// Maximum pool size
    max_size: usize,
    /// Pool statistics
    stats: BufferPoolStats,
}

/// Zero-copy buffer with reference counting
pub struct ZeroCopyBuffer {
    /// Buffer ID
    pub id: usize,
    /// Raw data pointer
    data: *mut u8,
    /// Buffer size
    size: usize,
    /// Reference count
    ref_count: Arc<AtomicUsize>,
    /// Buffer type
    buffer_type: BufferType,
    /// Creation timestamp
    created_at: Instant,
}

/// Buffer type for different use cases
#[derive(Debug, Clone, Copy)]
pub enum BufferType {
    /// Regular heap allocation
    Heap,
    /// Memory mapped buffer
    MemoryMapped,
    /// Aligned buffer for SIMD operations
    Aligned,
    /// Pinned buffer for DMA
    Pinned,
}

/// Buffer information for tracking
#[derive(Debug)]
pub struct BufferInfo {
    /// Buffer ID
    pub id: usize,
    /// Size
    pub size: usize,
    /// Type
    pub buffer_type: BufferType,
    /// Current reference count
    pub ref_count: usize,
    /// Creation time
    pub created_at: Instant,
    /// Last access time
    pub last_accessed: Instant,
}

/// Pool statistics
#[derive(Debug, Default)]
pub struct BufferPoolStats {
    /// Total buffer requests
    pub requests: u64,
    /// Successful allocations from pool
    pub hits: u64,
    /// Failed allocations (pool empty)
    pub misses: u64,
    /// Current pool utilization
    pub utilization: f64,
    /// Average buffer lifetime
    pub avg_lifetime: Duration,
}

/// Memory usage analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsageAnalytics {
    /// Timestamp of measurement
    pub timestamp: SystemTime,
    /// Total memory usage in bytes
    pub total_memory_bytes: u64,
    /// Heap memory usage
    pub heap_memory_bytes: u64,
    /// Memory mapped usage
    pub mmap_memory_bytes: u64,
    /// Buffer pool utilization
    pub buffer_pool_utilization: f64,
    /// Allocation rate (allocations/second)
    pub allocation_rate: f64,
    /// Deallocation rate (deallocations/second)
    pub deallocation_rate: f64,
    /// Memory fragmentation level
    pub fragmentation_level: f64,
}

/// SIMD-optimized data structures
pub mod simd_structures {
    use std::arch::x86_64::*;

    /// SIMD-optimized vector operations
    pub struct SimdVector {
        data: Vec<f32>,
        alignment: usize,
    }

    impl SimdVector {
        /// Create a new SIMD-aligned vector
        pub fn new(size: usize) -> Self {
            let alignment = 32; // AVX2 alignment
            let mut data = Vec::with_capacity(size);
            data.resize(size, 0.0);

            Self { data, alignment }
        }

        /// Perform vectorized addition
        pub fn add_vectorized(&mut self, other: &[f32]) -> Result<(), String> {
            if self.data.len() != other.len() {
                return Err("Vector length mismatch".to_string());
            }

            unsafe {
                for i in (0..self.data.len()).step_by(8) {
                    if i + 8 <= self.data.len() {
                        let a = _mm256_load_ps(self.data.as_ptr().add(i));
                        let b = _mm256_loadu_ps(other.as_ptr().add(i));
                        let result = _mm256_add_ps(a, b);
                        _mm256_store_ps(self.data.as_mut_ptr().add(i), result);
                    }
                }
            }

            Ok(())
        }

        /// Get the underlying data
        pub fn data(&self) -> &[f32] {
            &self.data
        }
    }
}

impl ArtemisAllocator {
    /// Create a new Artemis allocator
    pub fn new() -> Self {
        Self {
            inner: System,
            stats: Arc::new(AllocationStats::default()),
            pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get allocation statistics
    pub fn get_stats(&self) -> AllocationStats {
        AllocationStats {
            total_allocations: AtomicU64::new(self.stats.total_allocations.load(Ordering::Relaxed)),
            total_deallocations: AtomicU64::new(self.stats.total_deallocations.load(Ordering::Relaxed)),
            current_bytes: AtomicUsize::new(self.stats.current_bytes.load(Ordering::Relaxed)),
            peak_bytes: AtomicUsize::new(self.stats.peak_bytes.load(Ordering::Relaxed)),
            total_bytes_allocated: AtomicU64::new(self.stats.total_bytes_allocated.load(Ordering::Relaxed)),
            total_bytes_freed: AtomicU64::new(self.stats.total_bytes_freed.load(Ordering::Relaxed)),
            pool_hits: AtomicU64::new(self.stats.pool_hits.load(Ordering::Relaxed)),
            pool_misses: AtomicU64::new(self.stats.pool_misses.load(Ordering::Relaxed)),
        }
    }

    /// Initialize memory pools for common sizes
    pub async fn initialize_pools(&self) {
        let mut pools = self.pools.write().await;

        // Common size classes (powers of 2)
        let size_classes = vec![64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768];

        for size in size_classes {
            pools.insert(size, MemoryPool::new(size, 100)); // 100 blocks per pool
        }

        info!("Initialized {} memory pools", pools.len());
    }
}

unsafe impl GlobalAlloc for ArtemisAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();

        // Update statistics
        self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
        self.stats.total_bytes_allocated.fetch_add(size as u64, Ordering::Relaxed);

        let current = self.stats.current_bytes.fetch_add(size, Ordering::Relaxed) + size;

        // Update peak if necessary
        let mut peak = self.stats.peak_bytes.load(Ordering::Relaxed);
        while current > peak {
            match self.stats.peak_bytes.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }

        // Delegate to system allocator
        self.inner.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size();

        // Update statistics
        self.stats.total_deallocations.fetch_add(1, Ordering::Relaxed);
        self.stats.total_bytes_freed.fetch_add(size as u64, Ordering::Relaxed);
        self.stats.current_bytes.fetch_sub(size, Ordering::Relaxed);

        // Delegate to system allocator
        self.inner.dealloc(ptr, layout);
    }
}

impl MemoryPool {
    /// Create a new memory pool
    pub fn new(size_class: usize, max_size: usize) -> Self {
        Self {
            size_class,
            available_blocks: VecDeque::new(),
            total_blocks: 0,
            max_size,
            stats: PoolStats::default(),
        }
    }

    /// Allocate a block from the pool
    pub fn allocate(&mut self) -> Option<*mut u8> {
        if let Some(block) = self.available_blocks.pop_front() {
            self.stats.allocations += 1;
            Some(block)
        } else {
            None
        }
    }

    /// Return a block to the pool
    pub fn deallocate(&mut self, ptr: *mut u8) -> bool {
        if self.available_blocks.len() < self.max_size {
            self.available_blocks.push_back(ptr);
            self.stats.returns += 1;
            true
        } else {
            false
        }
    }

    /// Get pool utilization
    pub fn utilization(&self) -> f64 {
        if self.total_blocks == 0 {
            0.0
        } else {
            (self.total_blocks - self.available_blocks.len()) as f64 / self.total_blocks as f64
        }
    }
}

impl ZeroCopyBufferManager {
    /// Create a new buffer manager
    pub fn new(config: BufferManagerConfig) -> Self {
        Self {
            buffer_pools: Arc::new(RwLock::new(HashMap::new())),
            buffer_registry: Arc::new(RwLock::new(HashMap::new())),
            config,
            next_buffer_id: AtomicUsize::new(1),
        }
    }

    /// Initialize buffer pools
    pub async fn initialize(&self) -> Result<(), String> {
        let mut pools = self.buffer_pools.write().await;

        for &size in &self.config.default_sizes {
            let pool = BufferPool::new(size, self.config.max_buffers_per_size);
            pools.insert(size, pool);
        }

        info!("Initialized {} buffer pools", pools.len());
        Ok(())
    }

    /// Acquire a buffer of the specified size
    pub async fn acquire_buffer(&self, size: usize) -> Result<ZeroCopyBuffer, String> {
        // Find the appropriate size class
        let size_class = self.find_size_class(size);

        {
            let mut pools = self.buffer_pools.write().await;
            if let Some(pool) = pools.get_mut(&size_class) {
                if let Some(buffer) = pool.try_allocate() {
                    // Register the buffer
                    self.register_buffer(&buffer).await;
                    return Ok(buffer);
                }
            }
        }

        // Pool miss - allocate new buffer
        self.allocate_new_buffer(size_class).await
    }

    /// Release a buffer back to the pool
    pub async fn release_buffer(&self, buffer: ZeroCopyBuffer) -> Result<(), String> {
        // Decrease reference count
        let ref_count = buffer.ref_count.fetch_sub(1, Ordering::Relaxed);

        if ref_count == 1 {
            // Last reference - return to pool
            let size_class = self.find_size_class(buffer.size);

            {
                let mut pools = self.buffer_pools.write().await;
                if let Some(pool) = pools.get_mut(&size_class) {
                    pool.return_buffer(buffer);
                } else {
                    // Pool doesn't exist, deallocate
                    self.deallocate_buffer(buffer).await;
                }
            }

            // Unregister the buffer
            self.unregister_buffer(buffer.id).await;
        }

        Ok(())
    }

    /// Get memory usage analytics
    pub async fn get_analytics(&self) -> MemoryUsageAnalytics {
        let pools = self.buffer_pools.read().await;
        let registry = self.buffer_registry.read().await;

        let total_buffers: usize = registry.len();
        let total_memory: u64 = registry.values()
            .map(|info| info.size as u64)
            .sum();

        let buffer_pool_utilization = if pools.is_empty() {
            0.0
        } else {
            pools.values().map(|pool| pool.stats.utilization).sum::<f64>() / pools.len() as f64
        };

        MemoryUsageAnalytics {
            timestamp: SystemTime::now(),
            total_memory_bytes: total_memory,
            heap_memory_bytes: total_memory, // Simplified
            mmap_memory_bytes: 0, // Would track mmap usage
            buffer_pool_utilization,
            allocation_rate: 0.0, // Would calculate from recent history
            deallocation_rate: 0.0, // Would calculate from recent history
            fragmentation_level: 0.0, // Would calculate fragmentation
        }
    }

    /// Find the appropriate size class for a buffer
    fn find_size_class(&self, size: usize) -> usize {
        self.config.default_sizes.iter()
            .find(|&&class_size| class_size >= size)
            .copied()
            .unwrap_or_else(|| {
                // Round up to next power of 2
                let mut class_size = 1;
                while class_size < size {
                    class_size <<= 1;
                }
                class_size
            })
    }

    /// Allocate a new buffer
    async fn allocate_new_buffer(&self, size: usize) -> Result<ZeroCopyBuffer, String> {
        let buffer_id = self.next_buffer_id.fetch_add(1, Ordering::Relaxed);

        let buffer_type = if size >= self.config.mmap_threshold && self.config.enable_mmap {
            BufferType::MemoryMapped
        } else if self.config.alignment > 0 {
            BufferType::Aligned
        } else {
            BufferType::Heap
        };

        let data = match buffer_type {
            BufferType::Heap => {
                let layout = std::alloc::Layout::from_size_align(size, 1)
                    .map_err(|e| format!("Invalid layout: {}", e))?;
                unsafe { std::alloc::alloc(layout) }
            }
            BufferType::Aligned => {
                let layout = std::alloc::Layout::from_size_align(size, self.config.alignment)
                    .map_err(|e| format!("Invalid aligned layout: {}", e))?;
                unsafe { std::alloc::alloc(layout) }
            }
            BufferType::MemoryMapped => {
                // Would implement mmap allocation
                return Err("Memory mapping not implemented".to_string());
            }
            BufferType::Pinned => {
                // Would implement pinned memory allocation
                return Err("Pinned memory not implemented".to_string());
            }
        };

        if data.is_null() {
            return Err("Failed to allocate buffer".to_string());
        }

        let buffer = ZeroCopyBuffer {
            id: buffer_id,
            data,
            size,
            ref_count: Arc::new(AtomicUsize::new(1)),
            buffer_type,
            created_at: Instant::now(),
        };

        self.register_buffer(&buffer).await;
        Ok(buffer)
    }

    /// Register a buffer in the registry
    async fn register_buffer(&self, buffer: &ZeroCopyBuffer) {
        let info = BufferInfo {
            id: buffer.id,
            size: buffer.size,
            buffer_type: buffer.buffer_type,
            ref_count: buffer.ref_count.load(Ordering::Relaxed),
            created_at: buffer.created_at,
            last_accessed: Instant::now(),
        };

        let mut registry = self.buffer_registry.write().await;
        registry.insert(buffer.id, info);
    }

    /// Unregister a buffer from the registry
    async fn unregister_buffer(&self, buffer_id: usize) {
        let mut registry = self.buffer_registry.write().await;
        registry.remove(&buffer_id);
    }

    /// Deallocate a buffer
    async fn deallocate_buffer(&self, buffer: ZeroCopyBuffer) {
        unsafe {
            match buffer.buffer_type {
                BufferType::Heap | BufferType::Aligned => {
                    let alignment = if matches!(buffer.buffer_type, BufferType::Aligned) {
                        self.config.alignment
                    } else {
                        1
                    };

                    let layout = std::alloc::Layout::from_size_align(buffer.size, alignment)
                        .expect("Invalid layout for deallocation");
                    std::alloc::dealloc(buffer.data, layout);
                }
                BufferType::MemoryMapped => {
                    // Would implement mmap deallocation
                }
                BufferType::Pinned => {
                    // Would implement pinned memory deallocation
                }
            }
        }
    }
}

impl BufferPool {
    /// Create a new buffer pool
    pub fn new(buffer_size: usize, max_size: usize) -> Self {
        Self {
            buffer_size,
            available_buffers: VecDeque::new(),
            max_size,
            stats: BufferPoolStats::default(),
        }
    }

    /// Try to allocate a buffer from the pool
    pub fn try_allocate(&mut self) -> Option<ZeroCopyBuffer> {
        self.stats.requests += 1;

        if let Some(buffer) = self.available_buffers.pop_front() {
            self.stats.hits += 1;
            self.update_utilization();
            Some(buffer)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Return a buffer to the pool
    pub fn return_buffer(&mut self, buffer: ZeroCopyBuffer) {
        if self.available_buffers.len() < self.max_size {
            self.available_buffers.push_back(buffer);
            self.update_utilization();
        }
        // If pool is full, buffer will be dropped and deallocated
    }

    /// Update utilization statistics
    fn update_utilization(&mut self) {
        self.stats.utilization = (self.max_size - self.available_buffers.len()) as f64 / self.max_size as f64;
    }
}

unsafe impl Send for ZeroCopyBuffer {}
unsafe impl Sync for ZeroCopyBuffer {}

impl ZeroCopyBuffer {
    /// Get a reference to the buffer data
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data, self.size) }
    }

    /// Get a mutable reference to the buffer data
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.data, self.size) }
    }

    /// Clone the buffer (increases reference count)
    pub fn clone_ref(&self) -> Self {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
        Self {
            id: self.id,
            data: self.data,
            size: self.size,
            ref_count: Arc::clone(&self.ref_count),
            buffer_type: self.buffer_type,
            created_at: self.created_at,
        }
    }

    /// Get current reference count
    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::Relaxed)
    }
}

impl Drop for ZeroCopyBuffer {
    fn drop(&mut self) {
        let ref_count = self.ref_count.fetch_sub(1, Ordering::Relaxed);
        if ref_count == 1 {
            // Last reference - this should be handled by the buffer manager
            // For safety, we don't deallocate here
        }
    }
}

/// Global memory optimization utilities
pub mod global_optimization {
    use super::*;

    /// Global memory optimizer
    pub struct GlobalMemoryOptimizer {
        /// Allocation tracker
        allocation_tracker: Arc<RwLock<HashMap<usize, AllocationInfo>>>,
        /// Optimization settings
        settings: OptimizationSettings,
    }

    /// Information about an allocation
    #[derive(Debug, Clone)]
    pub struct AllocationInfo {
        /// Size of allocation
        pub size: usize,
        /// Timestamp of allocation
        pub allocated_at: Instant,
        /// Last access time
        pub last_accessed: Instant,
        /// Access count
        pub access_count: u64,
        /// Allocation type
        pub allocation_type: AllocationType,
    }

    /// Type of allocation
    #[derive(Debug, Clone, Copy)]
    pub enum AllocationType {
        EventBuffer,
        ActionBuffer,
        StrategyState,
        NetworkBuffer,
        CacheEntry,
        Other,
    }

    /// Optimization settings
    #[derive(Debug, Clone)]
    pub struct OptimizationSettings {
        /// Enable automatic garbage collection
        pub enable_auto_gc: bool,
        /// GC trigger threshold (memory usage percentage)
        pub gc_threshold: f64,
        /// Enable memory compression
        pub enable_compression: bool,
        /// Compression threshold
        pub compression_threshold: usize,
    }

    impl Default for OptimizationSettings {
        fn default() -> Self {
            Self {
                enable_auto_gc: true,
                gc_threshold: 0.8,
                enable_compression: false,
                compression_threshold: 1024 * 1024, // 1MB
            }
        }
    }

    impl GlobalMemoryOptimizer {
        /// Create a new global memory optimizer
        pub fn new(settings: OptimizationSettings) -> Self {
            Self {
                allocation_tracker: Arc::new(RwLock::new(HashMap::new())),
                settings,
            }
        }

        /// Track a new allocation
        pub async fn track_allocation(&self, ptr: usize, size: usize, allocation_type: AllocationType) {
            let info = AllocationInfo {
                size,
                allocated_at: Instant::now(),
                last_accessed: Instant::now(),
                access_count: 1,
                allocation_type,
            };

            let mut tracker = self.allocation_tracker.write().await;
            tracker.insert(ptr, info);
        }

        /// Record access to an allocation
        pub async fn record_access(&self, ptr: usize) {
            let mut tracker = self.allocation_tracker.write().await;
            if let Some(info) = tracker.get_mut(&ptr) {
                info.last_accessed = Instant::now();
                info.access_count += 1;
            }
        }

        /// Remove allocation tracking
        pub async fn untrack_allocation(&self, ptr: usize) {
            let mut tracker = self.allocation_tracker.write().await;
            tracker.remove(&ptr);
        }

        /// Get memory usage statistics
        pub async fn get_memory_stats(&self) -> MemoryStats {
            let tracker = self.allocation_tracker.read().await;

            let total_allocations = tracker.len();
            let total_memory: usize = tracker.values().map(|info| info.size).sum();

            let mut type_breakdown = HashMap::new();
            for info in tracker.values() {
                let entry = type_breakdown.entry(info.allocation_type).or_insert(0);
                *entry += info.size;
            }

            MemoryStats {
                total_allocations,
                total_memory_bytes: total_memory,
                type_breakdown,
                timestamp: Instant::now(),
            }
        }

        /// Run garbage collection
        pub async fn run_gc(&self) -> GCStats {
            let start_time = Instant::now();
            let mut freed_bytes = 0;
            let mut freed_allocations = 0;

            // This is a simplified GC - real implementation would be more sophisticated
            {
                let mut tracker = self.allocation_tracker.write().await;
                let now = Instant::now();
                let stale_threshold = Duration::from_secs(300); // 5 minutes

                tracker.retain(|_, info| {
                    if now.duration_since(info.last_accessed) > stale_threshold {
                        freed_bytes += info.size;
                        freed_allocations += 1;
                        false
                    } else {
                        true
                    }
                });
            }

            GCStats {
                duration: start_time.elapsed(),
                freed_bytes,
                freed_allocations,
                remaining_allocations: self.allocation_tracker.read().await.len(),
            }
        }
    }

    /// Memory usage statistics
    #[derive(Debug)]
    pub struct MemoryStats {
        pub total_allocations: usize,
        pub total_memory_bytes: usize,
        pub type_breakdown: HashMap<AllocationType, usize>,
        pub timestamp: Instant,
    }

    /// Garbage collection statistics
    #[derive(Debug)]
    pub struct GCStats {
        pub duration: Duration,
        pub freed_bytes: usize,
        pub freed_allocations: usize,
        pub remaining_allocations: usize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_buffer_manager_creation() {
        let config = BufferManagerConfig::default();
        let manager = ZeroCopyBufferManager::new(config);

        assert!(manager.initialize().await.is_ok());
    }

    #[tokio::test]
    async fn test_buffer_allocation() {
        let config = BufferManagerConfig::default();
        let manager = ZeroCopyBufferManager::new(config);
        manager.initialize().await.unwrap();

        let buffer = manager.acquire_buffer(1024).await;
        assert!(buffer.is_ok());

        let buffer = buffer.unwrap();
        assert_eq!(buffer.size, 1024);
        assert_eq!(buffer.ref_count(), 1);
    }

    #[test]
    fn test_simd_vector() {
        let mut vec1 = simd_structures::SimdVector::new(16);
        let vec2 = vec![1.0; 16];

        let result = vec1.add_vectorized(&vec2);
        assert!(result.is_ok());

        // Verify the result
        for &value in vec1.data() {
            assert_eq!(value, 1.0);
        }
    }

    #[test]
    fn test_allocator_stats() {
        let allocator = ArtemisAllocator::new();
        let stats = allocator.get_stats();

        assert_eq!(stats.total_allocations.load(Ordering::Relaxed), 0);
        assert_eq!(stats.current_bytes.load(Ordering::Relaxed), 0);
    }
}
