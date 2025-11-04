//! Pipeline pattern for sequential multi-agent processing
//!
//! Pass a message through a sequence of agents, where each agent
//! processes and transforms the message before passing to the next.

use crate::{error::MeshResult, mesh::AgentMesh, message::Message, types::AgentId};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// A stage in the pipeline
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Agent ID for this stage
    pub agent: AgentId,
    /// Stage name (for logging)
    pub name: String,
    /// Timeout for this stage
    pub timeout: Duration,
}

impl PipelineStage {
    /// Create a new pipeline stage
    pub fn new(agent: AgentId, name: impl Into<String>) -> Self {
        Self {
            agent,
            name: name.into(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Set timeout for this stage
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Pipeline coordinator
pub struct Pipeline<M: AgentMesh> {
    mesh: Arc<M>,
    stages: Vec<PipelineStage>,
}

impl<M: AgentMesh> Pipeline<M> {
    /// Create a new pipeline
    pub fn new(mesh: Arc<M>, stages: Vec<PipelineStage>) -> Self {
        Self { mesh, stages }
    }

    /// Execute the pipeline with an input message
    pub async fn execute(&self, mut message: Message) -> MeshResult<Message> {
        debug!("Starting pipeline with {} stages", self.stages.len());

        for (idx, stage) in self.stages.iter().enumerate() {
            debug!(
                "Pipeline stage {}/{}: {}",
                idx + 1,
                self.stages.len(),
                stage.name
            );

            // Send to stage agent
            self.mesh.send(&stage.agent, message.clone()).await?;

            // In a real implementation, would wait for response with timeout
            // For now, simplified - just pass the message through
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Message would be transformed by each stage
            message = message.with_metadata("stage", &stage.name);
        }

        debug!("Pipeline completed successfully");
        Ok(message)
    }

    /// Get number of stages
    pub fn stage_count(&self) -> usize {
        self.stages.len()
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
    async fn test_pipeline_execution() {
        let mesh = Arc::new(MockMesh);

        let stages = vec![
            PipelineStage::new(AgentId::new_unchecked("stage-1"), "validate"),
            PipelineStage::new(AgentId::new_unchecked("stage-2"), "process"),
            PipelineStage::new(AgentId::new_unchecked("stage-3"), "output"),
        ];

        let pipeline = Pipeline::new(mesh, stages);
        assert_eq!(pipeline.stage_count(), 3);

        let result = pipeline.execute(Message::new("test")).await;
        assert!(result.is_ok());
    }
}
