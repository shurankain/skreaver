//! Protocol bridge for connecting agents across protocols.
//!
//! This module provides the ability to expose agents from one protocol
//! to another, enabling cross-protocol communication.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info};

use crate::error::{AgentError, AgentResult};
use crate::traits::UnifiedAgent;
use crate::types::{AgentInfo, Protocol, StreamEvent, TaskStatus, UnifiedMessage, UnifiedTask};

// Used in feature-gated code
#[cfg(feature = "a2a")]
use crate::types::{ContentPart, MessageRole};

/// A registry of agents that can be discovered and used.
pub struct AgentRegistry {
    agents: Vec<Arc<dyn UnifiedAgent>>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self { agents: Vec::new() }
    }

    /// Register an agent.
    pub fn register(&mut self, agent: Arc<dyn UnifiedAgent>) {
        info!(
            agent_id = %agent.info().id,
            name = %agent.info().name,
            "Registering agent"
        );
        self.agents.push(agent);
    }

    /// Find an agent by ID.
    pub fn find(&self, id: &str) -> Option<Arc<dyn UnifiedAgent>> {
        self.agents.iter().find(|a| a.info().id == id).cloned()
    }

    /// Find agents by protocol.
    pub fn find_by_protocol(&self, protocol: Protocol) -> Vec<Arc<dyn UnifiedAgent>> {
        self.agents
            .iter()
            .filter(|a| a.supports_protocol(protocol))
            .cloned()
            .collect()
    }

    /// Find agents by capability.
    pub fn find_by_capability(&self, capability_id: &str) -> Vec<Arc<dyn UnifiedAgent>> {
        self.agents
            .iter()
            .filter(|a| a.capabilities().iter().any(|c| c.id == capability_id))
            .cloned()
            .collect()
    }

    /// List all registered agents.
    pub fn list(&self) -> &[Arc<dyn UnifiedAgent>] {
        &self.agents
    }

    /// Get count of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

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
}

impl FanOutAgent {
    /// Create a new fan-out agent.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            info: AgentInfo::new(id, name),
            targets: Vec::new(),
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

    async fn get_task(&self, _task_id: &str) -> AgentResult<UnifiedTask> {
        Err(AgentError::TaskNotFound(
            "Fan-out agent doesn't store tasks".to_string(),
        ))
    }

    async fn cancel_task(&self, _task_id: &str) -> AgentResult<UnifiedTask> {
        Err(AgentError::TaskNotFound(
            "Fan-out agent doesn't store tasks".to_string(),
        ))
    }
}

// ============================================================================
// A2A Conversion Helpers (only with a2a feature)
// ============================================================================

#[cfg(feature = "a2a")]
fn unified_info_to_a2a_card(info: &AgentInfo) -> skreaver_a2a::AgentCard {
    let mut card = skreaver_a2a::AgentCard::new(
        &info.id,
        &info.name,
        info.url.as_deref().unwrap_or("http://localhost"),
    );

    if let Some(desc) = &info.description {
        card = card.with_description(desc);
    }

    if info.supports_streaming {
        card = card.with_streaming();
    }

    // Convert capabilities to skills
    for cap in &info.capabilities {
        let mut skill = skreaver_a2a::AgentSkill::new(&cap.id, &cap.name);
        if let Some(desc) = &cap.description {
            skill = skill.with_description(desc);
        }
        card = card.with_skill(skill);
    }

    card
}

#[cfg(feature = "a2a")]
fn a2a_to_unified_message(message: &skreaver_a2a::Message) -> UnifiedMessage {
    let role = match message.role {
        skreaver_a2a::Role::User => MessageRole::User,
        skreaver_a2a::Role::Agent => MessageRole::Agent,
    };

    let content: Vec<ContentPart> = message
        .parts
        .iter()
        .map(|part| match part {
            skreaver_a2a::Part::Text(text) => ContentPart::Text {
                text: text.text.clone(),
            },
            skreaver_a2a::Part::Data(data) => {
                let data_str =
                    if let Some(base64) = data.data.get("base64").and_then(|v| v.as_str()) {
                        base64.to_string()
                    } else {
                        serde_json::to_string(&data.data).unwrap_or_default()
                    };
                ContentPart::Data {
                    data: data_str,
                    mime_type: data.media_type.clone(),
                    name: None,
                }
            }
            skreaver_a2a::Part::File(file) => ContentPart::File {
                uri: file.uri.clone(),
                mime_type: Some(file.media_type.clone()),
                name: file.name.clone(),
            },
        })
        .collect();

    let mut msg = UnifiedMessage::new(role, "");
    msg.id = message.id.clone().unwrap_or(msg.id);
    msg.content = content;
    msg.metadata = message.metadata.clone();
    msg.timestamp = message.timestamp;
    msg
}

#[cfg(feature = "a2a")]
fn update_a2a_task_from_unified(task: &mut skreaver_a2a::Task, unified: &UnifiedTask) {
    task.set_status(unified_to_a2a_status(unified.status));

    // Add new messages
    for msg in &unified.messages {
        let a2a_msg = unified_to_a2a_message(msg);
        task.add_message(a2a_msg);
    }

    // Add artifacts
    for artifact in &unified.artifacts {
        task.add_artifact(unified_to_a2a_artifact(artifact));
    }
}

#[cfg(feature = "a2a")]
fn unified_to_a2a_message(msg: &UnifiedMessage) -> skreaver_a2a::Message {
    let role = match msg.role {
        MessageRole::User => skreaver_a2a::Role::User,
        MessageRole::Agent | MessageRole::System => skreaver_a2a::Role::Agent,
    };

    let parts: Vec<skreaver_a2a::Part> = msg
        .content
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(skreaver_a2a::Part::Text(skreaver_a2a::TextPart {
                text: text.clone(),
                metadata: Default::default(),
            })),
            ContentPart::Data {
                data, mime_type, ..
            } => {
                let data_value = serde_json::json!({ "base64": data });
                Some(skreaver_a2a::Part::Data(skreaver_a2a::DataPart {
                    data: data_value,
                    media_type: mime_type.clone(),
                    metadata: Default::default(),
                }))
            }
            ContentPart::File {
                uri,
                mime_type,
                name,
            } => Some(skreaver_a2a::Part::File(skreaver_a2a::FilePart {
                uri: uri.clone(),
                media_type: mime_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                name: name.clone(),
                metadata: Default::default(),
            })),
            _ => None,
        })
        .collect();

    skreaver_a2a::Message {
        id: Some(msg.id.clone()),
        role,
        parts,
        reference_task_ids: Vec::new(),
        timestamp: msg.timestamp,
        metadata: msg.metadata.clone(),
    }
}

#[cfg(feature = "a2a")]
fn unified_to_a2a_artifact(artifact: &crate::types::Artifact) -> skreaver_a2a::Artifact {
    let parts: Vec<skreaver_a2a::Part> = artifact
        .content
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(skreaver_a2a::Part::Text(skreaver_a2a::TextPart {
                text: text.clone(),
                metadata: Default::default(),
            })),
            ContentPart::Data {
                data, mime_type, ..
            } => {
                let data_value = serde_json::json!({ "base64": data });
                Some(skreaver_a2a::Part::Data(skreaver_a2a::DataPart {
                    data: data_value,
                    media_type: mime_type.clone(),
                    metadata: Default::default(),
                }))
            }
            ContentPart::File {
                uri,
                mime_type,
                name,
            } => Some(skreaver_a2a::Part::File(skreaver_a2a::FilePart {
                uri: uri.clone(),
                media_type: mime_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                name: name.clone(),
                metadata: Default::default(),
            })),
            _ => None,
        })
        .collect();

    let mut a2a_artifact = skreaver_a2a::Artifact::new(&artifact.id).with_label(&artifact.name);
    for part in parts {
        a2a_artifact = a2a_artifact.with_part(part);
    }
    a2a_artifact
}

#[cfg(feature = "a2a")]
fn unified_to_a2a_status(status: TaskStatus) -> skreaver_a2a::TaskStatus {
    match status {
        TaskStatus::Pending => skreaver_a2a::TaskStatus::Working, // A2A doesn't have pending
        TaskStatus::Working => skreaver_a2a::TaskStatus::Working,
        TaskStatus::InputRequired => skreaver_a2a::TaskStatus::InputRequired,
        TaskStatus::Completed => skreaver_a2a::TaskStatus::Completed,
        TaskStatus::Failed => skreaver_a2a::TaskStatus::Failed,
        TaskStatus::Cancelled => skreaver_a2a::TaskStatus::Cancelled,
        TaskStatus::Rejected => skreaver_a2a::TaskStatus::Rejected,
    }
}

#[cfg(feature = "a2a")]
fn unified_to_a2a_stream_event(event: &StreamEvent) -> Option<skreaver_a2a::StreamingEvent> {
    match event {
        StreamEvent::StatusUpdate {
            task_id,
            status,
            message,
        } => {
            let a2a_message = message.as_ref().map(skreaver_a2a::Message::agent);
            Some(skreaver_a2a::StreamingEvent::TaskStatusUpdate(
                skreaver_a2a::TaskStatusUpdateEvent {
                    task_id: task_id.clone(),
                    status: unified_to_a2a_status(*status),
                    message: a2a_message,
                    timestamp: chrono::Utc::now(),
                },
            ))
        }
        StreamEvent::ArtifactAdded { task_id, artifact } => {
            Some(skreaver_a2a::StreamingEvent::TaskArtifactUpdate(
                skreaver_a2a::TaskArtifactUpdateEvent {
                    task_id: task_id.clone(),
                    artifact: unified_to_a2a_artifact(artifact),
                    is_final: true,
                    timestamp: chrono::Utc::now(),
                },
            ))
        }
        // Other events don't map to A2A streaming events
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_registry() {
        let registry = AgentRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_fan_out_agent_creation() {
        let agent = FanOutAgent::new("fan-out-1", "Fan Out Agent");
        assert_eq!(agent.info().id, "fan-out-1");
        assert!(agent.targets().is_empty());
    }
}
