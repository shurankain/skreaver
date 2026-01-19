//! A2A protocol adapter for the unified agent interface.
//!
//! This module provides adapters to use A2A agents through the unified interface.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tracing::info;

use crate::error::{AgentError, AgentResult};
use crate::traits::UnifiedAgent;
use crate::types::{
    AgentInfo, Artifact, Capability, ContentPart, MessageRole, Protocol, StreamEvent, TaskStatus,
    UnifiedMessage, UnifiedTask,
};

use skreaver_a2a::{
    A2aClient, AgentCard, AgentInterface, DataPart as A2aDataPart, FilePart as A2aFilePart,
    Message as A2aMessage, Part as A2aPart, StreamingEvent as A2aStreamingEvent, Task as A2aTask,
    TaskStatus as A2aTaskStatus, TextPart as A2aTextPart,
};

/// Adapter that wraps an A2A client to provide the unified agent interface.
pub struct A2aAgentAdapter {
    info: AgentInfo,
    client: A2aClient,
    agent_card: Option<Arc<AgentCard>>,
}

impl std::fmt::Debug for A2aAgentAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aAgentAdapter")
            .field("info", &self.info)
            .finish()
    }
}

impl A2aAgentAdapter {
    /// Create a new A2A adapter from a client.
    pub fn new(client: A2aClient) -> Self {
        let base_url = client.base_url().to_string();
        let info = AgentInfo::new(&base_url, &base_url).with_protocol(Protocol::A2a);

        Self {
            info,
            client,
            agent_card: None,
        }
    }

    /// Connect to an A2A agent and create an adapter.
    pub async fn connect(url: &str) -> AgentResult<Self> {
        info!(url = %url, "Connecting to A2A agent");
        let client = A2aClient::new(url).map_err(|e| AgentError::ConnectionError(e.to_string()))?;
        let mut adapter = Self::new(client);
        adapter.discover().await?;
        Ok(adapter)
    }

    /// Connect with bearer token authentication.
    pub async fn connect_with_bearer(url: &str, token: impl Into<String>) -> AgentResult<Self> {
        let client = A2aClient::new(url)
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?
            .with_bearer_token(token);
        let mut adapter = Self::new(client);
        adapter.discover().await?;
        Ok(adapter)
    }

    /// Connect with API key authentication.
    pub async fn connect_with_api_key(
        url: &str,
        header: impl Into<String>,
        key: impl Into<String>,
    ) -> AgentResult<Self> {
        let client = A2aClient::new(url)
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?
            .with_api_key(header, key);
        let mut adapter = Self::new(client);
        adapter.discover().await?;
        Ok(adapter)
    }

    /// Discover the agent's capabilities.
    pub async fn discover(&mut self) -> AgentResult<()> {
        let card = self
            .client
            .get_agent_card()
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        // Update info from agent card
        self.info = a2a_card_to_agent_info(&card);
        self.agent_card = Some(Arc::new(card));

        Ok(())
    }

    /// Get the cached agent card.
    pub fn agent_card(&self) -> Option<&AgentCard> {
        self.agent_card.as_deref()
    }

    /// Get the underlying client.
    pub fn client(&self) -> &A2aClient {
        &self.client
    }
}

#[async_trait]
impl UnifiedAgent for A2aAgentAdapter {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        let a2a_message = unified_to_a2a_message(&message);

        let task = self
            .client
            .send(a2a_message, None, None)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        Ok(a2a_to_unified_task(&task))
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        let a2a_message = unified_to_a2a_message(&message);

        let task = self
            .client
            .send(a2a_message, Some(task_id.to_string()), None)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        Ok(a2a_to_unified_task(&task))
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        let a2a_message = unified_to_a2a_message(&message);

        let stream = self
            .client
            .send_streaming(a2a_message, None, None)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        // Map the stream
        use futures::StreamExt;
        let mapped = stream.map(|result| {
            result
                .map(|event| a2a_to_unified_stream_event(&event))
                .map_err(|e| AgentError::ConnectionError(e.to_string()))
        });

        Ok(Box::pin(mapped))
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let task = self.client.get_task(task_id).await.map_err(|e| match e {
            skreaver_a2a::A2aError::TaskNotFound { .. } => {
                AgentError::TaskNotFound(task_id.to_string())
            }
            _ => AgentError::ConnectionError(e.to_string()),
        })?;

        Ok(a2a_to_unified_task(&task))
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let task = self
            .client
            .cancel_task(task_id, None)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        Ok(a2a_to_unified_task(&task))
    }
}

// ============================================================================
// Conversion Functions
// ============================================================================

/// Extract base URL from AgentCard interfaces.
fn get_base_url_from_card(card: &AgentCard) -> Option<String> {
    for interface in &card.interfaces {
        if let AgentInterface::Http { base_url } = interface {
            return Some(base_url.clone());
        }
    }
    None
}

/// Convert A2A AgentCard to unified AgentInfo.
fn a2a_card_to_agent_info(card: &AgentCard) -> AgentInfo {
    let url = get_base_url_from_card(card);

    let mut info = AgentInfo::new(&card.agent_id, &card.name).with_protocol(Protocol::A2a);

    if let Some(url) = url {
        info = info.with_url(url);
    }

    if let Some(desc) = &card.description {
        info = info.with_description(desc);
    }

    // Convert skills to capabilities
    for skill in &card.skills {
        let mut cap = Capability::new(&skill.id, &skill.name).with_tag("a2a");
        if let Some(desc) = &skill.description {
            cap = cap.with_description(desc);
        }
        for tag in &skill.tags {
            cap = cap.with_tag(tag);
        }
        info = info.with_capability(cap);
    }

    // Check streaming support
    if card.capabilities.streaming {
        info = info.with_streaming();
    }

    info
}

/// Convert unified message to A2A message.
fn unified_to_a2a_message(message: &UnifiedMessage) -> A2aMessage {
    let role = match message.role {
        MessageRole::User => skreaver_a2a::Role::User,
        MessageRole::Agent => skreaver_a2a::Role::Agent,
        MessageRole::System => skreaver_a2a::Role::User, // A2A doesn't have system role
    };

    let parts: Vec<A2aPart> = message
        .content
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(A2aPart::Text(A2aTextPart {
                text: text.clone(),
                metadata: Default::default(),
            })),
            ContentPart::Data {
                data, mime_type, ..
            } => {
                // Convert base64 string to JSON value
                let data_value = serde_json::json!({ "base64": data });
                Some(A2aPart::Data(A2aDataPart {
                    data: data_value,
                    media_type: mime_type.clone(),
                    metadata: Default::default(),
                }))
            }
            ContentPart::File {
                uri,
                mime_type,
                name,
            } => Some(A2aPart::File(A2aFilePart {
                uri: uri.clone(),
                media_type: mime_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                name: name.clone(),
                metadata: Default::default(),
            })),
            // Tool calls/results don't map directly to A2A
            ContentPart::ToolCall { .. } | ContentPart::ToolResult { .. } => None,
        })
        .collect();

    A2aMessage {
        id: Some(message.id.clone()),
        role,
        parts,
        reference_task_ids: Vec::new(),
        timestamp: message.timestamp,
        metadata: message
            .metadata
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    }
}

/// Convert A2A task to unified task.
fn a2a_to_unified_task(task: &A2aTask) -> UnifiedTask {
    let mut unified = UnifiedTask::new(&task.id);
    unified.session_id = task.context_id.clone();
    unified.status = a2a_to_unified_status(&task.status);

    // Convert messages
    for msg in &task.messages {
        unified.messages.push(a2a_to_unified_message(msg));
    }

    // Convert artifacts
    for artifact in &task.artifacts {
        unified.artifacts.push(a2a_to_unified_artifact(artifact));
    }

    // Copy metadata
    unified.metadata = task.metadata.clone();
    unified.created_at = task.created_at;
    unified.updated_at = task.updated_at;

    unified
}

/// Convert A2A message to unified message.
fn a2a_to_unified_message(message: &A2aMessage) -> UnifiedMessage {
    let role = match message.role {
        skreaver_a2a::Role::User => MessageRole::User,
        skreaver_a2a::Role::Agent => MessageRole::Agent,
    };

    let content: Vec<ContentPart> = message
        .parts
        .iter()
        .map(|part| match part {
            A2aPart::Text(text) => ContentPart::Text {
                text: text.text.clone(),
            },
            A2aPart::Data(data) => {
                // Try to extract base64 if present, otherwise serialize data
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
            A2aPart::File(file) => ContentPart::File {
                uri: file.uri.clone(),
                mime_type: Some(file.media_type.clone()),
                name: file.name.clone(),
            },
        })
        .collect();

    let mut unified = UnifiedMessage::new(role, "");
    unified.id = message.id.clone().unwrap_or(unified.id);
    unified.content = content;
    unified.metadata = message.metadata.clone();
    unified.timestamp = message.timestamp;
    unified
}

/// Convert A2A artifact to unified artifact.
fn a2a_to_unified_artifact(artifact: &skreaver_a2a::Artifact) -> Artifact {
    let content: Vec<ContentPart> = artifact
        .parts
        .iter()
        .map(|part| match part {
            A2aPart::Text(text) => ContentPart::Text {
                text: text.text.clone(),
            },
            A2aPart::Data(data) => {
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
            A2aPart::File(file) => ContentPart::File {
                uri: file.uri.clone(),
                mime_type: Some(file.media_type.clone()),
                name: file.name.clone(),
            },
        })
        .collect();

    Artifact {
        id: artifact.id.clone(),
        name: artifact
            .label
            .clone()
            .unwrap_or_else(|| artifact.id.clone()),
        mime_type: artifact.media_type.clone(),
        content,
        metadata: artifact.metadata.clone(),
    }
}

/// Convert A2A status to unified status.
fn a2a_to_unified_status(status: &A2aTaskStatus) -> TaskStatus {
    match status {
        A2aTaskStatus::Working => TaskStatus::Working,
        A2aTaskStatus::InputRequired => TaskStatus::InputRequired,
        A2aTaskStatus::Completed => TaskStatus::Completed,
        A2aTaskStatus::Failed => TaskStatus::Failed,
        A2aTaskStatus::Cancelled => TaskStatus::Cancelled,
        A2aTaskStatus::Rejected => TaskStatus::Failed, // Map rejected to failed
    }
}

/// Convert A2A streaming event to unified stream event.
fn a2a_to_unified_stream_event(event: &A2aStreamingEvent) -> StreamEvent {
    match event {
        A2aStreamingEvent::TaskStatusUpdate(update) => StreamEvent::StatusUpdate {
            task_id: update.task_id.clone(),
            status: a2a_to_unified_status(&update.status),
            message: update.message.as_ref().and_then(|m| {
                // Extract text from first text part if available
                m.parts
                    .iter()
                    .find_map(|p| p.as_text().map(|s| s.to_string()))
            }),
        },
        A2aStreamingEvent::TaskArtifactUpdate(update) => StreamEvent::ArtifactAdded {
            task_id: update.task_id.clone(),
            artifact: a2a_to_unified_artifact(&update.artifact),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_to_a2a_message() {
        let msg = UnifiedMessage::user("Hello, agent!");
        let a2a = unified_to_a2a_message(&msg);

        assert!(matches!(a2a.role, skreaver_a2a::Role::User));
        assert_eq!(a2a.parts.len(), 1);
    }

    #[test]
    fn test_a2a_to_unified_status() {
        assert_eq!(
            a2a_to_unified_status(&A2aTaskStatus::Completed),
            TaskStatus::Completed
        );
        assert_eq!(
            a2a_to_unified_status(&A2aTaskStatus::Working),
            TaskStatus::Working
        );
    }

    #[test]
    fn test_a2a_card_to_agent_info() {
        let card = AgentCard::new("test-agent", "Test Agent", "https://test.example.com")
            .with_description("A test agent")
            .with_streaming();

        let info = a2a_card_to_agent_info(&card);

        assert_eq!(info.id, "test-agent");
        assert_eq!(info.name, "Test Agent");
        assert!(info.supports_streaming);
        assert!(info.protocols.contains(&Protocol::A2a));
    }
}
