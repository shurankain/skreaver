//! A2A Protocol Server
//!
//! This module provides an HTTP server that exposes Skreaver agents via the A2A protocol.
//! It implements the standard A2A endpoints for agent discovery, task management, and streaming.
//!
//! # Example
//!
//! ```rust,ignore
//! use skreaver_a2a::server::{A2aServer, AgentHandler};
//! use skreaver_a2a::{AgentCard, Task, Message};
//!
//! struct MyAgent;
//!
//! #[async_trait::async_trait]
//! impl AgentHandler for MyAgent {
//!     fn agent_card(&self) -> AgentCard {
//!         AgentCard::new("my-agent", "My Agent", "https://my-agent.example.com")
//!     }
//!
//!     async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String> {
//!         // Process the message and update the task
//!         task.add_message(Message::agent("Hello! I received your message."));
//!         Ok(())
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let server = A2aServer::new(MyAgent);
//!     server.serve("0.0.0.0:3000").await.unwrap();
//! }
//! ```

use crate::error::{A2aError, A2aResult, ErrorResponse};
use crate::types::{
    AgentCard, Artifact, CancelTaskRequest, Message, SendMessageRequest, SendMessageResponse,
    StreamingEvent, Task, TaskArtifactUpdateEvent, TaskStatus, TaskStatusUpdateEvent,
};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use chrono::Utc;
use futures::Stream;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

// ============================================================================
// Constants
// ============================================================================

/// Size of broadcast channels for streaming events
const BROADCAST_CHANNEL_SIZE: usize = 64;

/// Trait for implementing A2A agent behavior
///
/// Implement this trait to define how your agent handles incoming messages
/// and produces responses.
#[async_trait]
pub trait AgentHandler: Send + Sync + 'static {
    /// Get the agent card describing this agent's capabilities
    fn agent_card(&self) -> AgentCard;

    /// Handle an incoming message
    ///
    /// This method is called when a new message is received. The implementation
    /// should process the message and update the task accordingly (add response
    /// messages, artifacts, update status, etc.)
    ///
    /// # Parameters
    ///
    /// * `task` - The task to update
    /// * `message` - The incoming message to process
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error message on failure
    async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String>;

    /// Called when a task is cancelled
    ///
    /// Override this to perform cleanup when a task is cancelled.
    async fn on_cancel(&self, _task: &Task) -> Result<(), String> {
        Ok(())
    }

    /// Check if streaming is supported
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Handle a message with streaming updates
    ///
    /// Override this to provide real-time streaming updates during processing.
    /// The default implementation delegates to `handle_message`.
    async fn handle_message_streaming(
        &self,
        task: &mut Task,
        message: Message,
        _event_tx: broadcast::Sender<StreamingEvent>,
    ) -> Result<(), String> {
        self.handle_message(task, message).await
    }
}

/// Configuration for task store
#[derive(Debug, Clone)]
pub struct TaskStoreConfig {
    /// Default TTL for tasks in seconds (default: 3600 = 1 hour)
    pub default_ttl_secs: u64,
    /// How often to run cleanup in seconds (default: 300 = 5 minutes)
    pub cleanup_interval_secs: u64,
}

impl Default for TaskStoreConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: 3600,     // 1 hour
            cleanup_interval_secs: 300, // 5 minutes
        }
    }
}

/// Task with expiration tracking
#[derive(Debug, Clone)]
struct StoredTask {
    task: Task,
    expires_at: chrono::DateTime<Utc>,
}

/// In-memory task store with expiration support
#[derive(Debug)]
struct TaskStore {
    tasks: RwLock<HashMap<String, StoredTask>>,
    subscribers: RwLock<HashMap<String, broadcast::Sender<StreamingEvent>>>,
    config: TaskStoreConfig,
}

impl TaskStore {
    fn new() -> Self {
        Self::with_config(TaskStoreConfig::default())
    }

    fn with_config(config: TaskStoreConfig) -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
            subscribers: RwLock::new(HashMap::new()),
            config,
        }
    }

    async fn get(&self, task_id: &str) -> Option<Task> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).and_then(|stored| {
            // Return None if expired
            if stored.expires_at < Utc::now() {
                None
            } else {
                Some(stored.task.clone())
            }
        })
    }

    async fn update(&self, task: Task) {
        let expires_at =
            Utc::now() + chrono::Duration::seconds(self.config.default_ttl_secs as i64);
        let stored = StoredTask {
            task: task.clone(),
            expires_at,
        };
        self.tasks.write().await.insert(task.id.clone(), stored);
    }

    async fn subscribe(&self, task_id: &str) -> broadcast::Receiver<StreamingEvent> {
        let mut subscribers = self.subscribers.write().await;
        if let Some(tx) = subscribers.get(task_id) {
            tx.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(BROADCAST_CHANNEL_SIZE);
            subscribers.insert(task_id.to_string(), tx);
            rx
        }
    }

    async fn get_sender(&self, task_id: &str) -> Option<broadcast::Sender<StreamingEvent>> {
        self.subscribers.read().await.get(task_id).cloned()
    }

    async fn create_sender(&self, task_id: &str) -> broadcast::Sender<StreamingEvent> {
        let mut subscribers = self.subscribers.write().await;
        let (tx, _) = broadcast::channel(BROADCAST_CHANNEL_SIZE);
        subscribers.insert(task_id.to_string(), tx.clone());
        tx
    }

    /// Clean up expired tasks and their subscribers
    async fn cleanup_expired(&self) -> usize {
        let now = Utc::now();
        let mut tasks = self.tasks.write().await;
        let mut subscribers = self.subscribers.write().await;

        // Find expired task IDs
        let expired: Vec<String> = tasks
            .iter()
            .filter(|(_, stored)| stored.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();

        // Remove expired tasks and their subscribers
        for id in &expired {
            tasks.remove(id);
            subscribers.remove(id);
            debug!(task_id = %id, "Cleaned up expired task");
        }

        if count > 0 {
            info!(count, "Cleaned up expired A2A tasks");
        }

        count
    }

    /// Get total task count
    async fn task_count(&self) -> usize {
        self.tasks.read().await.len()
    }
}

/// Shared application state
struct AppState<H: AgentHandler> {
    handler: Arc<H>,
    store: Arc<TaskStore>,
}

impl<H: AgentHandler> Clone for AppState<H> {
    fn clone(&self) -> Self {
        Self {
            handler: Arc::clone(&self.handler),
            store: Arc::clone(&self.store),
        }
    }
}

/// A2A Protocol Server
///
/// Exposes an agent via HTTP endpoints following the A2A protocol specification.
pub struct A2aServer<H: AgentHandler> {
    handler: Arc<H>,
    store: Arc<TaskStore>,
}

impl<H: AgentHandler> A2aServer<H> {
    /// Create a new A2A server with the given handler
    pub fn new(handler: H) -> Self {
        Self {
            handler: Arc::new(handler),
            store: Arc::new(TaskStore::new()),
        }
    }

    /// Create a new A2A server with custom task store configuration
    pub fn with_config(handler: H, config: TaskStoreConfig) -> Self {
        Self {
            handler: Arc::new(handler),
            store: Arc::new(TaskStore::with_config(config)),
        }
    }

    /// Start a background task that periodically cleans up expired tasks
    ///
    /// Returns a handle that can be used to abort the cleanup task.
    pub fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let store = Arc::clone(&self.store);
        let interval_secs = store.config.cleanup_interval_secs;

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;
                store.cleanup_expired().await;
            }
        })
    }

    /// Manually trigger cleanup of expired tasks
    pub async fn cleanup_expired_tasks(&self) -> usize {
        self.store.cleanup_expired().await
    }

    /// Get the current task count
    pub async fn task_count(&self) -> usize {
        self.store.task_count().await
    }

    /// Build the Axum router for this server
    pub fn router(&self) -> Router {
        let state = AppState {
            handler: Arc::clone(&self.handler),
            store: Arc::clone(&self.store),
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            // Agent card discovery
            .route("/.well-known/agent.json", get(get_agent_card::<H>))
            // Task management
            .route("/tasks/send", post(send_message::<H>))
            .route("/tasks/sendSubscribe", post(send_message_subscribe::<H>))
            .route("/tasks/{task_id}", get(get_task::<H>))
            .route("/tasks/{task_id}/cancel", post(cancel_task::<H>))
            .route("/tasks/{task_id}/subscribe", get(subscribe_task::<H>))
            .with_state(state)
            .layer(cors)
    }

    /// Serve the A2A server on the given address
    pub async fn serve(self, addr: &str) -> A2aResult<()> {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| A2aError::internal_error(format!("Failed to bind to {}: {}", addr, e)))?;

        let agent_card = self.handler.agent_card();
        info!(
            agent_id = %agent_card.agent_id,
            name = %agent_card.name,
            address = %addr,
            "A2A server starting"
        );

        let router = self.router();

        axum::serve(listener, router)
            .await
            .map_err(|e| A2aError::internal_error(format!("Server error: {}", e)))?;

        Ok(())
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /.well-known/agent.json - Agent card discovery
async fn get_agent_card<H: AgentHandler>(State(state): State<AppState<H>>) -> Json<AgentCard> {
    let card = state.handler.agent_card();
    debug!(agent_id = %card.agent_id, "Serving agent card");
    Json(card)
}

/// POST /tasks/send - Send a message
async fn send_message<H: AgentHandler>(
    State(state): State<AppState<H>>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, A2aErrorResponse> {
    debug!("Received send message request");

    // Get or create task
    let mut task = if let Some(task_id) = &request.task_id {
        state
            .store
            .get(task_id)
            .await
            .ok_or_else(|| A2aError::task_not_found(task_id))?
    } else {
        let mut task = Task::new_with_uuid();
        task.context_id = request.context_id.clone();
        task
    };

    // Check task isn't terminal
    if task.is_terminal() {
        return Err(A2aError::task_terminated(&task.id, task.status.to_string()).into());
    }

    // Add the message to the task
    task.add_message(request.message.clone());

    // Process the message
    match state
        .handler
        .handle_message(&mut task, request.message)
        .await
    {
        Ok(()) => {
            // Task completed successfully or is still working
            if !task.is_terminal() && task.status != TaskStatus::InputRequired {
                task.set_status(TaskStatus::Completed);
            }
        }
        Err(e) => {
            error!(error = %e, "Handler error");
            task.set_status(TaskStatus::Failed);
            task.add_message(Message::agent(format!("Error: {}", e)));
        }
    }

    // Store the task
    state.store.update(task.clone()).await;

    debug!(task_id = %task.id, status = %task.status, "Message processed");

    Ok(Json(SendMessageResponse { task }))
}

/// POST /tasks/sendSubscribe - Send a message with streaming response
async fn send_message_subscribe<H: AgentHandler>(
    State(state): State<AppState<H>>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, A2aErrorResponse> {
    debug!("Received streaming send message request");

    // Get or create task
    let mut task = if let Some(task_id) = &request.task_id {
        state
            .store
            .get(task_id)
            .await
            .ok_or_else(|| A2aError::task_not_found(task_id))?
    } else {
        let mut task = Task::new_with_uuid();
        task.context_id = request.context_id.clone();
        task
    };

    // Check task isn't terminal
    if task.is_terminal() {
        return Err(A2aError::task_terminated(&task.id, task.status.to_string()).into());
    }

    let task_id = task.id.clone();

    // Add the message to the task
    task.add_message(request.message.clone());
    state.store.update(task.clone()).await;

    // Create event sender
    let event_tx = state.store.create_sender(&task_id).await;
    let event_rx = event_tx.subscribe();

    // Spawn task to process the message
    let handler = Arc::clone(&state.handler);
    let store = Arc::clone(&state.store);
    let message = request.message;

    tokio::spawn(async move {
        let mut task = store.get(&task_id).await.unwrap();

        // Send initial status
        let _ = event_tx.send(StreamingEvent::TaskStatusUpdate(TaskStatusUpdateEvent {
            task_id: task.id.clone(),
            status: TaskStatus::Working,
            message: None,
            timestamp: Utc::now(),
        }));

        // Process the message
        match handler
            .handle_message_streaming(&mut task, message, event_tx.clone())
            .await
        {
            Ok(()) => {
                if !task.is_terminal() && task.status != TaskStatus::InputRequired {
                    task.set_status(TaskStatus::Completed);
                }
            }
            Err(e) => {
                error!(error = %e, "Handler error");
                task.set_status(TaskStatus::Failed);
                task.add_message(Message::agent(format!("Error: {}", e)));
            }
        }

        // Send final status
        let _ = event_tx.send(StreamingEvent::TaskStatusUpdate(TaskStatusUpdateEvent {
            task_id: task.id.clone(),
            status: task.status,
            message: task.messages.last().cloned(),
            timestamp: Utc::now(),
        }));

        // Store final state
        store.update(task).await;
    });

    // Create SSE stream
    let stream = create_sse_stream(event_rx, None);

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// GET /tasks/{task_id} - Get task status
async fn get_task<H: AgentHandler>(
    State(state): State<AppState<H>>,
    Path(task_id): Path<String>,
) -> Result<Json<Task>, A2aErrorResponse> {
    debug!(task_id = %task_id, "Getting task");

    let task = state
        .store
        .get(&task_id)
        .await
        .ok_or_else(|| A2aError::task_not_found(&task_id))?;

    Ok(Json(task))
}

/// POST /tasks/{task_id}/cancel - Cancel a task
async fn cancel_task<H: AgentHandler>(
    State(state): State<AppState<H>>,
    Path(task_id): Path<String>,
    Json(_request): Json<CancelTaskRequest>,
) -> Result<Json<Task>, A2aErrorResponse> {
    debug!(task_id = %task_id, "Cancelling task");

    let mut task = state
        .store
        .get(&task_id)
        .await
        .ok_or_else(|| A2aError::task_not_found(&task_id))?;

    if task.is_terminal() {
        return Err(A2aError::task_terminated(&task_id, task.status.to_string()).into());
    }

    // Call handler's cancel callback
    if let Err(e) = state.handler.on_cancel(&task).await {
        warn!(task_id = %task_id, error = %e, "Cancel callback failed");
    }

    task.set_status(TaskStatus::Cancelled);
    state.store.update(task.clone()).await;

    // Notify subscribers
    if let Some(tx) = state.store.get_sender(&task_id).await {
        let _ = tx.send(StreamingEvent::TaskStatusUpdate(TaskStatusUpdateEvent {
            task_id: task.id.clone(),
            status: TaskStatus::Cancelled,
            message: None,
            timestamp: Utc::now(),
        }));
    }

    info!(task_id = %task_id, "Task cancelled");

    Ok(Json(task))
}

/// GET /tasks/{task_id}/subscribe - Subscribe to task updates
async fn subscribe_task<H: AgentHandler>(
    State(state): State<AppState<H>>,
    Path(task_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, A2aErrorResponse> {
    debug!(task_id = %task_id, "Subscribing to task");

    // Check task exists
    let task = state
        .store
        .get(&task_id)
        .await
        .ok_or_else(|| A2aError::task_not_found(&task_id))?;

    // Get initial event if task is already terminal
    let initial_event = if task.is_terminal() {
        Some(StreamingEvent::TaskStatusUpdate(TaskStatusUpdateEvent {
            task_id: task.id.clone(),
            status: task.status,
            message: task.messages.last().cloned(),
            timestamp: Utc::now(),
        }))
    } else {
        None
    };

    // Subscribe to updates and create SSE stream
    let rx = state.store.subscribe(&task_id).await;
    let stream = create_sse_stream(rx, initial_event);

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

// =============================================================================
// Error Response
// =============================================================================

/// Wrapper for A2A errors that implements IntoResponse
struct A2aErrorResponse(A2aError);

impl From<A2aError> for A2aErrorResponse {
    fn from(err: A2aError) -> Self {
        Self(err)
    }
}

impl IntoResponse for A2aErrorResponse {
    fn into_response(self) -> Response {
        let error_response: ErrorResponse = self.0.into();
        let status = match error_response.code {
            400 => StatusCode::BAD_REQUEST,
            401 => StatusCode::UNAUTHORIZED,
            403 => StatusCode::FORBIDDEN,
            404 => StatusCode::NOT_FOUND,
            409 => StatusCode::CONFLICT,
            429 => StatusCode::TOO_MANY_REQUESTS,
            500 => StatusCode::INTERNAL_SERVER_ERROR,
            502 => StatusCode::BAD_GATEWAY,
            504 => StatusCode::GATEWAY_TIMEOUT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(error_response)).into_response()
    }
}

// =============================================================================
// Helper Functions for Handlers
// =============================================================================

/// Helper to send a streaming artifact update
pub fn send_artifact_update(
    tx: &broadcast::Sender<StreamingEvent>,
    task_id: &str,
    artifact: Artifact,
    is_final: bool,
) {
    let _ = tx.send(StreamingEvent::TaskArtifactUpdate(
        TaskArtifactUpdateEvent {
            task_id: task_id.to_string(),
            artifact,
            is_final,
            timestamp: Utc::now(),
        },
    ));
}

/// Helper to send a streaming status update
pub fn send_status_update(
    tx: &broadcast::Sender<StreamingEvent>,
    task_id: &str,
    status: TaskStatus,
    message: Option<Message>,
) {
    let _ = tx.send(StreamingEvent::TaskStatusUpdate(TaskStatusUpdateEvent {
        task_id: task_id.to_string(),
        status,
        message,
        timestamp: Utc::now(),
    }));
}

/// Create an SSE stream from a broadcast receiver.
///
/// If `initial_event` is provided, it will be sent immediately before processing
/// the broadcast receiver. If the initial event is terminal, the stream ends after
/// sending it.
fn create_sse_stream(
    rx: broadcast::Receiver<StreamingEvent>,
    initial_event: Option<StreamingEvent>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        // Send initial event if provided
        if let Some(event) = initial_event {
            let is_terminal = is_terminal_event(&event);
            yield Ok(streaming_event_to_sse(&event));
            if is_terminal {
                return;
            }
        }

        // Process live updates from the broadcast receiver
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let is_terminal = is_terminal_event(&event);
                    yield Ok(streaming_event_to_sse(&event));
                    if is_terminal {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }
}

/// Convert a StreamingEvent to an SSE Event.
#[inline]
fn streaming_event_to_sse(event: &StreamingEvent) -> Event {
    let data = serde_json::to_string(event).unwrap_or_default();
    let event_type = match event {
        StreamingEvent::TaskStatusUpdate(_) => "taskStatusUpdate",
        StreamingEvent::TaskArtifactUpdate(_) => "taskArtifactUpdate",
    };
    Event::default().event(event_type).data(data)
}

/// Check if a StreamingEvent represents a terminal status.
#[inline]
fn is_terminal_event(event: &StreamingEvent) -> bool {
    matches!(
        event,
        StreamingEvent::TaskStatusUpdate(update) if update.status.is_terminal()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentSkill;

    struct TestHandler;

    #[async_trait]
    impl AgentHandler for TestHandler {
        fn agent_card(&self) -> AgentCard {
            AgentCard::new("test-agent", "Test Agent", "http://localhost:3000")
                .with_description("A test agent")
                .with_skill(AgentSkill::new("echo", "Echo"))
        }

        async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String> {
            // Echo the message back
            let text = message
                .parts
                .first()
                .and_then(|p| p.as_text())
                .unwrap_or("No text");

            task.add_message(Message::agent(format!("Echo: {}", text)));
            Ok(())
        }
    }

    #[test]
    fn test_handler_agent_card() {
        let handler = TestHandler;
        let card = handler.agent_card();

        assert_eq!(card.agent_id, "test-agent");
        assert_eq!(card.name, "Test Agent");
        assert_eq!(card.skills.len(), 1);
    }

    #[tokio::test]
    async fn test_handler_message() {
        let handler = TestHandler;
        let mut task = Task::new("test-task");
        let message = Message::user("Hello!");

        handler.handle_message(&mut task, message).await.unwrap();

        assert_eq!(task.messages.len(), 1);
        let response = &task.messages[0];
        assert!(
            response.parts[0]
                .as_text()
                .unwrap()
                .contains("Echo: Hello!")
        );
    }

    #[test]
    fn test_server_creation() {
        let handler = TestHandler;
        let server = A2aServer::new(handler);

        // Just verify it creates without panic
        let _router = server.router();
    }

    #[test]
    fn test_error_response() {
        let error = A2aError::task_not_found("test-123");
        let response: A2aErrorResponse = error.into();
        let axum_response = response.into_response();

        assert_eq!(axum_response.status(), StatusCode::NOT_FOUND);
    }
}
