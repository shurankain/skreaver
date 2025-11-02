//! Internal queue management for per-agent request queuing.

use std::collections::VecDeque;
use std::sync::{Arc, atomic::AtomicUsize};
use tokio::sync::Semaphore;

use super::request::{QueuedRequest, ResponseSender};

/// Per-agent queue state
pub(super) struct AgentQueue {
    pub(super) queue: VecDeque<(QueuedRequest, ResponseSender<String>)>,
    pub(super) active_requests: Arc<AtomicUsize>,
    pub(super) semaphore: Arc<Semaphore>,
    pub(super) total_processed: u64,
    pub(super) total_timeouts: u64,
    pub(super) total_rejections: u64,
    pub(super) recent_processing_times: VecDeque<u64>,
}

impl AgentQueue {
    pub(super) fn new(max_concurrent: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            active_requests: Arc::new(AtomicUsize::new(0)),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            total_processed: 0,
            total_timeouts: 0,
            total_rejections: 0,
            recent_processing_times: VecDeque::new(),
        }
    }

    pub(super) fn avg_processing_time(&self) -> f64 {
        if self.recent_processing_times.is_empty() {
            0.0
        } else {
            let sum: u64 = self.recent_processing_times.iter().sum();
            sum as f64 / self.recent_processing_times.len() as f64
        }
    }

    pub(super) fn add_processing_time(&mut self, time_ms: u64) {
        self.recent_processing_times.push_back(time_ms);
        // Keep only last 100 measurements
        if self.recent_processing_times.len() > 100 {
            self.recent_processing_times.pop_front();
        }
    }
}
