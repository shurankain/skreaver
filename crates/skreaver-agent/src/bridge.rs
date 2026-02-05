//! Protocol bridge for connecting agents across protocols.
//!
//! This module provides the ability to expose agents from one protocol
//! to another, enabling cross-protocol communication.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::error::{AgentError, AgentResult};
use crate::traits::UnifiedAgent;
use crate::types::{AgentInfo, StreamEvent, TaskStatus, UnifiedMessage, UnifiedTask};

// Import A2A conversion functions when the feature is enabled
#[cfg(feature = "a2a")]
use crate::a2a::conversions::{
    a2a_to_unified_message, unified_info_to_a2a_card, unified_to_a2a_status,
    unified_to_a2a_stream_event, update_a2a_task_from_unified,
};

/// A bridge that exposes a unified agent as an A2A server handler.
///
/// This allows any agent implementing `UnifiedAgent` to be exposed
/// via the A2A protocol.
#[cfg(feature = "a2a")]
pub struct A2aBridgeHandler {
    agent: Arc<dyn UnifiedAgent>,
}

#[cfg(feature = "a2a")]
impl A2aBridgeHandler {
    /// Create a new A2A bridge handler for an agent.
    pub fn new(agent: Arc<dyn UnifiedAgent>) -> Self {
        Self { agent }
    }

    /// Get the underlying agent.
    pub fn agent(&self) -> &dyn UnifiedAgent {
        self.agent.as_ref()
    }
}

#[cfg(feature = "a2a")]
#[async_trait]
impl skreaver_a2a::AgentHandler for A2aBridgeHandler {
    fn agent_card(&self) -> skreaver_a2a::AgentCard {
        unified_info_to_a2a_card(self.agent.info())
    }

    async fn handle_message(
        &self,
        task: &mut skreaver_a2a::Task,
        message: skreaver_a2a::Message,
    ) -> Result<(), String> {
        let unified_message = a2a_to_unified_message(&message);

        // Send to the underlying agent
        let result = if task.messages.is_empty() {
            self.agent.send_message(unified_message).await
        } else {
            self.agent
                .send_message_to_task(&task.id, unified_message)
                .await
        };

        match result {
            Ok(unified_task) => {
                // Update the A2A task with results
                update_a2a_task_from_unified(task, &unified_task);
                Ok(())
            }
            Err(e) => {
                task.set_status(skreaver_a2a::TaskStatus::Failed);
                Err(e.to_string())
            }
        }
    }

    async fn on_cancel(&self, task: &skreaver_a2a::Task) -> Result<(), String> {
        self.agent
            .cancel_task(&task.id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn supports_streaming(&self) -> bool {
        self.agent.supports_streaming()
    }

    async fn handle_message_streaming(
        &self,
        task: &mut skreaver_a2a::Task,
        message: skreaver_a2a::Message,
        event_tx: tokio::sync::broadcast::Sender<skreaver_a2a::StreamingEvent>,
    ) -> Result<(), String> {
        let unified_message = a2a_to_unified_message(&message);

        // Get streaming response from underlying agent
        let mut stream = self
            .agent
            .send_message_streaming(unified_message)
            .await
            .map_err(|e| e.to_string())?;

        use futures::StreamExt;

        while let Some(result) = stream.next().await {
            match result {
                Ok(event) => {
                    // Convert unified event to A2A event
                    if let Some(a2a_event) = unified_to_a2a_stream_event(&event) {
                        // Update task status
                        if let StreamEvent::StatusUpdate { status, .. } = &event {
                            task.set_status(unified_to_a2a_status(*status));
                        }
                        let _ = event_tx.send(a2a_event);
                    }
                }
                Err(e) => {
                    task.set_status(skreaver_a2a::TaskStatus::Failed);
                    return Err(e.to_string());
                }
            }
        }

        Ok(())
    }
}

/// A proxy agent that forwards requests to another agent.
///
/// Useful for adding middleware, logging, or protocol translation.
pub struct ProxyAgent {
    info: AgentInfo,
    target: Arc<dyn UnifiedAgent>,
}

impl ProxyAgent {
    /// Create a new proxy agent.
    pub fn new(name: impl Into<String>, target: Arc<dyn UnifiedAgent>) -> Self {
        let target_info = target.info();
        let info = AgentInfo::new(format!("proxy-{}", target_info.id), name)
            .with_description(format!("Proxy to {}", target_info.name));

        // Copy protocols and capabilities from target
        let mut info = info;
        for proto in &target_info.protocols {
            info = info.with_protocol(*proto);
        }
        for cap in &target_info.capabilities {
            info = info.with_capability(cap.clone());
        }
        if target_info.supports_streaming {
            info = info.with_streaming();
        }

        Self { info, target }
    }

    /// Get the target agent.
    pub fn target(&self) -> &dyn UnifiedAgent {
        self.target.as_ref()
    }
}

#[async_trait]
impl UnifiedAgent for ProxyAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        debug!(
            target = %self.target.info().id,
            "Proxying message"
        );
        self.target.send_message(message).await
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        self.target.send_message_to_task(task_id, message).await
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<std::pin::Pin<Box<dyn futures::Stream<Item = AgentResult<StreamEvent>> + Send>>>
    {
        self.target.send_message_streaming(message).await
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.target.get_task(task_id).await
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.target.cancel_task(task_id).await
    }
}

/// A fan-out agent that sends messages to multiple agents.
pub struct FanOutAgent {
    info: AgentInfo,
    targets: Vec<Arc<dyn UnifiedAgent>>,
    tasks: RwLock<HashMap<String, UnifiedTask>>,
}

impl FanOutAgent {
    /// Create a new fan-out agent.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            info: AgentInfo::new(id, name),
            targets: Vec::new(),
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// Add a target agent.
    pub fn add_target(&mut self, agent: Arc<dyn UnifiedAgent>) {
        // Merge capabilities
        for cap in agent.capabilities() {
            if !self.info.capabilities.iter().any(|c| c.id == cap.id) {
                self.info.capabilities.push(cap.clone());
            }
        }
        self.targets.push(agent);
    }

    /// Get all target agents.
    pub fn targets(&self) -> &[Arc<dyn UnifiedAgent>] {
        &self.targets
    }
}

#[async_trait]
impl UnifiedAgent for FanOutAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        // Send to all targets concurrently
        let futures: Vec<_> = self
            .targets
            .iter()
            .map(|t| t.send_message(message.clone()))
            .collect();

        let results = futures::future::join_all(futures).await;

        // Combine results into a single task
        let mut combined = UnifiedTask::new_with_uuid();

        for result in results {
            match result {
                Ok(task) => {
                    // Add messages from this task
                    for msg in task.messages {
                        combined.add_message(msg);
                    }
                    // Add artifacts
                    for artifact in task.artifacts {
                        combined.add_artifact(artifact);
                    }
                }
                Err(e) => {
                    // Add error as message
                    combined.add_message(UnifiedMessage::agent(format!("Error: {}", e)));
                }
            }
        }

        combined.set_status(TaskStatus::Completed);

        // Store the task for later retrieval
        self.tasks
            .write()
            .await
            .insert(combined.id.clone(), combined.clone());

        Ok(combined)
    }

    async fn send_message_to_task(
        &self,
        _task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        // Fan-out doesn't really support continuing tasks
        self.send_message(message).await
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<std::pin::Pin<Box<dyn futures::Stream<Item = AgentResult<StreamEvent>> + Send>>>
    {
        // For streaming, we just use the first target that supports it
        for target in &self.targets {
            if target.supports_streaming() {
                return target.send_message_streaming(message).await;
            }
        }

        // Fall back to non-streaming
        let task = self.send_message(message).await?;
        let task_id = task.id.clone();

        let stream = async_stream::stream! {
            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Completed,
                message: None,
            });
        };

        Ok(Box::pin(stream))
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .cloned()
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.set_status(TaskStatus::Cancelled);
            Ok(task.clone())
        } else {
            Err(AgentError::TaskNotFound(task_id.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fan_out_agent_creation() {
        let agent = FanOutAgent::new("fan-out-1", "Fan Out Agent");
        assert_eq!(agent.info().id, "fan-out-1");
        assert!(agent.targets().is_empty());
    }
}
