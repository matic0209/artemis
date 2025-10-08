//! Z3 Solver Result Caching
//!
//! Caches Z3 solver results to avoid redundant computations.
//! Uses LRU cache with configurable size.

use alloy_primitives::U256;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Cache key for Z3 solver results
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SolverCacheKey {
    /// Strategy type (e.g., "triangular", "flashloan", "sandwich")
    pub strategy_type: String,
    /// Pool/swap parameters (hashed)
    pub params_hash: u64,
    /// Constraints hash
    pub constraints_hash: u64,
}

impl SolverCacheKey {
    pub fn new(
        strategy_type: String,
        params: &[U256],
        constraints: &[U256],
    ) -> Self {
        Self {
            strategy_type,
            params_hash: hash_u256_slice(params),
            constraints_hash: hash_u256_slice(constraints),
        }
    }
}

/// Cache value with result and metadata
#[derive(Debug, Clone)]
pub struct SolverCacheValue {
    /// Whether solver found a solution
    pub is_sat: bool,
    /// Optimal values (if SAT)
    pub solution: Option<HashMap<String, U256>>,
    /// When this entry was cached
    pub cached_at: Instant,
    /// How long solving took
    pub solve_time: Duration,
}

/// Z3 result cache with LRU eviction
pub struct Z3Cache {
    cache: Arc<Mutex<lru::LruCache<SolverCacheKey, SolverCacheValue>>>,
    stats: Arc<Mutex<CacheStats>>,
    max_age: Duration,
}

#[derive(Debug, Default)]
struct CacheStats {
    hits: u64,
    misses: u64,
    evictions: u64,
    total_solve_time_saved_ms: u64,
}

impl Z3Cache {
    /// Create new cache with specified capacity
    pub fn new(capacity: usize, max_age: Duration) -> Self {
        Self {
            cache: Arc::new(Mutex::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(capacity).unwrap()
            ))),
            stats: Arc::new(Mutex::new(CacheStats::default())),
            max_age,
        }
    }

    /// Try to get cached result
    pub fn get(&self, key: &SolverCacheKey) -> Option<SolverCacheValue> {
        let mut cache = self.cache.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();

        if let Some(value) = cache.get(key) {
            // Check if entry is still fresh
            if value.cached_at.elapsed() < self.max_age {
                stats.hits += 1;
                stats.total_solve_time_saved_ms += value.solve_time.as_millis() as u64;
                return Some(value.clone());
            } else {
                // Entry expired, remove it
                cache.pop(key);
            }
        }

        stats.misses += 1;
        None
    }

    /// Store result in cache
    pub fn put(&self, key: SolverCacheKey, value: SolverCacheValue) {
        let mut cache = self.cache.lock().unwrap();

        // Check if we're about to evict
        if cache.len() >= cache.cap().get() {
            let mut stats = self.stats.lock().unwrap();
            stats.evictions += 1;
        }

        cache.put(key, value);
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.lock().unwrap().clone()
    }

    /// Get hit rate
    pub fn hit_rate(&self) -> f64 {
        let stats = self.stats.lock().unwrap();
        let total = stats.hits + stats.misses;
        if total == 0 {
            0.0
        } else {
            stats.hits as f64 / total as f64
        }
    }

    /// Clear cache
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }
}

impl Clone for CacheStats {
    fn clone(&self) -> Self {
        Self {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            total_solve_time_saved_ms: self.total_solve_time_saved_ms,
        }
    }
}

/// Hash a slice of U256 values
fn hash_u256_slice(values: &[U256]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    for value in values {
        value.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = Z3Cache::new(10, Duration::from_secs(60));

        let key = SolverCacheKey {
            strategy_type: "test".to_string(),
            params_hash: 12345,
            constraints_hash: 67890,
        };

        let value = SolverCacheValue {
            is_sat: true,
            solution: Some(HashMap::new()),
            cached_at: Instant::now(),
            solve_time: Duration::from_millis(100),
        };

        // First get should miss
        assert!(cache.get(&key).is_none());

        // Put value
        cache.put(key.clone(), value.clone());

        // Second get should hit
        assert!(cache.get(&key).is_some());

        assert_eq!(cache.hit_rate(), 0.5); // 1 hit, 1 miss
    }

    #[test]
    fn test_cache_expiration() {
        let cache = Z3Cache::new(10, Duration::from_millis(100));

        let key = SolverCacheKey {
            strategy_type: "test".to_string(),
            params_hash: 1,
            constraints_hash: 2,
        };

        let value = SolverCacheValue {
            is_sat: true,
            solution: None,
            cached_at: Instant::now(),
            solve_time: Duration::from_millis(50),
        };

        cache.put(key.clone(), value);

        // Should be in cache
        assert!(cache.get(&key).is_some());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(150));

        // Should be expired
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cache_key_creation() {
        let params = vec![U256::from(100), U256::from(200)];
        let constraints = vec![U256::from(1000)];

        let key1 = SolverCacheKey::new("test".to_string(), &params, &constraints);
        let key2 = SolverCacheKey::new("test".to_string(), &params, &constraints);

        assert_eq!(key1, key2);
    }
}
