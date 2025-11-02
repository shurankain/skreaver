//! Configuration types for backpressure management.

use std::time::Duration;

/// Backpressure strategy mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureMode {
    /// No adaptive backpressure - only enforce queue/concurrency limits
    Static,
    /// Adaptive backpressure based on system load and processing times
    Adaptive,
}

impl Default for BackpressureMode {
    fn default() -> Self {
        Self::Adaptive
    }
}

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
    /// Backpressure strategy mode
    pub mode: BackpressureMode,
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
            mode: BackpressureMode::default(),
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
