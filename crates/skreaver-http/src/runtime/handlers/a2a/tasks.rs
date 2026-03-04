//! Task Management Endpoints
//!
//! This module implements task lifecycle management endpoints for the A2A protocol.
//! Tasks represent units of work that can be submitted, monitored, and cancelled.

use axum::{
    extract::{Path, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use skreaver_a2a::{
    DataPart, FilePart, Message, Part, SendMessageRequest, SendMessageResponse, Task, TaskStatus,
    TextPart,
};
use skreaver_tools::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::A2aState;
use super::errors::{A2aApiError, A2aApiResult};

/// In-memory task storage for A2A tasks
///
/// This provides a simple in-memory store for task state management.
/// In production, this would typically be backed by a persistent store.
pub struct A2aTaskStore {
    tasks: RwLock<HashMap<String, Task>>,
}

impl A2aTaskStore {
    /// Create a new task store
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new task
    pub async fn create(&self, task_id: &str) -> Result<Task, A2aApiError> {
        let mut tasks = self.tasks.write().await;
        if tasks.contains_key(task_id) {
            return Err(A2aApiError::task_already_exists(task_id));
        }

        let task = Task::new(task_id);
        tasks.insert(task_id.to_string(), task.clone());
        Ok(task)
    }

    /// Get a task by ID
    pub async fn get(&self, task_id: &str) -> Option<Task> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    /// Update a task
    pub async fn update(&self, task: Task) -> Result<(), A2aApiError> {
        let mut tasks = self.tasks.write().await;
        if !tasks.contains_key(&task.id) {
            return Err(A2aApiError::task_not_found(&task.id));
        }
        tasks.insert(task.id.clone(), task);
        Ok(())
    }

    /// Delete a task
    pub async fn delete(&self, task_id: &str) -> Result<Task, A2aApiError> {
        let mut tasks = self.tasks.write().await;
        tasks
            .remove(task_id)
            .ok_or_else(|| A2aApiError::task_not_found(task_id))
    }

    /// List all tasks
    pub async fn list(&self) -> Vec<Task> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    /// Get task count
    pub async fn count(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks.len()
    }
}

impl Default for A2aTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to create a new task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    /// Optional task ID (generated if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Initial message to send with the task
    pub message: TaskMessage,
    /// Optional session ID for multi-turn conversations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A message in a task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMessage {
    /// Message role (user, agent)
    pub role: String,
    /// Message content parts
    pub parts: Vec<MessagePart>,
}

/// A part of a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum MessagePart {
    /// Text content
    #[serde(rename = "text")]
    Text { text: String },
    /// File reference
    #[serde(rename = "file")]
    File {
        uri: String,
        #[serde(rename = "mediaType")]
        media_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Structured data
    #[serde(rename = "data")]
    Data {
        data: serde_json::Value,
        #[serde(rename = "mediaType")]
        media_type: String,
    },
}

/// Response for task creation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResponse {
    /// The created task
    pub task: Task,
}

/// Response for task retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskResponse {
    /// The task
    pub task: Task,
}

/// Response for task cancellation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskResponse {
    /// The cancelled task
    pub task: Task,
}

/// POST /a2a/tasks - Create a new task
///
/// Creates a new task with an initial message. The task will be processed
/// by the agent and status updates can be retrieved via polling or SSE.
pub async fn create_task<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(state): State<A2aState<T>>,
    Json(request): Json<CreateTaskRequest>,
) -> A2aApiResult<(axum::http::StatusCode, Json<CreateTaskResponse>)> {
    // Generate task ID if not provided
    let task_id = request
        .id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Create the task in the store
    let mut task = state.task_store.create(&task_id).await?;

    // Convert the request message to A2A Message format
    let parts: Vec<Part> = request
        .message
        .parts
        .into_iter()
        .map(|p| match p {
            MessagePart::Text { text } => Part::Text(TextPart {
                text,
                metadata: HashMap::new(),
            }),
            MessagePart::File {
                uri,
                media_type,
                name,
            } => Part::File(FilePart {
                uri,
                media_type,
                name,
                metadata: HashMap::new(),
            }),
            MessagePart::Data { data, media_type } => Part::Data(DataPart {
                data,
                media_type,
                metadata: HashMap::new(),
            }),
        })
        .collect();

    let role = match request.message.role.as_str() {
        "user" => skreaver_a2a::Role::User,
        "agent" => skreaver_a2a::Role::Agent,
        _ => skreaver_a2a::Role::User,
    };

    let message = Message::user("").with_part(Part::Text(TextPart {
        text: parts
            .iter()
            .filter_map(|p| p.as_text())
            .collect::<Vec<_>>()
            .join("\n"),
        metadata: HashMap::new(),
    }));

    // Override role if it was agent
    let message = if role == skreaver_a2a::Role::Agent {
        Message::agent(
            message
                .parts
                .iter()
                .filter_map(|p| p.as_text())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        message
    };

    // Add the message to the task
    task.add_message(message);

    // Set session ID if provided
    if let Some(session_id) = request.session_id {
        task.context_id = Some(session_id);
    }

    // Update the task in the store
    state.task_store.update(task.clone()).await?;

    // Broadcast task created event
    state.event_broadcaster.broadcast_task_created(&task).await;

    // Spawn background task processing
    let task_store = Arc::clone(&state.task_store);
    let event_broadcaster = Arc::clone(&state.event_broadcaster);
    let runtime = state.runtime.clone();
    let task_id_clone = task_id.clone();

    tokio::spawn(async move {
        process_task(task_id_clone, task_store, event_broadcaster, runtime).await;
    });

    Ok((
        axum::http::StatusCode::CREATED,
        Json(CreateTaskResponse { task }),
    ))
}

/// Background task processing
async fn process_task<T: ToolRegistry + Clone + Send + Sync + 'static>(
    task_id: String,
    task_store: Arc<A2aTaskStore>,
    event_broadcaster: Arc<super::A2aEventBroadcaster>,
    runtime: crate::runtime::HttpAgentRuntime<T>,
) {
    // Get the task
    let task = match task_store.get(&task_id).await {
        Some(t) => t,
        None => {
            tracing::error!(task_id = %task_id, "Task not found for processing");
            return;
        }
    };

    // Extract the input from the task's messages
    let input = task
        .messages
        .iter()
        .filter(|m| matches!(m.role, skreaver_a2a::Role::User))
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| p.as_text())
        .collect::<Vec<_>>()
        .join("\n");

    if input.is_empty() {
        tracing::warn!(task_id = %task_id, "No text input found in task");
        let mut task = task;
        task.set_status(TaskStatus::Failed);
        let _ = task_store.update(task.clone()).await;
        event_broadcaster.broadcast_task_status(&task).await;
        return;
    }

    // Update status to working
    let mut task = task;
    task.set_status(TaskStatus::Working);
    let _ = task_store.update(task.clone()).await;
    event_broadcaster.broadcast_task_status(&task).await;

    // Try to find an agent to process the task
    // Use the first available agent
    let agents = runtime.agents.read().await;

    if let Some((agent_id, _instance)) = agents.iter().next() {
        let agent_id = agent_id.clone();
        drop(agents); // Release the read lock

        // Process through the agent's coordinator (requires write lock for mutable access)
        let response = {
            let mut agents = runtime.agents.write().await;
            if let Some(instance) = agents.get_mut(&agent_id) {
                Some(instance.coordinator.step(input))
            } else {
                None
            }
        };

        if let Some(response) = response {
            // Update task with agent response
            let agent_message = Message::agent(&response);
            task.add_message(agent_message);
            task.set_status(TaskStatus::Completed);
        } else {
            task.set_status(TaskStatus::Failed);
        }
    } else {
        // No agents available - mark as failed
        tracing::warn!(task_id = %task_id, "No agents available to process task");
        task.set_status(TaskStatus::Failed);
    }

    // Update the task in the store
    let _ = task_store.update(task.clone()).await;
    event_broadcaster.broadcast_task_status(&task).await;
}

/// GET /a2a/tasks/:id - Get task status
///
/// Retrieves the current state of a task including its status, messages,
/// and any artifacts produced.
pub async fn get_task<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(state): State<A2aState<T>>,
    Path(task_id): Path<String>,
) -> A2aApiResult<Json<GetTaskResponse>> {
    let task = state
        .task_store
        .get(&task_id)
        .await
        .ok_or_else(|| A2aApiError::task_not_found(&task_id))?;

    Ok(Json(GetTaskResponse { task }))
}

/// DELETE /a2a/tasks/:id - Cancel a task
///
/// Cancels a running task. Tasks that are already in a terminal state
/// (completed, failed, cancelled) cannot be cancelled.
pub async fn cancel_task<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(state): State<A2aState<T>>,
    Path(task_id): Path<String>,
) -> A2aApiResult<Json<CancelTaskResponse>> {
    let mut task = state
        .task_store
        .get(&task_id)
        .await
        .ok_or_else(|| A2aApiError::task_not_found(&task_id))?;

    // Check if task can be cancelled
    if task.is_terminal() {
        return Err(A2aApiError::new(
            400,
            format!(
                "Task {} is already in terminal state: {:?}",
                task_id, task.status
            ),
        ));
    }

    // Cancel the task
    task.set_status(TaskStatus::Cancelled);
    state.task_store.update(task.clone()).await?;

    // Broadcast cancellation event
    state.event_broadcaster.broadcast_task_status(&task).await;

    Ok(Json(CancelTaskResponse { task }))
}

/// POST /a2a/tasks/:id/messages - Send a message to a task
///
/// Sends an additional message to an existing task, enabling multi-turn
/// conversations. This is used for follow-up queries or providing
/// additional context.
pub async fn send_task_message<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(state): State<A2aState<T>>,
    Path(task_id): Path<String>,
    Json(request): Json<SendMessageRequest>,
) -> A2aApiResult<Json<SendMessageResponse>> {
    let mut task = state
        .task_store
        .get(&task_id)
        .await
        .ok_or_else(|| A2aApiError::task_not_found(&task_id))?;

    // Check if task can receive messages
    if task.is_terminal() {
        return Err(A2aApiError::new(
            400,
            format!(
                "Cannot send message to task {} in terminal state: {:?}",
                task_id, task.status
            ),
        ));
    }

    // Add the message to the task
    task.add_message(request.message.clone());

    // Update status back to working if it was completed
    if task.status == TaskStatus::Completed {
        task.set_status(TaskStatus::Working);
    }

    state.task_store.update(task.clone()).await?;

    // Broadcast message event
    state
        .event_broadcaster
        .broadcast_task_message(&task, &request.message)
        .await;

    // Process the new message
    let task_store = Arc::clone(&state.task_store);
    let event_broadcaster = Arc::clone(&state.event_broadcaster);
    let runtime = state.runtime.clone();
    let task_id_clone = task_id.clone();

    tokio::spawn(async move {
        process_task(task_id_clone, task_store, event_broadcaster, runtime).await;
    });

    Ok(Json(SendMessageResponse { task }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_store_create() {
        let store = A2aTaskStore::new();

        let task = store.create("task-1").await.unwrap();
        assert_eq!(task.id, "task-1");
        assert_eq!(task.status, TaskStatus::Working);
    }

    #[tokio::test]
    async fn test_task_store_duplicate() {
        let store = A2aTaskStore::new();

        store.create("task-1").await.unwrap();
        let result = store.create("task-1").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_store_get() {
        let store = A2aTaskStore::new();

        store.create("task-1").await.unwrap();

        let task = store.get("task-1").await;
        assert!(task.is_some());
        assert_eq!(task.unwrap().id, "task-1");

        let missing = store.get("task-2").await;
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_task_store_update() {
        let store = A2aTaskStore::new();

        let mut task = store.create("task-1").await.unwrap();
        task.set_status(TaskStatus::Working);

        store.update(task).await.unwrap();

        let updated = store.get("task-1").await.unwrap();
        assert_eq!(updated.status, TaskStatus::Working);
    }

    #[tokio::test]
    async fn test_task_store_delete() {
        let store = A2aTaskStore::new();

        store.create("task-1").await.unwrap();

        let deleted = store.delete("task-1").await.unwrap();
        assert_eq!(deleted.id, "task-1");

        assert!(store.get("task-1").await.is_none());
    }

    #[tokio::test]
    async fn test_task_store_list() {
        let store = A2aTaskStore::new();

        store.create("task-1").await.unwrap();
        store.create("task-2").await.unwrap();
        store.create("task-3").await.unwrap();

        let tasks = store.list().await;
        assert_eq!(tasks.len(), 3);
    }

    #[test]
    fn test_message_part_serialization() {
        let part = MessagePart::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello\""));
    }

    #[test]
    fn test_create_task_request() {
        let request = CreateTaskRequest {
            id: Some("my-task".to_string()),
            message: TaskMessage {
                role: "user".to_string(),
                parts: vec![MessagePart::Text {
                    text: "Hello agent".to_string(),
                }],
            },
            session_id: None,
            metadata: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("my-task"));
        assert!(json.contains("Hello agent"));
    }
}
