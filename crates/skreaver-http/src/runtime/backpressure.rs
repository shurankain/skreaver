//! # Backpressure and Request Queue Management
//!
//! This module provides sophisticated request queue management and backpressure
//! mechanisms to prevent system overload and ensure stable performance under
//! high load conditions using type-safe state management.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, Semaphore, mpsc, oneshot};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Configuration for backpressure and queue management
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Maximum number of requests in queue per agent
    pub max_queue_size: usize,
    /// Maximum number of concurrent requests per agent
    pub max_concurrent_requests: usize,
    /// Global maximum concurrent requests across all agents
    pub global_max_concurrent: usize,
    /// Request timeout in the queue
    pub queue_timeout: Duration,
    /// Processing timeout for individual requests
    pub processing_timeout: Duration,
    /// Enable adaptive backpressure based on system load
    pub enable_adaptive_backpressure: bool,
    /// Target processing time for adaptive backpressure (milliseconds)
    pub target_processing_time_ms: u64,
    /// Load factor threshold for triggering backpressure (0.0-1.0)
    pub load_threshold: f64,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 100,
            max_concurrent_requests: 10,
            global_max_concurrent: 500,
            queue_timeout: Duration::from_secs(30),
            processing_timeout: Duration::from_secs(60),
            enable_adaptive_backpressure: true,
            target_processing_time_ms: 1000,
            load_threshold: 0.8,
        }
    }
}

/// Priority levels for requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

// ============================================================================
// Typestate pattern for Request lifecycle
// ============================================================================

/// Marker type for Queued state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    pub queued_at: Instant,
    pub timeout: Duration,
    pub input: Option<String>,
}

/// Marker type for Processing state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Processing {
    pub started_at: Instant,
    pub queued_duration: Duration,
}

/// Marker type for Completed state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed {
    pub completed_at: Instant,
    pub processing_time: Duration,
    pub result: String,
}

/// Marker type for Failed state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    pub failed_at: Instant,
    pub error: String,
}

/// Type-safe request using typestate pattern.
/// The type parameter `S` represents the current state and enforces
/// valid state transitions at compile time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request<S> {
    id: Uuid,
    agent_id: String,
    priority: RequestPriority,
    state: S,
}

// ============================================================================
// Constructors and state-independent methods
// ============================================================================

impl Request<Queued> {
    /// Create a new request in Queued state
    pub fn new(agent_id: String, priority: RequestPriority, timeout: Duration) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_id,
            priority,
            state: Queued {
                queued_at: Instant::now(),
                timeout,
                input: None,
            },
        }
    }

    /// Set input data for the request
    pub fn with_input(mut self, input: String) -> Self {
        self.state.input = Some(input);
        self
    }

    /// Get when queued
    pub fn queued_at(&self) -> Instant {
        self.state.queued_at
    }

    /// Get timeout duration
    pub fn timeout_duration(&self) -> Duration {
        self.state.timeout
    }

    /// Get input data if available
    pub fn input(&self) -> Option<&str> {
        self.state.input.as_deref()
    }

    /// Check if request has timed out
    pub fn has_timed_out(&self) -> bool {
        self.state.queued_at.elapsed() > self.state.timeout
    }

    /// Transition to Processing state
    pub fn start_processing(self) -> Request<Processing> {
        let queued_duration = self.state.queued_at.elapsed();
        Request {
            id: self.id,
            agent_id: self.agent_id,
            priority: self.priority,
            state: Processing {
                started_at: Instant::now(),
                queued_duration,
            },
        }
    }

    /// Transition to Failed state (timeout in queue)
    pub fn fail_timeout(self) -> Request<Failed> {
        Request {
            id: self.id,
            agent_id: self.agent_id,
            priority: self.priority,
            state: Failed {
                failed_at: Instant::now(),
                error: format!(
                    "Request timed out in queue after {:?}",
                    self.state.queued_at.elapsed()
                ),
            },
        }
    }
}

impl<S> Request<S> {
    /// Get the request ID (available in all states)
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get the agent ID (available in all states)
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Get the priority (available in all states)
    pub fn priority(&self) -> RequestPriority {
        self.priority
    }
}

impl Request<Processing> {
    /// Get when processing started
    pub fn started_at(&self) -> Instant {
        self.state.started_at
    }

    /// Get how long request was queued before processing
    pub fn queued_duration(&self) -> Duration {
        self.state.queued_duration
    }

    /// Get current processing duration
    pub fn processing_duration(&self) -> Duration {
        self.state.started_at.elapsed()
    }

    /// Transition to Completed state
    pub fn complete(self, result: String) -> Request<Completed> {
        let processing_time = self.state.started_at.elapsed();
        Request {
            id: self.id,
            agent_id: self.agent_id,
            priority: self.priority,
            state: Completed {
                completed_at: Instant::now(),
                processing_time,
                result,
            },
        }
    }

    /// Transition to Failed state
    pub fn fail(self, error: String) -> Request<Failed> {
        Request {
            id: self.id,
            agent_id: self.agent_id,
            priority: self.priority,
            state: Failed {
                failed_at: Instant::now(),
                error,
            },
        }
    }
}

impl Request<Completed> {
    /// Get when completed
    pub fn completed_at(&self) -> Instant {
        self.state.completed_at
    }

    /// Get processing time
    pub fn processing_time(&self) -> Duration {
        self.state.processing_time
    }

    /// Get result
    pub fn result(&self) -> &str {
        &self.state.result
    }
}

impl Request<Failed> {
    /// Get when failed
    pub fn failed_at(&self) -> Instant {
        self.state.failed_at
    }

    /// Get error message
    pub fn error(&self) -> &str {
        &self.state.error
    }
}

// ============================================================================
// Backward compatibility: Type-erased request for storage
// ============================================================================

/// Legacy QueuedRequest for backward compatibility
#[derive(Debug, Clone)]
pub struct QueuedRequest {
    pub id: Uuid,
    pub agent_id: String,
    pub priority: RequestPriority,
    pub queued_at: Instant,
    pub timeout: Duration,
    pub metadata: HashMap<String, String>,
}

impl From<Request<Queued>> for QueuedRequest {
    fn from(request: Request<Queued>) -> Self {
        let mut metadata = HashMap::new();
        if let Some(input) = request.state.input {
            metadata.insert("input".to_string(), input);
        }

        QueuedRequest {
            id: request.id,
            agent_id: request.agent_id,
            priority: request.priority,
            queued_at: request.state.queued_at,
            timeout: request.state.timeout,
            metadata,
        }
    }
}

impl From<QueuedRequest> for Request<Queued> {
    fn from(request: QueuedRequest) -> Self {
        let input = request.metadata.get("input").cloned();
        Request {
            id: request.id,
            agent_id: request.agent_id,
            priority: request.priority,
            state: Queued {
                queued_at: request.queued_at,
                timeout: request.timeout,
                input,
            },
        }
    }
}

/// Response channel for queued requests
pub type ResponseSender<T> = oneshot::Sender<Result<T, BackpressureError>>;
pub type ResponseReceiver<T> = oneshot::Receiver<Result<T, BackpressureError>>;

/// Backpressure and queue management errors
#[derive(Debug, thiserror::Error)]
pub enum BackpressureError {
    #[error("Queue is full for agent {agent_id} (max: {max_size})")]
    QueueFull { agent_id: String, max_size: usize },

    #[error("Request timed out in queue after {timeout_ms}ms")]
    QueueTimeout { timeout_ms: u64 },

    #[error("Processing timeout after {timeout_ms}ms")]
    ProcessingTimeout { timeout_ms: u64 },

    #[error("System overloaded, rejecting requests (load: {load:.2})")]
    SystemOverloaded { load: f64 },

    #[error("Agent {agent_id} not found")]
    AgentNotFound { agent_id: String },

    #[error("Request cancelled")]
    RequestCancelled,

    #[error("Internal error: {message}")]
    Internal { message: String },
}

/// Metrics for queue monitoring
#[derive(Debug, Clone)]
pub struct QueueMetrics {
    pub queue_size: usize,
    pub active_requests: usize,
    pub total_processed: u64,
    pub total_timeouts: u64,
    pub total_rejections: u64,
    pub avg_processing_time_ms: f64,
    pub load_factor: f64,
}

/// Per-agent queue state
struct AgentQueue {
    queue: VecDeque<(QueuedRequest, ResponseSender<String>)>,
    active_requests: Arc<AtomicUsize>,
    semaphore: Arc<Semaphore>,
    total_processed: u64,
    total_timeouts: u64,
    recent_processing_times: VecDeque<u64>,
}

impl AgentQueue {
    fn new(max_concurrent: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            active_requests: Arc::new(AtomicUsize::new(0)),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            total_processed: 0,
            total_timeouts: 0,
            recent_processing_times: VecDeque::new(),
        }
    }

    fn avg_processing_time(&self) -> f64 {
        if self.recent_processing_times.is_empty() {
            0.0
        } else {
            let sum: u64 = self.recent_processing_times.iter().sum();
            sum as f64 / self.recent_processing_times.len() as f64
        }
    }

    fn add_processing_time(&mut self, time_ms: u64) {
        self.recent_processing_times.push_back(time_ms);
        // Keep only last 100 measurements
        if self.recent_processing_times.len() > 100 {
            self.recent_processing_times.pop_front();
        }
    }
}

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
        // Check system load first
        if self.config.enable_adaptive_backpressure {
            let load = self.calculate_system_load().await;
            if load > self.config.load_threshold {
                return Err(BackpressureError::SystemOverloaded { load });
            }
        }

        let (tx, rx) = oneshot::channel();
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
        // Check system load first
        if self.config.enable_adaptive_backpressure {
            let load = self.calculate_system_load().await;
            if load > self.config.load_threshold {
                return Err(BackpressureError::SystemOverloaded { load });
            }
        }

        let (tx, rx) = oneshot::channel();
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
            let input = request.metadata.get("input").cloned().unwrap_or_default();
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
            total_rejections: 0, // TODO: Track rejections
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
            total_rejections: 0,
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

    #[test]
    fn test_typestate_request_transitions() {
        // Create a request in Queued state
        let request = Request::new(
            "test-agent".to_string(),
            RequestPriority::Normal,
            Duration::from_secs(30),
        );

        assert_eq!(request.agent_id(), "test-agent");
        assert_eq!(request.priority(), RequestPriority::Normal);
        assert!(!request.has_timed_out());

        // Add input
        let request = request.with_input("test input".to_string());
        assert_eq!(request.input(), Some("test input"));

        // Transition to Processing
        let processing_request = request.start_processing();
        assert!(processing_request.processing_duration() < Duration::from_millis(100));

        // Transition to Completed
        let completed_request = processing_request.complete("result".to_string());
        assert_eq!(completed_request.result(), "result");
    }

    #[test]
    fn test_request_timeout_transition() {
        let request = Request::new(
            "test-agent".to_string(),
            RequestPriority::Normal,
            Duration::from_millis(1),
        );

        std::thread::sleep(Duration::from_millis(10));

        assert!(request.has_timed_out());

        let failed_request = request.fail_timeout();
        assert!(failed_request.error().contains("timed out"));
    }

    #[test]
    fn test_processing_to_failed() {
        let request = Request::new(
            "test-agent".to_string(),
            RequestPriority::Normal,
            Duration::from_secs(30),
        );

        let processing = request.start_processing();
        let failed = processing.fail("Processing failed".to_string());

        assert_eq!(failed.error(), "Processing failed");
    }
}
