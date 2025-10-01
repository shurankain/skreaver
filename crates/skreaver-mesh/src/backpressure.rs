//! Backpressure management for mesh operations
//!
//! Monitors queue depths and provides flow control signals to prevent
//! message queue overflow.

use crate::error::{MeshError, MeshResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Backpressure configuration
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Queue depth threshold for warning (soft limit)
    pub warning_threshold: usize,
    /// Queue depth threshold for blocking (hard limit)
    pub blocking_threshold: usize,
    /// How often to check queue depth (seconds)
    pub check_interval_secs: u64,
    /// Enable backpressure monitoring
    pub enabled: bool,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            warning_threshold: 1000,
            blocking_threshold: 5000,
            check_interval_secs: 5,
            enabled: true,
        }
    }
}

/// Backpressure signal indicating system load
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureSignal {
    /// Normal operation - no backpressure
    Normal,
    /// Warning level - approaching limits
    Warning,
    /// Critical level - should block new messages
    Critical,
}

/// Backpressure statistics
#[derive(Debug, Clone, Default)]
pub struct BackpressureStats {
    /// Current queue depth
    pub current_depth: usize,
    /// Maximum queue depth observed
    pub max_depth: usize,
    /// Number of times warning threshold was exceeded
    pub warning_count: u64,
    /// Number of times critical threshold was exceeded
    pub critical_count: u64,
    /// Last check timestamp
    pub last_check: Option<Instant>,
}

/// Backpressure monitor for mesh operations
pub struct BackpressureMonitor {
    config: BackpressureConfig,
    stats: Arc<RwLock<BackpressureStats>>,
    current_signal: Arc<RwLock<BackpressureSignal>>,
}

impl BackpressureMonitor {
    /// Create a new backpressure monitor
    pub fn new(config: BackpressureConfig) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(BackpressureStats::default())),
            current_signal: Arc::new(RwLock::new(BackpressureSignal::Normal)),
        }
    }

    /// Create a monitor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(BackpressureConfig::default())
    }

    /// Update queue depth and calculate backpressure signal
    pub async fn update_depth(&self, depth: usize) -> BackpressureSignal {
        if !self.config.enabled {
            return BackpressureSignal::Normal;
        }

        let mut stats = self.stats.write().await;
        let mut signal = self.current_signal.write().await;

        stats.current_depth = depth;
        stats.last_check = Some(Instant::now());

        if depth > stats.max_depth {
            stats.max_depth = depth;
        }

        // Calculate new signal
        let new_signal = if depth >= self.config.blocking_threshold {
            stats.critical_count += 1;
            warn!(
                "Backpressure CRITICAL: depth {} >= threshold {}",
                depth, self.config.blocking_threshold
            );
            BackpressureSignal::Critical
        } else if depth >= self.config.warning_threshold {
            stats.warning_count += 1;
            debug!(
                "Backpressure WARNING: depth {} >= threshold {}",
                depth, self.config.warning_threshold
            );
            BackpressureSignal::Warning
        } else {
            BackpressureSignal::Normal
        };

        *signal = new_signal;
        new_signal
    }

    /// Get current backpressure signal
    pub async fn signal(&self) -> BackpressureSignal {
        *self.current_signal.read().await
    }

    /// Check if new messages should be blocked
    pub async fn should_block(&self) -> bool {
        self.signal().await == BackpressureSignal::Critical
    }

    /// Check if warning threshold is exceeded
    pub async fn is_warning(&self) -> bool {
        matches!(
            self.signal().await,
            BackpressureSignal::Warning | BackpressureSignal::Critical
        )
    }

    /// Get current statistics
    pub async fn stats(&self) -> BackpressureStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = BackpressureStats::default();
        debug!("Reset backpressure statistics");
    }

    /// Wait until backpressure subsides
    ///
    /// Blocks until the signal is no longer Critical, with timeout
    pub async fn wait_for_capacity(&self, timeout: Duration) -> MeshResult<()> {
        let start = Instant::now();

        while self.should_block().await {
            if start.elapsed() > timeout {
                return Err(MeshError::Timeout(timeout));
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }
}

/// Backpressure-aware queue wrapper
pub struct BackpressureQueue<T> {
    queue: Arc<RwLock<Vec<T>>>,
    monitor: Arc<BackpressureMonitor>,
}

impl<T> BackpressureQueue<T> {
    /// Create a new backpressure-aware queue
    pub fn new(config: BackpressureConfig) -> Self {
        Self {
            queue: Arc::new(RwLock::new(Vec::new())),
            monitor: Arc::new(BackpressureMonitor::new(config)),
        }
    }

    /// Push an item to the queue with backpressure check
    pub async fn push(&self, item: T, timeout: Duration) -> MeshResult<()> {
        // Wait for capacity if under backpressure
        self.monitor.wait_for_capacity(timeout).await?;

        let mut queue = self.queue.write().await;
        queue.push(item);

        // Update backpressure monitor
        self.monitor.update_depth(queue.len()).await;

        Ok(())
    }

    /// Pop an item from the queue
    pub async fn pop(&self) -> Option<T> {
        let mut queue = self.queue.write().await;
        let item = queue.pop();

        // Update backpressure monitor
        self.monitor.update_depth(queue.len()).await;

        item
    }

    /// Get current queue length
    pub async fn len(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        self.queue.read().await.is_empty()
    }

    /// Get backpressure signal
    pub async fn backpressure_signal(&self) -> BackpressureSignal {
        self.monitor.signal().await
    }

    /// Get backpressure statistics
    pub async fn backpressure_stats(&self) -> BackpressureStats {
        self.monitor.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backpressure_normal() {
        let monitor = BackpressureMonitor::with_defaults();
        let signal = monitor.update_depth(100).await;
        assert_eq!(signal, BackpressureSignal::Normal);
    }

    #[tokio::test]
    async fn test_backpressure_warning() {
        let config = BackpressureConfig {
            warning_threshold: 100,
            blocking_threshold: 200,
            ..Default::default()
        };
        let monitor = BackpressureMonitor::new(config);

        let signal = monitor.update_depth(150).await;
        assert_eq!(signal, BackpressureSignal::Warning);
    }

    #[tokio::test]
    async fn test_backpressure_critical() {
        let config = BackpressureConfig {
            warning_threshold: 100,
            blocking_threshold: 200,
            ..Default::default()
        };
        let monitor = BackpressureMonitor::new(config);

        let signal = monitor.update_depth(250).await;
        assert_eq!(signal, BackpressureSignal::Critical);
        assert!(monitor.should_block().await);
    }

    #[tokio::test]
    async fn test_backpressure_stats() {
        let monitor = BackpressureMonitor::with_defaults();

        monitor.update_depth(100).await;
        monitor.update_depth(2000).await; // Warning
        monitor.update_depth(6000).await; // Critical

        let stats = monitor.stats().await;
        assert_eq!(stats.max_depth, 6000);
        assert!(stats.warning_count > 0);
        assert!(stats.critical_count > 0);
    }

    #[tokio::test]
    async fn test_backpressure_queue() {
        let config = BackpressureConfig {
            warning_threshold: 2,
            blocking_threshold: 4,
            ..Default::default()
        };
        let queue: BackpressureQueue<String> = BackpressureQueue::new(config);

        // Add first item - should be normal
        queue
            .push("item1".to_string(), Duration::from_secs(1))
            .await
            .unwrap();

        assert_eq!(
            queue.backpressure_signal().await,
            BackpressureSignal::Normal
        );

        // Add second item - reaches warning threshold
        queue
            .push("item2".to_string(), Duration::from_secs(1))
            .await
            .unwrap();

        assert!(matches!(
            queue.backpressure_signal().await,
            BackpressureSignal::Warning | BackpressureSignal::Normal
        ));

        assert_eq!(queue.len().await, 2);
    }

    #[tokio::test]
    async fn test_wait_for_capacity_timeout() {
        let config = BackpressureConfig {
            warning_threshold: 1,
            blocking_threshold: 2,
            ..Default::default()
        };
        let monitor = BackpressureMonitor::new(config);

        // Set to critical
        monitor.update_depth(10).await;

        // Should timeout
        let result = monitor.wait_for_capacity(Duration::from_millis(100)).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MeshError::Timeout(_)));
    }
}
