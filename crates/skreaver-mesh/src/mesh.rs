//! Core AgentMesh trait for multi-agent communication

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::{
    error::MeshResult,
    message::Message,
    types::{AgentId, Topic},
};

/// Stream type for receiving messages
pub type MessageStream = Pin<Box<dyn Stream<Item = MeshResult<Message>> + Send + 'static>>;

/// Core trait for agent-to-agent communication
///
/// This trait defines the interface for multi-agent messaging systems,
/// supporting point-to-point, broadcast, and pub/sub patterns.
///
/// # Example
///
/// ```rust,no_run
/// use skreaver_mesh::{AgentMesh, Message, AgentId};
///
/// async fn example(mesh: impl AgentMesh) -> Result<(), Box<dyn std::error::Error>> {
///     // Send point-to-point message
///     let msg = Message::new("hello");
///     mesh.send(&AgentId::new_unchecked("agent-2"), msg).await?;
///
///     // Broadcast to all agents
///     let broadcast = Message::new("announcement");
///     mesh.broadcast(broadcast).await?;
///
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait AgentMesh: Send + Sync {
    /// Send a message to a specific agent
    ///
    /// # Parameters
    ///
    /// * `to` - The recipient agent ID
    /// * `message` - The message to send
    ///
    /// # Errors
    ///
    /// Returns `MeshError` if the message cannot be sent (agent not found,
    /// connection failure, queue full, etc.)
    async fn send(&self, to: &AgentId, message: Message) -> MeshResult<()>;

    /// Broadcast a message to all agents in the mesh
    ///
    /// # Parameters
    ///
    /// * `message` - The message to broadcast
    ///
    /// # Errors
    ///
    /// Returns `MeshError` if the broadcast fails
    async fn broadcast(&self, message: Message) -> MeshResult<()>;

    /// Subscribe to messages on a specific topic
    ///
    /// Returns a stream of messages published to the topic. The stream
    /// will receive all messages published after subscription.
    ///
    /// # Parameters
    ///
    /// * `topic` - The topic to subscribe to
    ///
    /// # Returns
    ///
    /// A stream of messages on the topic
    ///
    /// # Errors
    ///
    /// Returns `MeshError` if subscription fails
    async fn subscribe(&self, topic: &Topic) -> MeshResult<MessageStream>;

    /// Publish a message to a topic
    ///
    /// All agents subscribed to the topic will receive the message.
    ///
    /// # Parameters
    ///
    /// * `topic` - The topic to publish to
    /// * `message` - The message to publish
    ///
    /// # Errors
    ///
    /// Returns `MeshError` if publishing fails
    async fn publish(&self, topic: &Topic, message: Message) -> MeshResult<()>;

    /// Unsubscribe from a topic
    ///
    /// # Parameters
    ///
    /// * `topic` - The topic to unsubscribe from
    ///
    /// # Errors
    ///
    /// Returns `MeshError` if unsubscribe fails
    async fn unsubscribe(&self, topic: &Topic) -> MeshResult<()>;

    /// Get the current queue depth (pending messages)
    ///
    /// This is used for backpressure monitoring. Returns the number
    /// of messages waiting to be processed.
    ///
    /// # Errors
    ///
    /// Returns `MeshError` if queue depth cannot be determined
    async fn queue_depth(&self) -> MeshResult<usize>;

    /// Check if an agent is reachable in the mesh
    ///
    /// # Parameters
    ///
    /// * `agent_id` - The agent ID to check
    ///
    /// # Returns
    ///
    /// `true` if the agent is reachable, `false` otherwise
    async fn is_reachable(&self, agent_id: &AgentId) -> bool;

    /// List all agents currently in the mesh
    ///
    /// # Returns
    ///
    /// A list of agent IDs currently connected to the mesh
    ///
    /// # Errors
    ///
    /// Returns `MeshError` if the agent list cannot be retrieved
    async fn list_agents(&self) -> MeshResult<Vec<AgentId>>;
}

/// Extension trait for request/reply patterns
#[async_trait]
pub trait RequestReply: AgentMesh {
    /// Send a request and wait for a reply
    ///
    /// This is a convenience method for synchronous-style RPC over the mesh.
    /// It sends a message with a correlation ID and waits for a reply with
    /// the same correlation ID.
    ///
    /// # Parameters
    ///
    /// * `to` - The recipient agent ID
    /// * `request` - The request message
    /// * `timeout` - Maximum time to wait for a reply
    ///
    /// # Returns
    ///
    /// The reply message
    ///
    /// # Errors
    ///
    /// Returns `MeshError::Timeout` if no reply is received within the timeout
    async fn request(
        &self,
        to: &AgentId,
        request: Message,
        timeout: std::time::Duration,
    ) -> MeshResult<Message>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing trait interface
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
    async fn test_mock_mesh_send() {
        let mesh = MockMesh;
        let msg = Message::new("test");
        let result = mesh.send(&AgentId::new_unchecked("agent-1"), msg).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_mesh_broadcast() {
        let mesh = MockMesh;
        let msg = Message::new("broadcast");
        let result = mesh.broadcast(msg).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_mesh_queue_depth() {
        let mesh = MockMesh;
        let depth = mesh.queue_depth().await.unwrap();
        assert_eq!(depth, 0);
    }
}
