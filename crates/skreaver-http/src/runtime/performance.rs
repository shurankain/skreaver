//! Performance optimizations for HTTP runtime

use std::sync::Arc;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use tokio::sync::RwLock;
use dashmap::DashMap;
use moka::future::Cache;
use std::time::{Duration, Instant};

/// High-performance agent registry with caching and optimization
pub struct OptimizedAgentRegistry {
    /// Primary agent storage with concurrent access
    agents: DashMap<CachedAgentId, Arc<crate::runtime::agent_instance::AgentInstance>>,
    /// LRU cache for frequently accessed agents
    agent_cache: Cache<CachedAgentId, Arc<crate::runtime::agent_instance::AgentInstance>>,
    /// Performance metrics
    metrics: Arc<RegistryMetrics>,
}

/// Fast agent ID type with optimized hashing (internal optimization)
///
/// This type wraps the unified `skreaver_core::AgentId` with a pre-computed hash
/// for faster lookups in the agent registry. The hash is computed once during
/// creation and reused for all subsequent hash operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedAgentId {
    id: skreaver_core::AgentId,
    hash: u64,
}

impl CachedAgentId {
    /// Create a new cached agent ID with pre-computed hash
    pub fn new(id: skreaver_core::AgentId) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.as_str().hash(&mut hasher);
        let hash = hasher.finish();

        Self { id, hash }
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }

    /// Get the underlying AgentId
    pub fn inner(&self) -> &skreaver_core::AgentId {
        &self.id
    }
}

impl Hash for CachedAgentId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl From<skreaver_core::AgentId> for CachedAgentId {
    fn from(id: skreaver_core::AgentId) -> Self {
        Self::new(id)
    }
}

/// Registry performance metrics
#[derive(Debug, Default)]
pub struct RegistryMetrics {
    /// Total agent lookups
    pub lookups: std::sync::atomic::AtomicU64,
    /// Cache hits
    pub cache_hits: std::sync::atomic::AtomicU64,
    /// Cache misses
    pub cache_misses: std::sync::atomic::AtomicU64,
    /// Average lookup time in nanoseconds
    pub avg_lookup_time_ns: std::sync::atomic::AtomicU64,
}

impl OptimizedAgentRegistry {
    /// Create a new optimized agent registry
    pub fn new() -> Self {
        Self {
            agents: DashMap::new(),
            agent_cache: Cache::builder()
                .max_capacity(1000) // Cache up to 1000 agents
                .time_to_live(Duration::from_secs(300)) // 5 minute TTL
                .build(),
            metrics: Arc::new(RegistryMetrics::default()),
        }
    }
    
    /// Get an agent with optimized lookup
    pub async fn get_agent(&self, id: &CachedAgentId) -> Option<Arc<crate::runtime::agent_instance::AgentInstance>> {
        let start = Instant::now();
        self.metrics.lookups.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        // Try cache first
        if let Some(agent) = self.agent_cache.get(id).await {
            self.metrics.cache_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let lookup_time = start.elapsed().as_nanos() as u64;
            self.update_avg_lookup_time(lookup_time);
            return Some(agent);
        }
        
        // Cache miss, try primary storage
        if let Some(agent) = self.agents.get(id).map(|entry| entry.value().clone()) {
            // Add to cache for future lookups
            self.agent_cache.insert(id.clone(), agent.clone()).await;
            self.metrics.cache_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let lookup_time = start.elapsed().as_nanos() as u64;
            self.update_avg_lookup_time(lookup_time);
            Some(agent)
        } else {
            None
        }
    }
    
    /// Insert an agent with cache invalidation
    pub async fn insert_agent(&self, id: CachedAgentId, agent: Arc<crate::runtime::agent_instance::AgentInstance>) {
        self.agents.insert(id.clone(), agent.clone());
        self.agent_cache.insert(id, agent).await;
    }

    /// Remove an agent with cache invalidation
    pub async fn remove_agent(&self, id: &CachedAgentId) -> Option<Arc<crate::runtime::agent_instance::AgentInstance>> {
        self.agent_cache.invalidate(id).await;
        self.agents.remove(id).map(|(_, agent)| agent)
    }

    /// Get all agent IDs efficiently
    pub fn get_all_ids(&self) -> Vec<CachedAgentId> {
        self.agents.iter().map(|entry| entry.key().clone()).collect()
    }
    
    /// Get registry statistics
    pub fn get_metrics(&self) -> RegistryMetrics {
        RegistryMetrics {
            lookups: std::sync::atomic::AtomicU64::new(
                self.metrics.lookups.load(std::sync::atomic::Ordering::Relaxed)
            ),
            cache_hits: std::sync::atomic::AtomicU64::new(
                self.metrics.cache_hits.load(std::sync::atomic::Ordering::Relaxed)
            ),
            cache_misses: std::sync::atomic::AtomicU64::new(
                self.metrics.cache_misses.load(std::sync::atomic::Ordering::Relaxed)
            ),
            avg_lookup_time_ns: std::sync::atomic::AtomicU64::new(
                self.metrics.avg_lookup_time_ns.load(std::sync::atomic::Ordering::Relaxed)
            ),
        }
    }
    
    fn update_avg_lookup_time(&self, new_time_ns: u64) {
        // Simple moving average update (could be improved with proper EMA)
        let current_avg = self.metrics.avg_lookup_time_ns.load(std::sync::atomic::Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            new_time_ns
        } else {
            (current_avg + new_time_ns) / 2
        };
        self.metrics.avg_lookup_time_ns.store(new_avg, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Optimized request processing with batching
pub struct RequestBatcher {
    /// Pending requests grouped by agent
    pending_requests: Arc<RwLock<HashMap<CachedAgentId, Vec<BatchedRequest>>>>,
    /// Batch processing configuration
    config: BatchConfig,
}

/// Configuration for request batching
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Maximum wait time before processing partial batch
    pub max_wait_time: Duration,
    /// Enable batching (can be disabled for debugging)
    pub enabled: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 10,
            max_wait_time: Duration::from_millis(50),
            enabled: true,
        }
    }
}

/// A request that can be batched with others
#[derive(Debug)]
pub struct BatchedRequest {
    /// Request content
    pub content: String,
    /// Response sender
    pub response_sender: tokio::sync::oneshot::Sender<Result<String, String>>,
    /// Request timestamp
    pub timestamp: Instant,
}

impl RequestBatcher {
    /// Create a new request batcher
    pub fn new(config: BatchConfig) -> Self {
        let batcher = Self {
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            config,
        };
        
        // Start background batch processor
        if config.enabled {
            batcher.start_batch_processor();
        }
        
        batcher
    }
    
    /// Submit a request for batching
    pub async fn submit_request(
        &self,
        agent_id: CachedAgentId,
        content: String,
    ) -> Result<String, String> {
        if !self.config.enabled {
            // If batching is disabled, process immediately
            // This would need to be implemented with direct agent call
            return Err("Batching disabled - direct processing not implemented".to_string());
        }
        
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = BatchedRequest {
            content,
            response_sender: tx,
            timestamp: Instant::now(),
        };
        
        // Add to pending requests
        {
            let mut pending = self.pending_requests.write().await;
            pending.entry(agent_id).or_insert_with(Vec::new).push(request);
        }
        
        // Wait for response
        rx.await.map_err(|e| format!("Request cancelled or channel closed: {}", e))?
    }
    
    fn start_batch_processor(&self) {
        let pending_requests = Arc::clone(&self.pending_requests);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.max_wait_time);
            
            loop {
                interval.tick().await;
                
                // Process batches for each agent
                let mut pending = pending_requests.write().await;
                let mut agents_to_process = Vec::new();
                
                for (agent_id, requests) in pending.iter_mut() {
                    if !requests.is_empty() {
                        // Check if we should process this batch
                        let should_process = requests.len() >= config.max_batch_size ||
                            requests.iter().any(|req| req.timestamp.elapsed() >= config.max_wait_time);
                        
                        if should_process {
                            let batch = std::mem::take(requests);
                            agents_to_process.push((agent_id.clone(), batch));
                        }
                    }
                }
                
                // Remove empty entries
                pending.retain(|_, requests| !requests.is_empty());
                drop(pending); // Release the lock early
                
                // Process batches outside the lock
                for (agent_id, batch) in agents_to_process {
                    tokio::spawn(Self::process_batch(agent_id, batch));
                }
            }
        });
    }
    
    async fn process_batch(agent_id: CachedAgentId, batch: Vec<BatchedRequest>) {
        // TODO: Implement actual batch processing with agent
        // For now, simulate processing each request individually
        
        for request in batch {
            let response = format!("Processed: {}", request.content);
            let _ = request.response_sender.send(Ok(response));
        }
    }
}

/// Connection pooling for external resources
pub struct ConnectionPool<T> {
    /// Available connections
    pool: Arc<tokio::sync::Mutex<Vec<T>>>,
    /// Pool configuration
    config: PoolConfig,
    /// Pool metrics
    metrics: Arc<PoolMetrics>,
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections
    pub max_connections: usize,
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Maximum idle time before closing connection
    pub max_idle_time: Duration,
}

/// Pool performance metrics
#[derive(Debug, Default)]
pub struct PoolMetrics {
    /// Total connections created
    pub connections_created: std::sync::atomic::AtomicU64,
    /// Total connections destroyed
    pub connections_destroyed: std::sync::atomic::AtomicU64,
    /// Current active connections
    pub active_connections: std::sync::atomic::AtomicU64,
    /// Pool hits (successful borrows)
    pub pool_hits: std::sync::atomic::AtomicU64,
    /// Pool misses (had to create new connection)
    pub pool_misses: std::sync::atomic::AtomicU64,
}

impl<T> ConnectionPool<T> {
    /// Create a new connection pool
    pub fn new(config: PoolConfig) -> Self {
        Self {
            pool: Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(config.max_connections))),
            config,
            metrics: Arc::new(PoolMetrics::default()),
        }
    }
    
    /// Get a connection from the pool
    pub async fn get_connection<F>(&self, factory: F) -> Result<PooledConnection<T>, PoolError>
    where
        F: FnOnce() -> Result<T, PoolError>,
    {
        let mut pool = self.pool.lock().await;
        
        if let Some(connection) = pool.pop() {
            self.metrics.pool_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(PooledConnection::new(connection, Arc::clone(&self.pool)))
        } else {
            // No available connections, create new one
            drop(pool); // Release lock before potentially slow factory call
            
            let connection = factory()?;
            self.metrics.pool_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.metrics.connections_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.metrics.active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            
            Ok(PooledConnection::new(connection, Arc::clone(&self.pool)))
        }
    }
    
    /// Get current pool statistics
    pub fn get_metrics(&self) -> PoolMetrics {
        PoolMetrics {
            connections_created: std::sync::atomic::AtomicU64::new(
                self.metrics.connections_created.load(std::sync::atomic::Ordering::Relaxed)
            ),
            connections_destroyed: std::sync::atomic::AtomicU64::new(
                self.metrics.connections_destroyed.load(std::sync::atomic::Ordering::Relaxed)
            ),
            active_connections: std::sync::atomic::AtomicU64::new(
                self.metrics.active_connections.load(std::sync::atomic::Ordering::Relaxed)
            ),
            pool_hits: std::sync::atomic::AtomicU64::new(
                self.metrics.pool_hits.load(std::sync::atomic::Ordering::Relaxed)
            ),
            pool_misses: std::sync::atomic::AtomicU64::new(
                self.metrics.pool_misses.load(std::sync::atomic::Ordering::Relaxed)
            ),
        }
    }
}

/// A connection borrowed from the pool
pub struct PooledConnection<T> {
    connection: Option<T>,
    pool: Arc<tokio::sync::Mutex<Vec<T>>>,
}

impl<T> PooledConnection<T> {
    fn new(connection: T, pool: Arc<tokio::sync::Mutex<Vec<T>>>) -> Self {
        Self {
            connection: Some(connection),
            pool,
        }
    }
}

impl<T> std::ops::Deref for PooledConnection<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: connection is only None after Drop, which consumes self
        // Deref cannot be called after Drop, so this unwrap is safe
        self.connection.as_ref().expect("connection exists until Drop")
    }
}

impl<T> std::ops::DerefMut for PooledConnection<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: connection is only None after Drop, which consumes self
        // DerefMut cannot be called after Drop, so this unwrap is safe
        self.connection.as_mut().expect("connection exists until Drop")
    }
}

impl<T> Drop for PooledConnection<T> {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let pool = Arc::clone(&self.pool);
            tokio::spawn(async move {
                let mut pool = pool.lock().await;
                pool.push(connection);
            });
        }
    }
}

/// Pool-related errors
#[derive(Debug, Clone)]
pub enum PoolError {
    ConnectionFailed(String),
    PoolExhausted,
    Timeout,
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            Self::PoolExhausted => write!(f, "Connection pool exhausted"),
            Self::Timeout => write!(f, "Connection timeout"),
        }
    }
}

impl std::error::Error for PoolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_agent_id_hashing() {
        let id1 = CachedAgentId::new(skreaver_core::AgentId::new_unchecked("test-agent"));
        let id2 = CachedAgentId::new(skreaver_core::AgentId::new_unchecked("test-agent"));
        let id3 = CachedAgentId::new(skreaver_core::AgentId::new_unchecked("different-agent"));

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert_eq!(id1.hash, id2.hash);
        assert_ne!(id1.hash, id3.hash);
    }
    
    #[tokio::test]
    async fn test_optimized_registry() {
        let registry = OptimizedAgentRegistry::new();
        
        // Mock agent instance creation would go here
        // let agent_id = AgentId::new("test".to_string());
        // registry.insert_agent(agent_id.clone(), mock_agent).await;
        // let retrieved = registry.get_agent(&agent_id).await;
        // assert!(retrieved.is_some());
        
        let metrics = registry.get_metrics();
        assert_eq!(metrics.lookups.load(std::sync::atomic::Ordering::Relaxed), 0);
    }
    
    #[test]
    fn test_batch_config() {
        let config = BatchConfig::default();
        assert_eq!(config.max_batch_size, 10);
        assert!(config.enabled);
    }
}