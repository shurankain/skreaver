//! Request/Reply pattern for RPC-style communication
//!
//! Provides synchronous-style request/reply over async messaging,
//! with timeout and correlation ID tracking.

use crate::{
    error::{MeshError, MeshResult},
    mesh::AgentMesh,
    message::Message,
    types::AgentId,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};
use tracing::{debug, warn};

/// Configuration for Request/Reply pattern
#[derive(Debug, Clone)]
pub struct RequestReplyConfig {
    /// Default timeout for requests
    pub default_timeout: Duration,
    /// Maximum number of pending requests
    pub max_pending: usize,
}

impl Default for RequestReplyConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            max_pending: 1000,
        }
    }
}

/// Pending request awaiting reply
pub(crate) struct PendingRequest {
    pub(crate) sender: oneshot::Sender<Message>,
    pub(crate) created_at: tokio::time::Instant,
}

/// Request/Reply coordinator
pub struct RequestReply<M: AgentMesh> {
    mesh: Arc<M>,
    config: RequestReplyConfig,
    pending: Arc<RwLock<HashMap<String, PendingRequest>>>,
}

impl<M: AgentMesh + 'static> RequestReply<M> {
    /// Create a new Request/Reply coordinator
    pub fn new(mesh: Arc<M>, config: RequestReplyConfig) -> Self {
        Self {
            mesh,
            config,
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default configuration
    pub fn with_defaults(mesh: Arc<M>) -> Self {
        Self::new(mesh, RequestReplyConfig::default())
    }

    /// Send a request and wait for reply
    pub async fn request(
        &self,
        to: &AgentId,
        request: Message,
        timeout: Option<Duration>,
    ) -> MeshResult<Message> {
        let timeout = timeout.unwrap_or(self.config.default_timeout);
        let correlation_id = request.id.to_string();

        // Check pending request limit
        {
            let pending = self.pending.read().await;
            if pending.len() >= self.config.max_pending {
                return Err(MeshError::QueueFull {
                    capacity: self.config.max_pending,
                    current: pending.len(),
                });
            }
        }

        // Create reply channel
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending.write().await;
            pending.insert(
                correlation_id.clone(),
                PendingRequest {
                    sender: tx,
                    created_at: tokio::time::Instant::now(),
                },
            );
        }

        debug!("Sending request {} to {}", correlation_id, to);

        // Send request with correlation ID
        let request_msg = request.with_correlation_id(correlation_id.clone());
        self.mesh.send(to, request_msg).await?;

        // Wait for reply with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(reply)) => {
                debug!("Received reply for request {}", correlation_id);
                Ok(reply)
            }
            Ok(Err(_)) => {
                warn!("Reply channel closed for request {}", correlation_id);
                Err(MeshError::ReceiveFailed("Reply channel closed".to_string()))
            }
            Err(_) => {
                // Cleanup pending request
                self.pending.write().await.remove(&correlation_id);
                warn!("Request {} timed out after {:?}", correlation_id, timeout);
                Err(MeshError::Timeout(timeout))
            }
        }
    }

    /// Handle incoming reply (call this when you receive a message)
    pub async fn handle_reply(&self, reply: Message) -> MeshResult<()> {
        if let Some(correlation_id) = reply.correlation_id.clone() {
            let mut pending = self.pending.write().await;

            if let Some(request) = pending.remove(&correlation_id) {
                debug!("Matched reply to request {}", correlation_id);
                if request.sender.send(reply).is_err() {
                    warn!("Failed to send reply for {}", correlation_id);
                }
            } else {
                warn!("Received reply for unknown request {}", correlation_id);
            }
        }

        Ok(())
    }

    /// Clean up expired pending requests
    pub async fn cleanup_expired(&self) -> usize {
        let mut pending = self.pending.write().await;
        let now = tokio::time::Instant::now();

        let initial_len = pending.len();
        pending.retain(|id, request| {
            let expired = now.duration_since(request.created_at) > self.config.default_timeout;
            if expired {
                debug!("Cleaning up expired request {}", id);
            }
            !expired
        });

        initial_len - pending.len()
    }

    /// Get number of pending requests
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }

    /// Start periodic cleanup task
    pub fn start_cleanup_task(self: Arc<Self>, interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;
                let cleaned = self.cleanup_expired().await;
                if cleaned > 0 {
                    debug!("Cleaned up {} expired requests", cleaned);
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::MessageStream;
    use crate::types::Topic;
    use async_trait::async_trait;

    struct MockMesh;

    #[async_trait]
    impl AgentMesh for MockMesh {
        async fn send(&self, _to: &AgentId, _message: Message) -> MeshResult<()> {
            Ok(())
        }
        async fn broadcast(&self, _message: Message) -> MeshResult<()> {
            Ok(())
        }
        async fn subscribe(&self, _topic: &Topic) -> MeshResult<MessageStream> {
            use futures::stream;
            Ok(Box::pin(stream::empty()))
        }
        async fn publish(&self, _topic: &Topic, _message: Message) -> MeshResult<()> {
            Ok(())
        }
        async fn unsubscribe(&self, _topic: &Topic) -> MeshResult<()> {
            Ok(())
        }
        async fn queue_depth(&self) -> MeshResult<usize> {
            Ok(0)
        }
        async fn is_reachable(&self, _agent_id: &AgentId) -> bool {
            true
        }
        async fn list_agents(&self) -> MeshResult<Vec<AgentId>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_request_reply_timeout() {
        let mesh = Arc::new(MockMesh);
        let rr = RequestReply::with_defaults(mesh);

        let request = Message::new("test");
        let result = rr
            .request(
                &AgentId::new_unchecked("agent-1"),
                request,
                Some(Duration::from_millis(100)),
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MeshError::Timeout(_)));
    }

    #[tokio::test]
    async fn test_pending_count() {
        let mesh = Arc::new(MockMesh);
        let rr = Arc::new(RequestReply::with_defaults(mesh));

        let rr_clone = Arc::clone(&rr);
        tokio::spawn(async move {
            let _ = rr_clone
                .request(
                    &AgentId::new_unchecked("agent-1"),
                    Message::new("test"),
                    Some(Duration::from_secs(10)),
                )
                .await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let count = rr.pending_count().await;
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let config = RequestReplyConfig {
            default_timeout: Duration::from_millis(50),
            max_pending: 10,
        };
        let mesh = Arc::new(MockMesh);
        let rr = Arc::new(RequestReply::new(mesh, config));

        // Manually insert a pending request without going through the request() method
        // to avoid the timeout mechanism removing it before cleanup runs
        let (tx, _rx) = oneshot::channel();
        let correlation_id = "test-correlation-id".to_string();
        {
            let mut pending = rr.pending.write().await;
            pending.insert(
                correlation_id.clone(),
                PendingRequest {
                    sender: tx,
                    created_at: tokio::time::Instant::now() - Duration::from_millis(100), // Already expired
                },
            );
        }

        assert_eq!(rr.pending_count().await, 1);

        let cleaned = rr.cleanup_expired().await;
        assert_eq!(cleaned, 1, "Expected 1 cleaned, got {}", cleaned);
        assert_eq!(rr.pending_count().await, 0);
    }
}
