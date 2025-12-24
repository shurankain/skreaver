//! Dead Letter Queue for failed messages
//!
//! The DLQ stores messages that failed to be delivered, with TTL and volume limits.
//! Messages in the DLQ can be retried or inspected for debugging.

use crate::{error::MeshResult, message::Message};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Configuration for Dead Letter Queue
///
/// Use `Option<DlqConfig>` to enable/disable:
/// - `None` = DLQ disabled (failed messages are dropped)
/// - `Some(config)` = DLQ enabled with given config
#[derive(Debug, Clone)]
pub struct DlqConfig {
    /// Maximum number of messages in DLQ
    pub max_size: usize,
    /// Default TTL for messages in DLQ (seconds)
    pub default_ttl_secs: u64,
    /// Maximum retry attempts before permanent failure
    pub max_retries: u32,
}

impl Default for DlqConfig {
    fn default() -> Self {
        Self {
            max_size: 10_000,
            default_ttl_secs: 86400, // 24 hours
            max_retries: 3,
        }
    }
}

impl DlqConfig {
    /// Create a configuration for high-volume scenarios
    pub fn high_volume() -> Self {
        Self {
            max_size: 100_000,
            default_ttl_secs: 86400 * 7, // 7 days
            max_retries: 5,
        }
    }

    /// Create a configuration for low-latency scenarios with quick cleanup
    pub fn low_latency() -> Self {
        Self {
            max_size: 1_000,
            default_ttl_secs: 3600, // 1 hour
            max_retries: 1,
        }
    }
}

/// A message in the Dead Letter Queue with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqEntry {
    /// The original message
    pub message: Message,
    /// When the message was added to DLQ
    pub added_at: DateTime<Utc>,
    /// When the message expires (TTL)
    pub expires_at: DateTime<Utc>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Reason for DLQ placement
    pub failure_reason: String,
    /// Last error encountered
    pub last_error: Option<String>,
}

impl DlqEntry {
    /// Create a new DLQ entry
    pub fn new(message: Message, ttl_secs: u64, failure_reason: String) -> Self {
        let now = Utc::now();
        Self {
            message,
            added_at: now,
            expires_at: now + Duration::seconds(ttl_secs as i64),
            retry_count: 0,
            failure_reason,
            last_error: None,
        }
    }

    /// Check if the entry has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if retry limit has been reached
    pub fn has_exhausted_retries(&self, max_retries: u32) -> bool {
        self.retry_count >= max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self, error: Option<String>) {
        self.retry_count += 1;
        self.last_error = error;
    }
}

/// Statistics for the Dead Letter Queue
#[derive(Debug, Clone, Default)]
pub struct DlqStats {
    /// Total messages currently in DLQ
    pub current_size: usize,
    /// Total messages added to DLQ (lifetime)
    pub total_added: u64,
    /// Total messages removed from DLQ (lifetime)
    pub total_removed: u64,
    /// Total messages that expired
    pub total_expired: u64,
    /// Total messages that exhausted retries
    pub total_exhausted: u64,
    /// Total successful retries
    pub total_retried: u64,
}

/// Dead Letter Queue for failed messages
pub struct DeadLetterQueue {
    config: Option<DlqConfig>,
    queue: Arc<RwLock<VecDeque<DlqEntry>>>,
    stats: Arc<RwLock<DlqStats>>,
}

impl DeadLetterQueue {
    /// Create a new Dead Letter Queue
    ///
    /// Pass `Some(config)` to enable DLQ, `None` to disable (drops failed messages)
    pub fn new(config: Option<DlqConfig>) -> Self {
        Self {
            config,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(DlqStats::default())),
        }
    }

    /// Create a DLQ with default configuration (enabled)
    pub fn with_defaults() -> Self {
        Self::new(Some(DlqConfig::default()))
    }

    /// Create a disabled DLQ (failed messages will be dropped)
    pub fn disabled() -> Self {
        Self::new(None)
    }

    /// Check if DLQ is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.is_some()
    }

    /// Add a message to the DLQ
    pub async fn add(&self, message: Message, failure_reason: impl Into<String>) -> MeshResult<()> {
        let Some(config) = &self.config else {
            debug!("DLQ disabled, dropping failed message");
            return Ok(());
        };

        let mut queue = self.queue.write().await;
        let mut stats = self.stats.write().await;

        // Check size limit
        if queue.len() >= config.max_size {
            warn!("DLQ size limit reached, dropping oldest message");
            queue.pop_front();
        }

        // Create DLQ entry
        let entry = DlqEntry::new(message, config.default_ttl_secs, failure_reason.into());

        queue.push_back(entry);
        // CRIT-1: Use saturating arithmetic to prevent counter overflow
        stats.total_added = stats.total_added.saturating_add(1);
        stats.current_size = queue.len();

        debug!("Added message to DLQ (total: {})", queue.len());
        Ok(())
    }

    /// Get all messages from the DLQ (for inspection)
    pub async fn list(&self) -> Vec<DlqEntry> {
        let queue = self.queue.read().await;
        queue.iter().cloned().collect()
    }

    /// Get messages that are ready for retry
    pub async fn get_retriable(&self, limit: usize) -> Vec<DlqEntry> {
        let Some(config) = &self.config else {
            return Vec::new();
        };

        let queue = self.queue.read().await;
        queue
            .iter()
            .filter(|entry| !entry.is_expired() && !entry.has_exhausted_retries(config.max_retries))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Remove a message from the DLQ (after successful retry)
    pub async fn remove(&self, message_id: &str) -> MeshResult<()> {
        let mut queue = self.queue.write().await;
        let mut stats = self.stats.write().await;

        let initial_len = queue.len();
        queue.retain(|entry| entry.message.id.as_str() != message_id);

        if queue.len() < initial_len {
            // CRIT-1: Use saturating arithmetic to prevent counter overflow
            stats.total_removed = stats.total_removed.saturating_add(1);
            stats.current_size = queue.len();
            debug!("Removed message {} from DLQ", message_id);
        }

        Ok(())
    }

    /// Mark a message as retried
    pub async fn mark_retried(&self, message_id: &str, error: Option<String>) -> MeshResult<()> {
        let mut queue = self.queue.write().await;

        if let Some(entry) = queue
            .iter_mut()
            .find(|e| e.message.id.as_str() == message_id)
        {
            entry.increment_retry(error);
            debug!(
                "Marked message {} as retried (count: {})",
                message_id, entry.retry_count
            );
        }

        Ok(())
    }

    /// Clean up expired messages
    pub async fn cleanup_expired(&self) -> MeshResult<usize> {
        let mut queue = self.queue.write().await;
        let mut stats = self.stats.write().await;

        let initial_len = queue.len();
        queue.retain(|entry| !entry.is_expired());
        let removed = initial_len - queue.len();

        if removed > 0 {
            // CRIT-1: Use saturating arithmetic to prevent counter overflow
            stats.total_expired = stats.total_expired.saturating_add(removed as u64);
            stats.current_size = queue.len();
            debug!("Cleaned up {} expired messages from DLQ", removed);
        }

        Ok(removed)
    }

    /// Clean up messages that exhausted retries
    pub async fn cleanup_exhausted(&self) -> MeshResult<usize> {
        let Some(config) = &self.config else {
            return Ok(0);
        };

        let mut queue = self.queue.write().await;
        let mut stats = self.stats.write().await;

        let initial_len = queue.len();
        queue.retain(|entry| !entry.has_exhausted_retries(config.max_retries));
        let removed = initial_len - queue.len();

        if removed > 0 {
            // CRIT-1: Use saturating arithmetic to prevent counter overflow
            stats.total_exhausted = stats.total_exhausted.saturating_add(removed as u64);
            stats.current_size = queue.len();
            debug!("Cleaned up {} exhausted messages from DLQ", removed);
        }

        Ok(removed)
    }

    /// Get current statistics
    pub async fn stats(&self) -> DlqStats {
        self.stats.read().await.clone()
    }

    /// Get current queue size
    pub async fn size(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Clear all messages from DLQ
    pub async fn clear(&self) -> MeshResult<()> {
        let mut queue = self.queue.write().await;
        let mut stats = self.stats.write().await;

        let cleared = queue.len();
        queue.clear();
        stats.current_size = 0;

        debug!("Cleared {} messages from DLQ", cleared);
        Ok(())
    }

    /// Start periodic cleanup task
    pub fn start_cleanup_task(self: Arc<Self>, interval_secs: u64) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                if let Err(e) = self.cleanup_expired().await {
                    warn!("Failed to cleanup expired DLQ messages: {}", e);
                }

                if let Err(e) = self.cleanup_exhausted().await {
                    warn!("Failed to cleanup exhausted DLQ messages: {}", e);
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dlq_add_and_list() {
        let dlq = DeadLetterQueue::with_defaults();
        let msg = Message::new("test");

        dlq.add(msg.clone(), "test failure").await.unwrap();

        let entries = dlq.list().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message.id, msg.id);
    }

    #[tokio::test]
    async fn test_dlq_size_limit() {
        let config = Some(DlqConfig {
            max_size: 3,
            ..Default::default()
        });
        let dlq = DeadLetterQueue::new(config);

        // Add 5 messages, should keep only last 3
        for i in 0..5 {
            let msg = Message::new(format!("msg-{}", i));
            dlq.add(msg, "test").await.unwrap();
        }

        assert_eq!(dlq.size().await, 3);
    }

    #[tokio::test]
    async fn test_dlq_expiry() {
        let config = Some(DlqConfig {
            default_ttl_secs: 1, // 1 second TTL
            ..Default::default()
        });
        let dlq = DeadLetterQueue::new(config);

        let msg = Message::new("expiring message");
        dlq.add(msg, "test").await.unwrap();

        // Wait for expiry
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let removed = dlq.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(dlq.size().await, 0);
    }

    #[tokio::test]
    async fn test_dlq_retry_limit() {
        let config = Some(DlqConfig {
            max_retries: 2,
            ..Default::default()
        });
        let dlq = DeadLetterQueue::new(config);

        let msg = Message::new("retry test");
        let msg_id = msg.id.clone();
        dlq.add(msg, "test").await.unwrap();

        // Retry twice
        dlq.mark_retried(msg_id.as_str(), Some("error 1".to_string()))
            .await
            .unwrap();
        dlq.mark_retried(msg_id.as_str(), Some("error 2".to_string()))
            .await
            .unwrap();

        // Should be exhausted now
        let retriable = dlq.get_retriable(10).await;
        assert_eq!(retriable.len(), 0);

        let removed = dlq.cleanup_exhausted().await.unwrap();
        assert_eq!(removed, 1);
    }

    #[tokio::test]
    async fn test_dlq_stats() {
        let dlq = DeadLetterQueue::with_defaults();

        dlq.add(Message::new("msg1"), "test").await.unwrap();
        dlq.add(Message::new("msg2"), "test").await.unwrap();

        let stats = dlq.stats().await;
        assert_eq!(stats.total_added, 2);
        assert_eq!(stats.current_size, 2);
    }

    #[tokio::test]
    async fn test_dlq_disabled() {
        let dlq = DeadLetterQueue::disabled();

        assert!(!dlq.is_enabled());

        // Should drop messages when disabled
        dlq.add(Message::new("msg1"), "test").await.unwrap();
        assert_eq!(dlq.size().await, 0);

        let stats = dlq.stats().await;
        assert_eq!(stats.total_added, 0);
    }
}
