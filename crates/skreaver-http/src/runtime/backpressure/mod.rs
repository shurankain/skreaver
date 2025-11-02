//! # Backpressure and Request Queue Management
//!
//! This module provides sophisticated request queue management and backpressure
//! mechanisms to prevent system overload and ensure stable performance under
//! high load conditions using type-safe state management.

use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, Semaphore, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

// Module declarations
mod config;
mod error;
mod metrics;
mod request;
mod queue;

// Public re-exports
pub use config::{BackpressureConfig, BackpressureMode, RequestPriority};
pub use error::BackpressureError;
pub use metrics::QueueMetrics;
pub use request::{
    Completed, Failed, Processing, Queued, QueuedRequest, Request, ResponseReceiver,
    ResponseSender,
};

// Internal imports
use queue::AgentQueue;

/// Main backpressure manager
pub struct BackpressureManager {
    config: BackpressureConfig,
    agent_queues: Arc<RwLock<HashMap<String, AgentQueue>>>,
    global_semaphore: Arc<Semaphore>,
    shutdown_tx: Arc<RwLock<Option<mpsc::UnboundedSender<()>>>>,
}

impl BackpressureManager {
    /// Create a new backpressure manager
    pub fn new(config: BackpressureConfig) -> Self {
        let global_semaphore = Arc::new(Semaphore::new(config.global_max_concurrent));

        Self {
            config,
            agent_queues: Arc::new(RwLock::new(HashMap::new())),
            global_semaphore,
            shutdown_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize background queue processing
    pub async fn start(&self) -> Result<(), BackpressureError> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();
        {
            let mut shutdown_tx_guard = self.shutdown_tx.write().await;
            *shutdown_tx_guard = Some(shutdown_tx);
        }

        let agent_queues = Arc::clone(&self.agent_queues);
        let config = self.config.clone();

        // Start queue processor task
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        Self::cleanup_expired_requests(&agent_queues, &config).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Backpressure manager shutting down");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Queue a request for processing with input data
    pub async fn queue_request_with_input(
        &self,
        agent_id: String,
        input: String,
        priority: RequestPriority,
        timeout: Option<Duration>,
    ) -> Result<(Uuid, ResponseReceiver<String>), BackpressureError> {
        // Check system load first if adaptive mode is enabled
        if self.config.mode == BackpressureMode::Adaptive {
            let load = self.calculate_system_load().await;
            if load > self.config.load_threshold {
                // Increment rejection counter for the agent
                {
                    let mut queues = self.agent_queues.write().await;
                    if let Some(queue) = queues.get_mut(&agent_id) {
                        queue.total_rejections += 1;
                    }
                }
                return Err(BackpressureError::SystemOverloaded { load });
            }
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        let timeout = timeout.unwrap_or(self.config.queue_timeout);

        // Create type-safe request
        let request = Request::new(agent_id.clone(), priority, timeout).with_input(input);
        let request_id = request.id();

        // Convert to legacy QueuedRequest for storage
        let queued_request: QueuedRequest = request.into();

        // Get or create agent queue
        {
            let mut queues = self.agent_queues.write().await;
            let queue = queues
                .entry(agent_id.clone())
                .or_insert_with(|| AgentQueue::new(self.config.max_concurrent_requests));

            // Check queue capacity
            if queue.queue.len() >= self.config.max_queue_size {
                queue.total_rejections += 1;
                return Err(BackpressureError::QueueFull {
                    agent_id,
                    max_size: self.config.max_queue_size,
                });
            }

            // Insert based on priority
            let insert_pos = queue
                .queue
                .iter()
                .position(|(req, _)| req.priority < priority)
                .unwrap_or(queue.queue.len());

            queue.queue.insert(insert_pos, (queued_request, tx));
        }

        Ok((request_id, rx))
    }

    /// Queue a request for processing
    pub async fn queue_request(
        &self,
        agent_id: String,
        priority: RequestPriority,
        timeout: Option<Duration>,
    ) -> Result<(Uuid, ResponseReceiver<String>), BackpressureError> {
        // Check system load first if adaptive mode is enabled
        if self.config.mode == BackpressureMode::Adaptive {
            let load = self.calculate_system_load().await;
            if load > self.config.load_threshold {
                // Increment rejection counter for the agent
                {
                    let mut queues = self.agent_queues.write().await;
                    if let Some(queue) = queues.get_mut(&agent_id) {
                        queue.total_rejections += 1;
                    }
                }
                return Err(BackpressureError::SystemOverloaded { load });
            }
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        let timeout = timeout.unwrap_or(self.config.queue_timeout);

        // Create type-safe request
        let request = Request::new(agent_id.clone(), priority, timeout);
        let request_id = request.id();

        // Convert to legacy QueuedRequest for storage
        let queued_request: QueuedRequest = request.into();

        // Get or create agent queue
        {
            let mut queues = self.agent_queues.write().await;
            let queue = queues
                .entry(agent_id.clone())
                .or_insert_with(|| AgentQueue::new(self.config.max_concurrent_requests));

            // Check queue capacity
            if queue.queue.len() >= self.config.max_queue_size {
                queue.total_rejections += 1;
                return Err(BackpressureError::QueueFull {
                    agent_id,
                    max_size: self.config.max_queue_size,
                });
            }

            // Insert based on priority
            let insert_pos = queue
                .queue
                .iter()
                .position(|(req, _)| req.priority < priority)
                .unwrap_or(queue.queue.len());

            queue.queue.insert(insert_pos, (queued_request, tx));
        }

        Ok((request_id, rx))
    }

    /// Process the next request for an agent using queued input
    pub async fn process_next_queued_request<F, Fut>(
        &self,
        agent_id: &str,
        processor: F,
    ) -> Option<()>
    where
        F: FnOnce(String) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = String> + Send + 'static,
    {
        // Try to get a request from the queue
        let (request, tx, semaphore, input) = {
            let mut queues = self.agent_queues.write().await;
            let queue = queues.get_mut(agent_id)?;

            if queue.queue.is_empty() {
                return None;
            }

            let (request, tx) = queue.queue.pop_front()?;
            let input = request.input.clone().unwrap_or_default();
            (request, tx, Arc::clone(&queue.semaphore), input)
        };

        // Acquire permits
        let _global_permit = match self.global_semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                // Global capacity exhausted, requeue the request
                {
                    let mut queues = self.agent_queues.write().await;
                    if let Some(queue) = queues.get_mut(agent_id) {
                        queue.queue.push_front((request, tx));
                    }
                }
                return None;
            }
        };

        let _local_permit = match semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                // Local capacity exhausted, requeue the request
                {
                    let mut queues = self.agent_queues.write().await;
                    if let Some(queue) = queues.get_mut(agent_id) {
                        queue.queue.push_front((request, tx));
                    }
                }
                return None;
            }
        };

        // Check if request has timed out while in queue
        if request.queued_at.elapsed() > request.timeout {
            let _ = tx.send(Err(BackpressureError::QueueTimeout {
                timeout_ms: request.timeout.as_millis() as u64,
            }));

            // Update metrics
            self.record_timeout(&request.agent_id).await;
            return Some(());
        }

        // Update active request count atomically
        let active_requests_clone = {
            let queues = self.agent_queues.read().await;
            if let Some(queue) = queues.get(&request.agent_id) {
                queue.active_requests.fetch_add(1, Ordering::Relaxed);
                Arc::clone(&queue.active_requests)
            } else {
                // Agent was removed while processing - rare race condition
                let _ = tx.send(Err(BackpressureError::AgentNotFound {
                    agent_id: request.agent_id.clone(),
                }));
                return Some(());
            }
        };

        let agent_id_clone = request.agent_id.clone();
        let agent_queues = Arc::clone(&self.agent_queues);
        let processing_timeout = self.config.processing_timeout;

        // Process request in background
        tokio::spawn(async move {
            let start_time = Instant::now();

            // Execute with timeout
            let result =
                tokio::time::timeout(processing_timeout, async { processor(input).await }).await;

            let processing_time = start_time.elapsed().as_millis() as u64;

            // Send result
            let response = match result {
                Ok(output) => Ok(output),
                Err(_) => Err(BackpressureError::ProcessingTimeout {
                    timeout_ms: processing_timeout.as_millis() as u64,
                }),
            };

            let _ = tx.send(response);

            // Update metrics - use atomic counter and batch queue updates
            active_requests_clone.fetch_sub(1, Ordering::Relaxed);

            // Update queue metrics in batch
            {
                let mut queues = agent_queues.write().await;
                if let Some(queue) = queues.get_mut(&agent_id_clone) {
                    queue.total_processed += 1;
                    queue.add_processing_time(processing_time);
                }
            }
        });

        Some(())
    }

    /// Process the next request for an agent
    pub async fn process_next_request<F, Fut>(
        &self,
        agent_id: &str,
        input: String,
        processor: F,
    ) -> Option<()>
    where
        F: FnOnce(String) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = String> + Send + 'static,
    {
        // Try to get a request from the queue
        let (request, tx, semaphore) = {
            let mut queues = self.agent_queues.write().await;
            let queue = queues.get_mut(agent_id)?;

            if queue.queue.is_empty() {
                return None;
            }

            let (request, tx) = queue.queue.pop_front()?;
            (request, tx, Arc::clone(&queue.semaphore))
        };

        // Acquire permits
        let _global_permit = match self.global_semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                // Global capacity exhausted, requeue the request
                {
                    let mut queues = self.agent_queues.write().await;
                    if let Some(queue) = queues.get_mut(agent_id) {
                        queue.queue.push_front((request, tx));
                    }
                }
                return None;
            }
        };

        let _local_permit = match semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                // Local capacity exhausted, requeue the request
                {
                    let mut queues = self.agent_queues.write().await;
                    if let Some(queue) = queues.get_mut(agent_id) {
                        queue.queue.push_front((request, tx));
                    }
                }
                return None;
            }
        };

        // Check if request has timed out while in queue
        if request.queued_at.elapsed() > request.timeout {
            let _ = tx.send(Err(BackpressureError::QueueTimeout {
                timeout_ms: request.timeout.as_millis() as u64,
            }));

            // Update metrics
            self.record_timeout(&request.agent_id).await;
            return Some(());
        }

        // Update active request count atomically
        let active_requests_clone = {
            let queues = self.agent_queues.read().await;
            if let Some(queue) = queues.get(&request.agent_id) {
                queue.active_requests.fetch_add(1, Ordering::Relaxed);
                Arc::clone(&queue.active_requests)
            } else {
                // Agent was removed while processing - rare race condition
                let _ = tx.send(Err(BackpressureError::AgentNotFound {
                    agent_id: request.agent_id.clone(),
                }));
                return Some(());
            }
        };

        let agent_id_clone = request.agent_id.clone();
        let agent_queues = Arc::clone(&self.agent_queues);
        let processing_timeout = self.config.processing_timeout;

        // Process request in background
        tokio::spawn(async move {
            let start_time = Instant::now();

            // Execute with timeout
            let result =
                tokio::time::timeout(processing_timeout, async { processor(input).await }).await;

            let processing_time = start_time.elapsed().as_millis() as u64;

            // Send result
            let response = match result {
                Ok(output) => Ok(output),
                Err(_) => Err(BackpressureError::ProcessingTimeout {
                    timeout_ms: processing_timeout.as_millis() as u64,
                }),
            };

            let _ = tx.send(response);

            // Update metrics - use atomic counter and batch queue updates
            active_requests_clone.fetch_sub(1, Ordering::Relaxed);

            // Update queue metrics in batch
            {
                let mut queues = agent_queues.write().await;
                if let Some(queue) = queues.get_mut(&agent_id_clone) {
                    queue.total_processed += 1;
                    queue.add_processing_time(processing_time);
                }
            }
        });

        Some(())
    }

    /// Get metrics for an agent
    pub async fn get_agent_metrics(&self, agent_id: &str) -> Option<QueueMetrics> {
        let queues = self.agent_queues.read().await;
        let queue = queues.get(agent_id)?;

        Some(QueueMetrics {
            queue_size: queue.queue.len(),
            active_requests: queue.active_requests.load(Ordering::Relaxed),
            total_processed: queue.total_processed,
            total_timeouts: queue.total_timeouts,
            total_rejections: queue.total_rejections,
            avg_processing_time_ms: queue.avg_processing_time(),
            load_factor: self.calculate_agent_load(queue).await,
        })
    }

    /// Get global system metrics
    pub async fn get_global_metrics(&self) -> QueueMetrics {
        let queues = self.agent_queues.read().await;

        let total_queue_size: usize = queues.values().map(|q| q.queue.len()).sum();
        let total_active: usize = queues
            .values()
            .map(|q| q.active_requests.load(Ordering::Relaxed))
            .sum();
        let total_processed: u64 = queues.values().map(|q| q.total_processed).sum();
        let total_timeouts: u64 = queues.values().map(|q| q.total_timeouts).sum();
        let total_rejections: u64 = queues.values().map(|q| q.total_rejections).sum();

        let avg_processing_time = if queues.is_empty() {
            0.0
        } else {
            queues
                .values()
                .map(|q| q.avg_processing_time())
                .sum::<f64>()
                / queues.len() as f64
        };

        QueueMetrics {
            queue_size: total_queue_size,
            active_requests: total_active,
            total_processed,
            total_timeouts,
            total_rejections,
            avg_processing_time_ms: avg_processing_time,
            load_factor: self.calculate_system_load().await,
        }
    }

    /// Calculate system load factor
    async fn calculate_system_load(&self) -> f64 {
        let queues = self.agent_queues.read().await;
        let total_active: usize = queues
            .values()
            .map(|q| q.active_requests.load(Ordering::Relaxed))
            .sum();

        total_active as f64 / self.config.global_max_concurrent as f64
    }

    /// Calculate load factor for a specific agent
    async fn calculate_agent_load(&self, queue: &AgentQueue) -> f64 {
        let active = queue.active_requests.load(Ordering::Relaxed);
        active as f64 / self.config.max_concurrent_requests as f64
    }

    /// Record a timeout for metrics
    async fn record_timeout(&self, agent_id: &str) {
        let mut queues = self.agent_queues.write().await;
        if let Some(queue) = queues.get_mut(agent_id) {
            queue.total_timeouts += 1;
        }
    }

    /// Clean up expired requests from queues
    async fn cleanup_expired_requests(
        agent_queues: &Arc<RwLock<HashMap<String, AgentQueue>>>,
        config: &BackpressureConfig,
    ) {
        let mut queues = agent_queues.write().await;
        let now = Instant::now();

        for (agent_id, queue) in queues.iter_mut() {
            let mut expired_count = 0;

            // Remove expired requests from front of queue
            while let Some((request, _)) = queue.queue.front() {
                if now.duration_since(request.queued_at) > config.queue_timeout {
                    if let Some((_, tx)) = queue.queue.pop_front() {
                        let _ = tx.send(Err(BackpressureError::QueueTimeout {
                            timeout_ms: config.queue_timeout.as_millis() as u64,
                        }));
                        expired_count += 1;
                        queue.total_timeouts += 1;
                    }
                } else {
                    break;
                }
            }

            if expired_count > 0 {
                warn!(
                    "Cleaned up {} expired requests for agent {}",
                    expired_count, agent_id
                );
            }
        }
    }
}

impl Drop for BackpressureManager {
    fn drop(&mut self) {
        // Use blocking call since Drop can't be async
        if let Ok(mut shutdown_tx_guard) = self.shutdown_tx.try_write()
            && let Some(shutdown_tx) = shutdown_tx_guard.take()
        {
            let _ = shutdown_tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_backpressure_manager_creation() {
        let config = BackpressureConfig::default();
        let manager = BackpressureManager::new(config);
        assert!(manager.agent_queues.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_queue_request() {
        let config = BackpressureConfig::default();
        let manager = BackpressureManager::new(config);
        manager.start().await.unwrap();

        let (request_id, _rx) = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();

        assert!(!request_id.is_nil());

        let metrics = manager.get_agent_metrics("test-agent").await.unwrap();
        assert_eq!(metrics.queue_size, 1);
    }

    #[tokio::test]
    async fn test_queue_full_rejection() {
        let config = BackpressureConfig {
            max_queue_size: 1,
            ..BackpressureConfig::default()
        };
        let manager = BackpressureManager::new(config);
        manager.start().await.unwrap();

        // First request should succeed
        let (_id1, _rx1) = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();

        // Second request should fail
        let result = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await;

        assert!(matches!(result, Err(BackpressureError::QueueFull { .. })));
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let config = BackpressureConfig::default();
        let manager = BackpressureManager::new(config);
        manager.start().await.unwrap();

        // Queue requests with different priorities
        let (_id1, _rx1) = manager
            .queue_request("test-agent".to_string(), RequestPriority::Low, None)
            .await
            .unwrap();

        let (_id2, _rx2) = manager
            .queue_request("test-agent".to_string(), RequestPriority::High, None)
            .await
            .unwrap();

        let (_id3, _rx3) = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();

        let metrics = manager.get_agent_metrics("test-agent").await.unwrap();
        assert_eq!(metrics.queue_size, 3);

        // Verify priority ordering by checking queue state
        let queues = manager.agent_queues.read().await;
        let queue = queues.get("test-agent").unwrap();

        // High priority should be first, then Normal, then Low
        assert_eq!(queue.queue[0].0.priority, RequestPriority::High);
        assert_eq!(queue.queue[1].0.priority, RequestPriority::Normal);
        assert_eq!(queue.queue[2].0.priority, RequestPriority::Low);
    }

    #[tokio::test]
    async fn test_processing_timeout() {
        let config = BackpressureConfig {
            processing_timeout: Duration::from_millis(100),
            ..BackpressureConfig::default()
        };
        let manager = BackpressureManager::new(config);
        manager.start().await.unwrap();

        let (_id, rx) = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();

        // Process with a slow operation
        manager
            .process_next_request("test-agent", "test-input".to_string(), |_input| async {
                sleep(Duration::from_millis(200)).await;
                "result".to_string()
            })
            .await;

        let result = rx.await.unwrap();
        assert!(matches!(
            result,
            Err(BackpressureError::ProcessingTimeout { .. })
        ));
    }

    #[tokio::test]
    async fn test_global_metrics() {
        let config = BackpressureConfig::default();
        let manager = BackpressureManager::new(config);
        manager.start().await.unwrap();

        // Add requests for multiple agents
        let (_id1, _rx1) = manager
            .queue_request("agent1".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();

        let (_id2, _rx2) = manager
            .queue_request("agent2".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();

        let global_metrics = manager.get_global_metrics().await;
        assert_eq!(global_metrics.queue_size, 2);
        assert_eq!(global_metrics.active_requests, 0);
    }

    #[tokio::test]
    async fn test_rejection_metrics() {
        let config = BackpressureConfig {
            max_queue_size: 2,
            ..BackpressureConfig::default()
        };
        let manager = BackpressureManager::new(config);
        manager.start().await.unwrap();

        // Queue two requests to fill the queue
        let (_id1, _rx1) = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();
        let (_id2, _rx2) = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();

        // Verify metrics before rejections
        let metrics = manager.get_agent_metrics("test-agent").await.unwrap();
        assert_eq!(metrics.queue_size, 2);
        assert_eq!(metrics.total_rejections, 0);

        // Try to queue a third request - should be rejected due to queue full
        let result = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await;
        assert!(matches!(result, Err(BackpressureError::QueueFull { .. })));

        // Verify rejection was counted
        let metrics = manager.get_agent_metrics("test-agent").await.unwrap();
        assert_eq!(metrics.total_rejections, 1);

        // Try another rejection
        let result = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await;
        assert!(matches!(result, Err(BackpressureError::QueueFull { .. })));

        // Verify second rejection was counted
        let metrics = manager.get_agent_metrics("test-agent").await.unwrap();
        assert_eq!(metrics.total_rejections, 2);

        // Verify global metrics include rejections
        let global_metrics = manager.get_global_metrics().await;
        assert_eq!(global_metrics.total_rejections, 2);
    }

    #[tokio::test]
    async fn test_system_overload_rejection_metrics() {
        let config = BackpressureConfig {
            mode: BackpressureMode::Adaptive,
            load_threshold: 0.01,     // Very low threshold
            global_max_concurrent: 1, // Low global limit to make it easy to trigger overload
            ..BackpressureConfig::default()
        };
        let manager = BackpressureManager::new(config);
        manager.start().await.unwrap();

        // First, queue a request to create the agent queue and increase load
        let (_id1, _rx1) = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await
            .unwrap();

        // Simulate active processing to increase system load
        {
            let queues = manager.agent_queues.read().await;
            if let Some(queue) = queues.get("test-agent") {
                queue.active_requests.store(1, Ordering::Relaxed);
            }
        }

        // Now try to queue another request - should be rejected due to system overload
        let result = manager
            .queue_request("test-agent".to_string(), RequestPriority::Normal, None)
            .await;

        // Should be rejected with SystemOverloaded error
        assert!(matches!(
            result,
            Err(BackpressureError::SystemOverloaded { .. })
        ));

        // Verify rejection was counted
        let metrics = manager.get_agent_metrics("test-agent").await.unwrap();
        assert_eq!(metrics.total_rejections, 1);
    }
}
