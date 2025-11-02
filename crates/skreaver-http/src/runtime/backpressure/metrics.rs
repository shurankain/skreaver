//! Metrics types for queue monitoring.

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
