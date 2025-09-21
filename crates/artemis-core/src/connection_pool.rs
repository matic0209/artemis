use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use anyhow::{anyhow, Result};
use dashmap::DashMap;

use crate::eth::Provider;

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections per endpoint
    pub max_connections: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Maximum connection age before refresh
    pub max_connection_age: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            connect_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
            max_connection_age: Duration::from_secs(300),
        }
    }
}

/// Connection metadata
#[derive(Debug)]
struct ConnectionInfo {
    provider: Arc<Provider>,
    created_at: Instant,
    last_used: Instant,
    request_count: u64,
    error_count: u64,
}

/// High-performance connection pool for Ethereum providers
pub struct ConnectionPool {
    /// Active connections by endpoint
    connections: DashMap<String, Vec<Arc<Mutex<ConnectionInfo>>>>,
    /// Pool configuration
    config: PoolConfig,
    /// Health checker
    health_checker: Arc<RwLock<HealthChecker>>,
}

struct HealthChecker {
    unhealthy_endpoints: DashMap<String, Instant>,
    check_interval: Duration,
}

impl ConnectionPool {
    pub fn new(config: PoolConfig) -> Self {
        Self {
            connections: DashMap::new(),
            config,
            health_checker: Arc::new(RwLock::new(HealthChecker {
                unhealthy_endpoints: DashMap::new(),
                check_interval: config.health_check_interval,
            })),
        }
    }

    /// Get an optimized provider connection
    pub async fn get_provider(&self, endpoint: &str) -> Result<Arc<Provider>> {
        // Check if endpoint is healthy
        if self.is_endpoint_unhealthy(endpoint).await {
            return Err(anyhow!("Endpoint {} is marked unhealthy", endpoint));
        }

        // Try to get existing connection
        if let Some(connection) = self.get_existing_connection(endpoint).await {
            return Ok(connection);
        }

        // Create new connection
        self.create_new_connection(endpoint).await
    }

    async fn get_existing_connection(&self, endpoint: &str) -> Option<Arc<Provider>> {
        let connections = self.connections.get(endpoint)?;
        
        for conn_mutex in connections.value() {
            let mut conn = conn_mutex.lock().await;
            
            // Check if connection is still valid
            if conn.created_at.elapsed() < self.config.max_connection_age 
                && conn.error_count < 5 {
                conn.last_used = Instant::now();
                conn.request_count += 1;
                
                metrics::counter!("artemis.connection_pool.reused").increment(1);
                return Some(Arc::clone(&conn.provider));
            }
        }
        
        None
    }

    async fn create_new_connection(&self, endpoint: &str) -> Result<Arc<Provider>> {
        use crate::eth::alloy_support::helpers;
        
        let provider = if endpoint.starts_with("ws") {
            helpers::create_ws_provider(endpoint).await?
        } else {
            helpers::create_http_provider(endpoint).await?
        };

        let provider = Arc::new(provider);
        let conn_info = ConnectionInfo {
            provider: Arc::clone(&provider),
            created_at: Instant::now(),
            last_used: Instant::now(),
            request_count: 1,
            error_count: 0,
        };

        // Add to pool
        self.connections
            .entry(endpoint.to_string())
            .or_insert_with(Vec::new)
            .push(Arc::new(Mutex::new(conn_info)));

        metrics::counter!("artemis.connection_pool.created").increment(1);
        Ok(provider)
    }

    async fn is_endpoint_unhealthy(&self, endpoint: &str) -> bool {
        let checker = self.health_checker.read().await;
        if let Some(marked_time) = checker.unhealthy_endpoints.get(endpoint) {
            // Re-enable after 60 seconds
            marked_time.elapsed() < Duration::from_secs(60)
        } else {
            false
        }
    }

    /// Mark endpoint as unhealthy
    pub async fn mark_unhealthy(&self, endpoint: &str) {
        let checker = self.health_checker.write().await;
        checker.unhealthy_endpoints.insert(endpoint.to_string(), Instant::now());
        
        metrics::counter!("artemis.connection_pool.marked_unhealthy").increment(1);
    }

    /// Start background health checker
    pub async fn start_health_checker(&self) {
        let pool = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(pool.config.health_check_interval);
            
            loop {
                interval.tick().await;
                pool.perform_health_check().await;
            }
        });
    }

    async fn perform_health_check(&self) {
        for entry in self.connections.iter() {
            let endpoint = entry.key();
            let connections = entry.value();
            
            // Remove old/unhealthy connections
            connections.retain(|conn_mutex| {
                let conn = conn_mutex.try_lock();
                match conn {
                    Ok(conn) => {
                        let is_healthy = conn.created_at.elapsed() < self.config.max_connection_age
                            && conn.error_count < 10;
                        
                        if !is_healthy {
                            metrics::counter!("artemis.connection_pool.removed_unhealthy").increment(1);
                        }
                        
                        is_healthy
                    }
                    Err(_) => true, // Keep if locked (in use)
                }
            });
        }
        
        metrics::gauge!("artemis.connection_pool.total_connections")
            .set(self.total_connections() as f64);
    }

    fn total_connections(&self) -> usize {
        self.connections.iter().map(|entry| entry.value().len()).sum()
    }

    /// Get connection pool statistics
    pub async fn get_pool_stats(&self) -> PoolStats {
        let stats = self.stats.read().await;
        PoolStats {
            total_connections: self.total_connections(),
            cache_hit_rate: if stats.hits + stats.misses > 0 {
                stats.hits as f64 / (stats.hits + stats.misses) as f64
            } else {
                0.0
            },
            prefetch_hit_rate: if stats.prefetch_hits > 0 {
                stats.prefetch_hits as f64 / stats.hits as f64
            } else {
                0.0
            },
        }
    }
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            config: self.config.clone(),
            health_checker: Arc::clone(&self.health_checker),
        }
    }
}

#[derive(Debug)]
pub struct PoolStats {
    pub total_connections: usize,
    pub cache_hit_rate: f64,
    pub prefetch_hit_rate: f64,
}
