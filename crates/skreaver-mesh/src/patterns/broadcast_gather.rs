//! Broadcast/Gather pattern for scatter-gather operations
//!
//! Broadcast a request to multiple agents and gather their responses.

use crate::{error::MeshResult, mesh::AgentMesh, message::Message, types::AgentId};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

/// Configuration for gather operation
#[derive(Debug, Clone)]
pub struct GatherConfig {
    /// Timeout for gathering responses
    pub timeout: Duration,
    /// Minimum number of responses required
    pub min_responses: usize,
    /// Maximum number of responses to wait for
    pub max_responses: usize,
}

impl Default for GatherConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            min_responses: 1,
            max_responses: usize::MAX,
        }
    }
}

/// Result of a gather operation
#[derive(Debug)]
pub struct GatherResult {
    /// Successfully received responses
    pub responses: Vec<(AgentId, Message)>,
    /// Agents that didn't respond
    pub missing: Vec<AgentId>,
    /// Whether minimum responses were received
    pub complete: bool,
}

/// Broadcast/Gather coordinator
pub struct BroadcastGather<M: AgentMesh> {
    mesh: Arc<M>,
}

impl<M: AgentMesh> BroadcastGather<M> {
    /// Create a new broadcast/gather coordinator
    pub fn new(mesh: Arc<M>) -> Self {
        Self { mesh }
    }

    /// Broadcast to all agents and gather responses
    pub async fn broadcast_gather(
        &self,
        targets: Vec<AgentId>,
        message: Message,
        config: GatherConfig,
    ) -> MeshResult<GatherResult> {
        let correlation_id = message.id.to_string();
        let responses = Arc::new(RwLock::new(Vec::new()));

        // Broadcast to all targets
        for target in &targets {
            let msg = message.clone().with_correlation_id(correlation_id.clone());
            self.mesh.send(target, msg).await?;
        }

        debug!(
            "Broadcast message {} to {} agents",
            correlation_id,
            targets.len()
        );

        // Wait for responses (simplified - in production would use proper response handling)
        tokio::time::sleep(config.timeout).await;

        let responses_vec = responses.read().await.clone();
        let received_count = responses_vec.len();

        let missing: Vec<AgentId> = targets
            .iter()
            .filter(|id| !responses_vec.iter().any(|(agent, _)| agent == *id))
            .cloned()
            .collect();

        let complete = received_count >= config.min_responses;

        Ok(GatherResult {
            responses: responses_vec,
            missing,
            complete,
        })
    }

    /// Send to multiple agents (no gather)
    pub async fn multicast(&self, targets: Vec<AgentId>, message: Message) -> MeshResult<()> {
        for target in targets {
            self.mesh.send(&target, message.clone()).await?;
        }
        Ok(())
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
    async fn test_multicast() {
        let mesh = Arc::new(MockMesh);
        let bg = BroadcastGather::new(mesh);

        let targets = vec![
            AgentId::from("agent-1"),
            AgentId::from("agent-2"),
            AgentId::from("agent-3"),
        ];

        let result = bg.multicast(targets, Message::new("test")).await;
        assert!(result.is_ok());
    }
}
