use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::Mutex;

/// Custom memory allocator with tracking and optimization
pub struct OptimizedAllocator {
    /// System allocator
    inner: System,
    /// Allocation statistics
    stats: AllocationStats,
}

#[derive(Debug)]
struct AllocationStats {
    total_allocated: AtomicUsize,
    total_deallocated: AtomicUsize,
    peak_usage: AtomicUsize,
    allocation_count: AtomicUsize,
}

impl AllocationStats {
    fn new() -> Self {
        Self {
            total_allocated: AtomicUsize::new(0),
            total_deallocated: AtomicUsize::new(0),
            peak_usage: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
        }
    }

    fn record_allocation(&self, size: usize) {
        self.total_allocated.fetch_add(size, Ordering::Relaxed);
        self.allocation_count.fetch_add(1, Ordering::Relaxed);
        
        let current_usage = self.current_usage();
        let mut peak = self.peak_usage.load(Ordering::Relaxed);
        while current_usage > peak {
            match self.peak_usage.compare_exchange_weak(
                peak,
                current_usage,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    fn record_deallocation(&self, size: usize) {
        self.total_deallocated.fetch_add(size, Ordering::Relaxed);
    }

    fn current_usage(&self) -> usize {
        self.total_allocated.load(Ordering::Relaxed)
            .saturating_sub(self.total_deallocated.load(Ordering::Relaxed))
    }
}

unsafe impl GlobalAlloc for OptimizedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = self.inner.alloc(layout);
        if !ptr.is_null() {
            self.stats.record_allocation(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.stats.record_deallocation(layout.size());
        self.inner.dealloc(ptr, layout);
    }
}

/// Object pool for high-frequency allocations
pub struct ObjectPool<T> {
    pool: Arc<Mutex<VecDeque<T>>>,
    factory: Box<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
    stats: PoolStatistics,
}

#[derive(Debug, Default)]
struct PoolStatistics {
    gets: AtomicUsize,
    puts: AtomicUsize,
    creates: AtomicUsize,
    discards: AtomicUsize,
}

impl<T> ObjectPool<T>
where
    T: Send + 'static,
{
    pub fn new<F>(factory: F, max_size: usize) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            factory: Box::new(factory),
            max_size,
            stats: PoolStatistics::default(),
        }
    }

    /// Get object from pool or create new one
    pub async fn get(&self) -> PooledObject<T> {
        let mut pool = self.pool.lock().await;
        
        let object = if let Some(obj) = pool.pop_front() {
            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            obj
        } else {
            self.stats.creates.fetch_add(1, Ordering::Relaxed);
            (self.factory)()
        };
        
        drop(pool); // Release lock early
        
        PooledObject {
            object: Some(object),
            pool: Arc::clone(&self.pool),
            stats: &self.stats,
            max_size: self.max_size,
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> (usize, usize, usize, usize) {
        (
            self.stats.gets.load(Ordering::Relaxed),
            self.stats.puts.load(Ordering::Relaxed),
            self.stats.creates.load(Ordering::Relaxed),
            self.stats.discards.load(Ordering::Relaxed),
        )
    }
}

/// RAII wrapper for pooled objects
pub struct PooledObject<T> {
    object: Option<T>,
    pool: Arc<Mutex<VecDeque<T>>>,
    stats: *const PoolStatistics,
    max_size: usize,
}

impl<T> PooledObject<T> {
    /// Get reference to the pooled object
    pub fn get(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    /// Get mutable reference to the pooled object
    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            let stats = unsafe { &*self.stats };
            
            // Try to return to pool
            if let Ok(mut pool) = self.pool.try_lock() {
                if pool.len() < self.max_size {
                    pool.push_back(object);
                    stats.puts.fetch_add(1, Ordering::Relaxed);
                } else {
                    stats.discards.fetch_add(1, Ordering::Relaxed);
                }
            } else {
                stats.discards.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Zero-allocation string operations
pub struct ZeroAllocStringOps;

impl ZeroAllocStringOps {
    /// Parse hex without intermediate string allocation
    pub fn parse_hex_to_bytes(hex: &str) -> Result<Vec<u8>, hex::FromHexError> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        hex::decode(hex)
    }

    /// Format bytes to hex using a reusable buffer
    pub fn format_bytes_to_hex(bytes: &[u8], buffer: &mut String) {
        buffer.clear();
        buffer.push_str("0x");
        for byte in bytes {
            use std::fmt::Write;
            write!(buffer, "{:02x}", byte).unwrap();
        }
    }

    /// Efficient string interning for repeated values
    pub fn intern_string(s: &str, interner: &mut StringInterner) -> InternedString {
        interner.intern(s)
    }
}

/// String interner to reduce memory usage for repeated strings
pub struct StringInterner {
    strings: HashMap<String, InternedString>,
    next_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternedString(u32);

impl StringInterner {
    pub fn new() -> Self {
        Self {
            strings: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn intern(&mut self, s: &str) -> InternedString {
        if let Some(&interned) = self.strings.get(s) {
            interned
        } else {
            let interned = InternedString(self.next_id);
            self.next_id += 1;
            self.strings.insert(s.to_string(), interned);
            interned
        }
    }
}

/// CPU cache-friendly data structures
pub struct CacheFriendlyData {
    /// Structure of Arrays instead of Array of Structures
    addresses: Vec<Address>,
    values: Vec<U256>,
    timestamps: Vec<u64>,
    /// Indices for fast lookup
    address_to_index: HashMap<Address, usize>,
}

impl CacheFriendlyData {
    pub fn new() -> Self {
        Self {
            addresses: Vec::new(),
            values: Vec::new(),
            timestamps: Vec::new(),
            address_to_index: HashMap::new(),
        }
    }

    /// Add data in cache-friendly manner
    pub fn insert(&mut self, address: Address, value: U256, timestamp: u64) {
        if let Some(&index) = self.address_to_index.get(&address) {
            // Update existing
            self.values[index] = value;
            self.timestamps[index] = timestamp;
        } else {
            // Add new
            let index = self.addresses.len();
            self.addresses.push(address);
            self.values.push(value);
            self.timestamps.push(timestamp);
            self.address_to_index.insert(address, index);
        }
    }

    /// Get value with cache-friendly access pattern
    pub fn get_value(&self, address: &Address) -> Option<U256> {
        self.address_to_index
            .get(address)
            .map(|&index| self.values[index])
    }

    /// Bulk operations on all data (vectorizable)
    pub fn apply_discount_all(&mut self, discount_percent: u8) {
        let multiplier = U256::from(100 - discount_percent);
        let divisor = U256::from(100);
        
        // Vectorized operation
        for value in &mut self.values {
            *value = (*value * multiplier) / divisor;
        }
    }

    /// Sort by value efficiently
    pub fn sort_by_value(&mut self) {
        // Create index array for indirect sorting
        let mut indices: Vec<usize> = (0..self.addresses.len()).collect();
        
        // Sort indices by values
        indices.sort_unstable_by(|&a, &b| self.values[b].cmp(&self.values[a]));
        
        // Reorder all arrays based on sorted indices
        let old_addresses = std::mem::take(&mut self.addresses);
        let old_values = std::mem::take(&mut self.values);
        let old_timestamps = std::mem::take(&mut self.timestamps);
        
        for (new_index, &old_index) in indices.iter().enumerate() {
            self.addresses.push(old_addresses[old_index]);
            self.values.push(old_values[old_index]);
            self.timestamps.push(old_timestamps[old_index]);
            
            // Update lookup map
            self.address_to_index.insert(self.addresses[new_index], new_index);
        }
    }
}

use std::collections::HashMap;

/// Lock-free concurrent data structures for high-performance access
pub mod lockfree {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::collections::HashMap;
    use crate::eth::{Address, U256};

    /// Lock-free counter for high-frequency updates
    pub struct AtomicCounter {
        value: AtomicU64,
    }

    impl AtomicCounter {
        pub fn new() -> Self {
            Self {
                value: AtomicU64::new(0),
            }
        }

        pub fn increment(&self) -> u64 {
            self.value.fetch_add(1, Ordering::Relaxed)
        }

        pub fn get(&self) -> u64 {
            self.value.load(Ordering::Relaxed)
        }

        pub fn reset(&self) -> u64 {
            self.value.swap(0, Ordering::Relaxed)
        }
    }

    /// Lock-free metrics collection
    pub struct LockFreeMetrics {
        counters: HashMap<String, AtomicCounter>,
        gauges: HashMap<String, AtomicU64>,
    }

    impl LockFreeMetrics {
        pub fn new() -> Self {
            Self {
                counters: HashMap::new(),
                gauges: HashMap::new(),
            }
        }

        pub fn increment_counter(&self, name: &str) {
            if let Some(counter) = self.counters.get(name) {
                counter.increment();
            }
        }

        pub fn set_gauge(&self, name: &str, value: u64) {
            if let Some(gauge) = self.gauges.get(name) {
                gauge.store(value, Ordering::Relaxed);
            }
        }
    }
}

/// Advanced memory layout optimizations
pub struct MemoryLayoutOptimizer;

impl MemoryLayoutOptimizer {
    /// Pack multiple small values into single cache line
    pub fn pack_address_value_pairs(pairs: &[(Address, U256)]) -> PackedAddressValues {
        PackedAddressValues::new(pairs)
    }

    /// Optimize struct layout for cache efficiency
    pub fn optimize_for_cache_lines<T>(data: Vec<T>) -> CacheOptimizedVec<T> {
        CacheOptimizedVec::new(data)
    }
}

/// Cache-line optimized storage for address-value pairs
#[repr(C, align(64))] // Align to cache line
pub struct PackedAddressValues {
    addresses: Vec<Address>,
    values: Vec<U256>,
    count: usize,
}

impl PackedAddressValues {
    fn new(pairs: &[(Address, U256)]) -> Self {
        let (addresses, values): (Vec<_>, Vec<_>) = pairs.iter().cloned().unzip();
        Self {
            count: addresses.len(),
            addresses,
            values,
        }
    }

    pub fn get(&self, index: usize) -> Option<(Address, U256)> {
        if index < self.count {
            Some((self.addresses[index], self.values[index]))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }
}

/// Cache-optimized vector with prefetching
pub struct CacheOptimizedVec<T> {
    data: Vec<T>,
    prefetch_distance: usize,
}

impl<T> CacheOptimizedVec<T> {
    fn new(data: Vec<T>) -> Self {
        Self {
            data,
            prefetch_distance: 8, // Prefetch 8 elements ahead
        }
    }

    /// Iterator with automatic prefetching
    pub fn iter_with_prefetch(&self) -> impl Iterator<Item = &T> {
        self.data.iter().enumerate().map(move |(i, item)| {
            // Prefetch future elements
            if i + self.prefetch_distance < self.data.len() {
                let future_ptr = &self.data[i + self.prefetch_distance] as *const T;
                unsafe {
                    std::intrinsics::prefetch_read_data(future_ptr as *const i8, 3);
                }
            }
            item
        })
    }
}

/// NUMA-aware memory allocation strategies
pub struct NumaOptimizer;

impl NumaOptimizer {
    /// Allocate memory on the same NUMA node as the current thread
    pub fn allocate_local<T>(size: usize) -> Vec<T> {
        // For now, just use regular allocation
        // In production, could use libnuma for NUMA-aware allocation
        Vec::with_capacity(size)
    }

    /// Pin thread to specific CPU core for consistent performance
    pub fn pin_thread_to_core(core_id: usize) -> Result<(), Box<dyn std::error::Error>> {
        // Platform-specific implementation would go here
        // For now, just return Ok
        Ok(())
    }
}

/// Compile-time optimizations and hints
pub mod compile_hints {
    /// Hint to compiler for branch prediction
    #[inline(always)]
    pub fn likely(condition: bool) -> bool {
        std::intrinsics::likely(condition)
    }

    #[inline(always)]
    pub fn unlikely(condition: bool) -> bool {
        std::intrinsics::unlikely(condition)
    }

    /// Force inlining for hot path functions
    #[inline(always)]
    pub fn hot_path_add(a: u64, b: u64) -> u64 {
        a.wrapping_add(b)
    }

    /// Prefetch memory for better cache performance
    #[inline(always)]
    pub fn prefetch_read<T>(ptr: *const T) {
        unsafe {
            std::intrinsics::prefetch_read_data(ptr as *const i8, 3);
        }
    }
}
