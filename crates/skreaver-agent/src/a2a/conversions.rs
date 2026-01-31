//! A2A protocol conversion functions.
//!
//! This module provides bidirectional conversion between unified agent types
//! and A2A protocol types.

use crate::types::{
    AgentInfo, Artifact, Capability, ContentPart, MessageRole, Protocol, StreamEvent, TaskStatus,
    UnifiedMessage, UnifiedTask,
};

use skreaver_a2a::{
    AgentCard, AgentInterface, DataPart as A2aDataPart, FilePart as A2aFilePart,
    Message as A2aMessage, Part as A2aPart, StreamingEvent as A2aStreamingEvent, Task as A2aTask,
    TaskStatus as A2aTaskStatus, TextPart as A2aTextPart,
};

// ============================================================================
// Unified -> A2A Conversions
// ============================================================================

/// Convert unified AgentInfo to A2A AgentCard.
pub fn unified_info_to_a2a_card(info: &AgentInfo) -> AgentCard {
    let mut card = AgentCard::new(
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

/// Convert unified message to A2A message.
pub fn unified_to_a2a_message(message: &UnifiedMessage) -> A2aMessage {
    let role = unified_to_a2a_role(message.role);
    let parts = unified_content_to_a2a_parts(&message.content);

    A2aMessage {
        id: Some(message.id.clone()),
        role,
        parts,
        reference_task_ids: Vec::new(),
        timestamp: message.timestamp,
        metadata: message.metadata.clone(),
    }
}

/// Convert unified message role to A2A role.
///
/// Note: A2A only has User and Agent roles. System messages are mapped to Agent
/// since they represent agent-side instructions/context.
#[inline]
pub fn unified_to_a2a_role(role: MessageRole) -> skreaver_a2a::Role {
    match role {
        MessageRole::User => skreaver_a2a::Role::User,
        // Both Agent and System map to Agent - System is agent-side context
        MessageRole::Agent | MessageRole::System => skreaver_a2a::Role::Agent,
    }
}

/// Convert unified content parts to A2A parts.
pub fn unified_content_to_a2a_parts(content: &[ContentPart]) -> Vec<A2aPart> {
    content
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(A2aPart::Text(A2aTextPart {
                text: text.clone(),
                metadata: Default::default(),
            })),
            ContentPart::Data {
                data, mime_type, ..
            } => {
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
            // Tool calls/results don't map directly to A2A parts
            ContentPart::ToolCall { .. } | ContentPart::ToolResult { .. } => None,
        })
        .collect()
}

/// Convert unified artifact to A2A artifact.
pub fn unified_to_a2a_artifact(artifact: &Artifact) -> skreaver_a2a::Artifact {
    let parts = unified_content_to_a2a_parts(&artifact.content);

    let mut a2a_artifact = skreaver_a2a::Artifact::new(&artifact.id).with_label(&artifact.name);
    for part in parts {
        a2a_artifact = a2a_artifact.with_part(part);
    }
    a2a_artifact
}

/// Convert unified task status to A2A task status.
pub fn unified_to_a2a_status(status: TaskStatus) -> A2aTaskStatus {
    match status {
        // A2A doesn't have a Pending status, map to Working
        TaskStatus::Pending => A2aTaskStatus::Working,
        TaskStatus::Working => A2aTaskStatus::Working,
        TaskStatus::InputRequired => A2aTaskStatus::InputRequired,
        TaskStatus::Completed => A2aTaskStatus::Completed,
        TaskStatus::Failed => A2aTaskStatus::Failed,
        TaskStatus::Cancelled => A2aTaskStatus::Cancelled,
        TaskStatus::Rejected => A2aTaskStatus::Rejected,
    }
}

/// Convert unified stream event to A2A streaming event.
///
/// Returns `None` for events that don't have an A2A equivalent.
pub fn unified_to_a2a_stream_event(event: &StreamEvent) -> Option<A2aStreamingEvent> {
    match event {
        StreamEvent::StatusUpdate {
            task_id,
            status,
            message,
        } => {
            let a2a_message = message.as_ref().map(skreaver_a2a::Message::agent);
            Some(A2aStreamingEvent::TaskStatusUpdate(
                skreaver_a2a::TaskStatusUpdateEvent {
                    task_id: task_id.clone(),
                    status: unified_to_a2a_status(*status),
                    message: a2a_message,
                    timestamp: chrono::Utc::now(),
                },
            ))
        }
        StreamEvent::ArtifactAdded { task_id, artifact } => Some(
            A2aStreamingEvent::TaskArtifactUpdate(skreaver_a2a::TaskArtifactUpdateEvent {
                task_id: task_id.clone(),
                artifact: unified_to_a2a_artifact(artifact),
                is_final: true,
                timestamp: chrono::Utc::now(),
            }),
        ),
        // Other events don't map to A2A streaming events
        _ => None,
    }
}

/// Update an A2A task from a unified task.
pub fn update_a2a_task_from_unified(task: &mut skreaver_a2a::Task, unified: &UnifiedTask) {
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

// ============================================================================
// A2A -> Unified Conversions
// ============================================================================

/// Convert A2A AgentCard to unified AgentInfo.
pub fn a2a_card_to_agent_info(card: &AgentCard) -> AgentInfo {
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

/// Extract base URL from AgentCard interfaces.
fn get_base_url_from_card(card: &AgentCard) -> Option<String> {
    for interface in &card.interfaces {
        if let AgentInterface::Http { base_url } = interface {
            return Some(base_url.clone());
        }
    }
    None
}

/// Convert A2A task to unified task.
pub fn a2a_to_unified_task(task: &A2aTask) -> UnifiedTask {
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
pub fn a2a_to_unified_message(message: &A2aMessage) -> UnifiedMessage {
    let role = a2a_to_unified_role(message.role);
    let content: Vec<ContentPart> = message.parts.iter().map(a2a_part_to_content_part).collect();

    let mut unified = UnifiedMessage::new(role, "");
    unified.id = message.id.clone().unwrap_or(unified.id);
    unified.content = content;
    unified.metadata = message.metadata.clone();
    unified.timestamp = message.timestamp;
    unified
}

/// Convert A2A role to unified message role.
#[inline]
pub fn a2a_to_unified_role(role: skreaver_a2a::Role) -> MessageRole {
    match role {
        skreaver_a2a::Role::User => MessageRole::User,
        skreaver_a2a::Role::Agent => MessageRole::Agent,
    }
}

/// Convert A2A Part to unified ContentPart.
pub fn a2a_part_to_content_part(part: &A2aPart) -> ContentPart {
    match part {
        A2aPart::Text(text) => ContentPart::Text {
            text: text.text.clone(),
        },
        A2aPart::Data(data) => {
            // Try to extract base64 if present, otherwise serialize data
            let data_str = if let Some(base64) = data.data.get("base64").and_then(|v| v.as_str()) {
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
    }
}

/// Convert A2A artifact to unified artifact.
pub fn a2a_to_unified_artifact(artifact: &skreaver_a2a::Artifact) -> Artifact {
    let content: Vec<ContentPart> = artifact
        .parts
        .iter()
        .map(a2a_part_to_content_part)
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
pub fn a2a_to_unified_status(status: &A2aTaskStatus) -> TaskStatus {
    match status {
        A2aTaskStatus::Working => TaskStatus::Working,
        A2aTaskStatus::InputRequired => TaskStatus::InputRequired,
        A2aTaskStatus::Completed => TaskStatus::Completed,
        A2aTaskStatus::Failed => TaskStatus::Failed,
        A2aTaskStatus::Cancelled => TaskStatus::Cancelled,
        A2aTaskStatus::Rejected => TaskStatus::Rejected,
    }
}

/// Convert A2A streaming event to unified stream event.
pub fn a2a_to_unified_stream_event(event: &A2aStreamingEvent) -> StreamEvent {
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
    fn test_unified_to_a2a_status() {
        assert_eq!(
            unified_to_a2a_status(TaskStatus::Completed),
            A2aTaskStatus::Completed
        );
        // Pending maps to Working in A2A
        assert_eq!(
            unified_to_a2a_status(TaskStatus::Pending),
            A2aTaskStatus::Working
        );
    }

    #[test]
    fn test_system_role_maps_to_agent() {
        // System role should map to Agent in A2A (agent-side context)
        assert_eq!(
            unified_to_a2a_role(MessageRole::System),
            skreaver_a2a::Role::Agent
        );
        assert_eq!(
            unified_to_a2a_role(MessageRole::Agent),
            skreaver_a2a::Role::Agent
        );
        assert_eq!(
            unified_to_a2a_role(MessageRole::User),
            skreaver_a2a::Role::User
        );
    }

    #[test]
    fn test_roundtrip_message() {
        let original = UnifiedMessage::user("Test message");
        let a2a = unified_to_a2a_message(&original);
        let converted = a2a_to_unified_message(&a2a);

        assert_eq!(converted.role, original.role);
        assert_eq!(converted.content.len(), original.content.len());
    }
}
