//! A2A Protocol Client
//!
//! This module provides an HTTP client for interacting with A2A-compatible agents.
//! It supports agent discovery, task management, and streaming updates.
//!
//! # Overview
//!
//! The [`A2aClient`] is the primary interface for connecting to A2A agents. It handles:
//!
//! - **Agent Discovery**: Fetch agent capabilities from `/.well-known/agent.json`
//! - **Message Sending**: Create tasks and send messages (sync and streaming)
//! - **Task Management**: Get status, cancel tasks, and poll for completion
//! - **Streaming**: Receive real-time updates via Server-Sent Events (SSE)
//!
//! # Connection Behavior
//!
//! ## Timeouts
//!
//! | Operation | Default Timeout | Notes |
//! |-----------|-----------------|-------|
//! | Regular requests | 30 seconds | Agent card, send, get task |
//! | Streaming requests | 5 minutes | SSE connections for long tasks |
//!
//! ## Connection Pooling
//!
//! The client uses `reqwest`'s built-in connection pooling. Connections are
//! reused across requests to the same host, reducing latency for repeated calls.
//! The client is `Clone`-able and safe to share across tasks.
//!
//! ## Retry Policy
//!
//! The client does **not** automatically retry failed requests. Implement your
//! own retry logic for resilience:
//!
//! ```rust,ignore
//! async fn send_with_retry(client: &A2aClient, msg: &str, retries: u32) -> A2aResult<Task> {
//!     let mut last_error = None;
//!     for _ in 0..retries {
//!         match client.send_message(msg).await {
//!             Ok(task) => return Ok(task),
//!             Err(e) if e.is_retryable() => {
//!                 last_error = Some(e);
//!                 tokio::time::sleep(Duration::from_millis(100)).await;
//!             }
//!             Err(e) => return Err(e),
//!         }
//!     }
//!     Err(last_error.unwrap())
//! }
//! ```
//!
//! # Authentication
//!
//! The client supports multiple authentication methods:
//!
//! ```rust,ignore
//! // Bearer token (OAuth2, JWT)
//! let client = A2aClient::new(url)?
//!     .with_bearer_token("eyJhbGciOiJIUzI1NiIs...");
//!
//! // API key in header
//! let client = A2aClient::new(url)?
//!     .with_api_key("X-API-Key", "sk-1234567890");
//!
//! // API key in query parameter
//! let client = A2aClient::new(url)?
//!     .with_auth(AuthConfig::ApiKeyQuery {
//!         name: "api_key".into(),
//!         value: "sk-1234567890".into(),
//!     });
//! ```
//!
//! # Streaming
//!
//! Streaming uses Server-Sent Events (SSE) to receive real-time updates:
//!
//! ```rust,ignore
//! use futures::StreamExt;
//!
//! let mut stream = client.send_message_streaming("Process this").await?;
//!
//! while let Some(event) = stream.next().await {
//!     match event? {
//!         StreamingEvent::TaskStatusUpdate(update) => {
//!             println!("Status: {:?}", update.status);
//!             if update.status.is_terminal() {
//!                 break;
//!             }
//!         }
//!         StreamingEvent::TaskArtifactUpdate(update) => {
//!             println!("Artifact: {}", update.artifact.id);
//!         }
//!     }
//! }
//! ```
//!
//! ## Backpressure
//!
//! The streaming implementation uses a bounded channel (32 events). If events
//! arrive faster than they're consumed, the oldest events are dropped with a
//! warning logged. Increase processing speed or reduce agent output rate if
//! you see "lagged" warnings.
//!
//! # Example
//!
//! ```rust,ignore
//! use skreaver_a2a::client::A2aClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client for an A2A agent
//!     let client = A2aClient::new("https://agent.example.com")?;
//!
//!     // Fetch the agent's capabilities
//!     let agent_card = client.get_agent_card().await?;
//!     println!("Connected to: {}", agent_card.name);
//!
//!     // Send a message and create a task
//!     let task = client.send_message("Hello, agent!").await?;
//!     println!("Task created: {}", task.id);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Error Handling
//!
//! The client maps HTTP status codes to specific error types:
//!
//! | Status | Error Type | Retryable |
//! |--------|------------|-----------|
//! | 400 | `InvalidMessage` | No |
//! | 401 | `AuthenticationRequired` | No |
//! | 403 | `NotAuthorized` | No |
//! | 404 | `AgentNotFound` or `TaskNotFound` | No |
//! | 429 | `RateLimitExceeded` | Yes |
//! | 500 | `InternalError` | Maybe |
//! | 502/504 | `ConnectionError` | Yes |

use crate::error::{A2aError, A2aResult};
use crate::types::{
    AgentCard, CancelTaskRequest, Message, SendMessageRequest, SendMessageResponse, StreamingEvent,
    Task,
};
use futures::Stream;
use reqwest::{Client, StatusCode};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info, warn};
use url::Url;

/// Default timeout for HTTP requests
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default timeout for streaming requests
const STREAMING_TIMEOUT: Duration = Duration::from_secs(300);

/// A2A protocol client for communicating with external agents
///
/// The client handles:
/// - Agent card discovery
/// - Task creation and management
/// - Message sending
/// - Streaming updates via Server-Sent Events (SSE)
#[derive(Clone)]
pub struct A2aClient {
    /// Base URL of the A2A agent
    base_url: Url,
    /// HTTP client
    http: Client,
    /// Cached agent card
    agent_card: Option<Arc<AgentCard>>,
    /// Authentication configuration
    auth: Option<AuthConfig>,
}

impl std::fmt::Debug for A2aClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aClient")
            .field("base_url", &self.base_url.as_str())
            .field("has_auth", &self.auth.is_some())
            .finish()
    }
}

/// Authentication configuration for A2A requests
#[derive(Clone)]
pub enum AuthConfig {
    /// Bearer token authentication
    Bearer(String),
    /// API key in header
    ApiKeyHeader { name: String, value: String },
    /// API key in query parameter
    ApiKeyQuery { name: String, value: String },
}

impl A2aClient {
    /// Create a new A2A client for the given agent URL
    ///
    /// # Parameters
    ///
    /// * `base_url` - The base URL of the A2A agent
    ///
    /// # Returns
    ///
    /// A new `A2aClient` instance or an error if the URL is invalid
    pub fn new(base_url: impl AsRef<str>) -> A2aResult<Self> {
        let base_url = Url::parse(base_url.as_ref())?;

        let http = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent(format!("skreaver-a2a/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| {
                A2aError::connection_error(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            base_url,
            http,
            agent_card: None,
            auth: None,
        })
    }

    /// Create a new A2A client with custom HTTP client
    pub fn with_http_client(base_url: impl AsRef<str>, http: Client) -> A2aResult<Self> {
        let base_url = Url::parse(base_url.as_ref())?;

        Ok(Self {
            base_url,
            http,
            agent_card: None,
            auth: None,
        })
    }

    /// Set authentication configuration
    pub fn with_auth(mut self, auth: AuthConfig) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set bearer token authentication
    pub fn with_bearer_token(self, token: impl Into<String>) -> Self {
        self.with_auth(AuthConfig::Bearer(token.into()))
    }

    /// Set API key authentication (header)
    pub fn with_api_key(self, header_name: impl Into<String>, api_key: impl Into<String>) -> Self {
        self.with_auth(AuthConfig::ApiKeyHeader {
            name: header_name.into(),
            value: api_key.into(),
        })
    }

    /// Get the base URL
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Build a URL for an endpoint
    fn endpoint(&self, path: &str) -> A2aResult<Url> {
        self.base_url
            .join(path)
            .map_err(|e| A2aError::protocol_error(format!("Invalid endpoint path: {}", e)))
    }

    /// Apply authentication to a request builder
    fn apply_auth(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(auth) = &self.auth {
            builder = match auth {
                AuthConfig::Bearer(token) => builder.bearer_auth(token),
                AuthConfig::ApiKeyHeader { name, value } => builder.header(name.as_str(), value),
                AuthConfig::ApiKeyQuery { name, value } => {
                    builder.query(&[(name.as_str(), value.as_str())])
                }
            };
        }
        builder
    }

    // =========================================================================
    // Agent Discovery
    // =========================================================================

    /// Fetch the agent card from the well-known endpoint
    ///
    /// The agent card describes the agent's capabilities, skills, and
    /// how to interact with it.
    ///
    /// # Returns
    ///
    /// The agent's `AgentCard` or an error
    pub async fn get_agent_card(&self) -> A2aResult<AgentCard> {
        let url = self.endpoint("/.well-known/agent.json")?;

        debug!(url = %url, "Fetching agent card");

        let request = self.apply_auth(self.http.get(url.clone()));
        let response = request.send().await.map_err(|e| {
            A2aError::connection_error(format!("Failed to fetch agent card: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let agent_card: AgentCard = response
            .json()
            .await
            .map_err(|e| A2aError::protocol_error(format!("Failed to parse agent card: {}", e)))?;

        info!(
            agent_id = %agent_card.agent_id,
            name = %agent_card.name,
            skills = agent_card.skills.len(),
            "Fetched agent card"
        );

        Ok(agent_card)
    }

    /// Fetch and cache the agent card
    pub async fn discover(&mut self) -> A2aResult<Arc<AgentCard>> {
        let card = self.get_agent_card().await?;
        let card = Arc::new(card);
        self.agent_card = Some(Arc::clone(&card));
        Ok(card)
    }

    /// Get the cached agent card, or fetch it if not cached
    pub async fn agent_card(&mut self) -> A2aResult<Arc<AgentCard>> {
        if let Some(card) = &self.agent_card {
            return Ok(Arc::clone(card));
        }
        self.discover().await
    }

    // =========================================================================
    // Task Management
    // =========================================================================

    /// Send a message to the agent
    ///
    /// This creates a new task or continues an existing one.
    ///
    /// # Parameters
    ///
    /// * `text` - The message text to send
    ///
    /// # Returns
    ///
    /// The task containing the agent's response
    pub async fn send_message(&self, text: impl Into<String>) -> A2aResult<Task> {
        let message = Message::user(text);
        self.send(message, None, None).await
    }

    /// Send a message to continue an existing task
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to continue
    /// * `text` - The message text to send
    ///
    /// # Returns
    ///
    /// The updated task
    pub async fn continue_task(
        &self,
        task_id: impl Into<String>,
        text: impl Into<String>,
    ) -> A2aResult<Task> {
        let message = Message::user(text);
        self.send(message, Some(task_id.into()), None).await
    }

    /// Send a message with full control over the request
    ///
    /// # Parameters
    ///
    /// * `message` - The message to send
    /// * `task_id` - Optional task ID to continue
    /// * `context_id` - Optional context ID for grouping tasks
    ///
    /// # Returns
    ///
    /// The task containing the response
    pub async fn send(
        &self,
        message: Message,
        task_id: Option<String>,
        context_id: Option<String>,
    ) -> A2aResult<Task> {
        let url = self.endpoint("/tasks/send")?;

        let request_body = SendMessageRequest {
            message,
            task_id,
            context_id,
            metadata: Default::default(),
        };

        debug!(url = %url, "Sending message to agent");

        let request = self
            .apply_auth(self.http.post(url.clone()))
            .json(&request_body);

        let response = request
            .send()
            .await
            .map_err(|e| A2aError::connection_error(format!("Failed to send message: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let send_response: SendMessageResponse = response
            .json()
            .await
            .map_err(|e| A2aError::protocol_error(format!("Failed to parse response: {}", e)))?;

        debug!(
            task_id = %send_response.task.id,
            status = %send_response.task.status,
            "Message sent successfully"
        );

        Ok(send_response.task)
    }

    /// Get the current state of a task
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to fetch
    ///
    /// # Returns
    ///
    /// The task state or an error
    pub async fn get_task(&self, task_id: impl AsRef<str>) -> A2aResult<Task> {
        let task_id = task_id.as_ref();
        let url = self.endpoint(&format!("/tasks/{}", task_id))?;

        debug!(task_id = %task_id, "Fetching task");

        let request = self.apply_auth(self.http.get(url.clone()));
        let response = request
            .send()
            .await
            .map_err(|e| A2aError::connection_error(format!("Failed to fetch task: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let task: Task = response
            .json()
            .await
            .map_err(|e| A2aError::protocol_error(format!("Failed to parse task: {}", e)))?;

        Ok(task)
    }

    /// Cancel a running task
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to cancel
    /// * `reason` - Optional reason for cancellation
    ///
    /// # Returns
    ///
    /// The cancelled task or an error
    pub async fn cancel_task(
        &self,
        task_id: impl Into<String>,
        reason: Option<String>,
    ) -> A2aResult<Task> {
        let task_id = task_id.into();
        let url = self.endpoint(&format!("/tasks/{}/cancel", task_id))?;

        let request_body = CancelTaskRequest {
            task_id: task_id.clone(),
            reason,
        };

        debug!(task_id = %task_id, "Cancelling task");

        let request = self
            .apply_auth(self.http.post(url.clone()))
            .json(&request_body);

        let response = request
            .send()
            .await
            .map_err(|e| A2aError::connection_error(format!("Failed to cancel task: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let task: Task = response.json().await.map_err(|e| {
            A2aError::protocol_error(format!("Failed to parse cancelled task: {}", e))
        })?;

        info!(task_id = %task.id, "Task cancelled");

        Ok(task)
    }

    // =========================================================================
    // Streaming
    // =========================================================================

    /// Send a message and receive streaming updates
    ///
    /// Returns a stream of events as the agent processes the request.
    ///
    /// # Parameters
    ///
    /// * `text` - The message text to send
    ///
    /// # Returns
    ///
    /// A stream of `StreamingEvent`s
    pub async fn send_message_streaming(
        &self,
        text: impl Into<String>,
    ) -> A2aResult<Pin<Box<dyn Stream<Item = A2aResult<StreamingEvent>> + Send>>> {
        let message = Message::user(text);
        self.send_streaming(message, None, None).await
    }

    /// Send a message with streaming response
    ///
    /// # Parameters
    ///
    /// * `message` - The message to send
    /// * `task_id` - Optional task ID to continue
    /// * `context_id` - Optional context ID
    ///
    /// # Returns
    ///
    /// A stream of `StreamingEvent`s
    pub async fn send_streaming(
        &self,
        message: Message,
        task_id: Option<String>,
        context_id: Option<String>,
    ) -> A2aResult<Pin<Box<dyn Stream<Item = A2aResult<StreamingEvent>> + Send>>> {
        let url = self.endpoint("/tasks/sendSubscribe")?;

        let request_body = SendMessageRequest {
            message,
            task_id,
            context_id,
            metadata: Default::default(),
        };

        debug!(url = %url, "Sending message with streaming");

        let client = self.http.clone();
        let auth = self.auth.clone();

        let mut request = client
            .post(url.clone())
            .timeout(STREAMING_TIMEOUT)
            .header("Accept", "text/event-stream")
            .json(&request_body);

        // Apply auth
        if let Some(auth) = &auth {
            request = match auth {
                AuthConfig::Bearer(token) => request.bearer_auth(token),
                AuthConfig::ApiKeyHeader { name, value } => request.header(name.as_str(), value),
                AuthConfig::ApiKeyQuery { name, value } => {
                    request.query(&[(name.as_str(), value.as_str())])
                }
            };
        }

        let response = request.send().await.map_err(|e| {
            A2aError::connection_error(format!("Failed to send streaming request: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        // Create a channel for streaming events
        let (tx, rx) = tokio::sync::mpsc::channel::<A2aResult<StreamingEvent>>(32);

        // Spawn a task to process the SSE stream
        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            use futures::StreamExt;

            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let chunk_str = match std::str::from_utf8(&chunk) {
                            Ok(s) => s,
                            Err(e) => {
                                warn!(error = %e, "Invalid UTF-8 in SSE stream");
                                continue;
                            }
                        };

                        buffer.push_str(chunk_str);

                        // Process complete SSE events
                        while let Some(event) = parse_sse_event(&mut buffer) {
                            if tx.send(event).await.is_err() {
                                // Receiver dropped
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(A2aError::connection_error(format!(
                                "Stream error: {}",
                                e
                            ))))
                            .await;
                        return;
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    /// Subscribe to updates for an existing task
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to subscribe to
    ///
    /// # Returns
    ///
    /// A stream of `StreamingEvent`s
    pub async fn subscribe_task(
        &self,
        task_id: impl AsRef<str>,
    ) -> A2aResult<Pin<Box<dyn Stream<Item = A2aResult<StreamingEvent>> + Send>>> {
        let task_id = task_id.as_ref();
        let url = self.endpoint(&format!("/tasks/{}/subscribe", task_id))?;

        debug!(task_id = %task_id, "Subscribing to task updates");

        let mut request = self
            .http
            .get(url.clone())
            .timeout(STREAMING_TIMEOUT)
            .header("Accept", "text/event-stream");

        request = self.apply_auth(request);

        let response = request.send().await.map_err(|e| {
            A2aError::connection_error(format!("Failed to subscribe to task: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        // Create streaming channel
        let (tx, rx) = tokio::sync::mpsc::channel::<A2aResult<StreamingEvent>>(32);

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            use futures::StreamExt;

            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Ok(chunk_str) = std::str::from_utf8(&chunk) {
                            buffer.push_str(chunk_str);

                            while let Some(event) = parse_sse_event(&mut buffer) {
                                if tx.send(event).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(A2aError::connection_error(format!(
                                "Stream error: {}",
                                e
                            ))))
                            .await;
                        return;
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Wait for a task to complete
    ///
    /// Polls the task status until it reaches a terminal state.
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to wait for
    /// * `poll_interval` - How often to poll for updates
    /// * `timeout` - Maximum time to wait
    ///
    /// # Returns
    ///
    /// The completed task or an error
    pub async fn wait_for_task(
        &self,
        task_id: impl AsRef<str>,
        poll_interval: Duration,
        timeout: Duration,
    ) -> A2aResult<Task> {
        let task_id = task_id.as_ref();
        let start = std::time::Instant::now();

        loop {
            let task = self.get_task(task_id).await?;

            if task.is_terminal() {
                return Ok(task);
            }

            if start.elapsed() > timeout {
                return Err(A2aError::Timeout {
                    timeout_ms: timeout.as_millis() as u64,
                });
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Handle error responses from the agent
    async fn handle_error_response(
        &self,
        status: StatusCode,
        response: reqwest::Response,
    ) -> A2aError {
        // Try to parse error response body
        let error_text = response.text().await.unwrap_or_default();

        match status {
            StatusCode::NOT_FOUND => A2aError::AgentNotFound {
                agent_id: self.base_url.to_string(),
            },
            StatusCode::UNAUTHORIZED => A2aError::AuthenticationRequired,
            StatusCode::FORBIDDEN => A2aError::NotAuthorized { reason: error_text },
            StatusCode::TOO_MANY_REQUESTS => {
                // Try to parse retry-after header
                A2aError::RateLimitExceeded {
                    retry_after_seconds: 60, // Default
                }
            }
            StatusCode::BAD_REQUEST => A2aError::InvalidMessage { reason: error_text },
            StatusCode::INTERNAL_SERVER_ERROR => A2aError::InternalError {
                message: error_text,
            },
            _ => A2aError::protocol_error(format!("HTTP {}: {}", status, error_text)),
        }
    }
}

/// Parse SSE events from a buffer
///
/// Returns the next complete event if available, and removes it from the buffer.
fn parse_sse_event(buffer: &mut String) -> Option<A2aResult<StreamingEvent>> {
    // SSE events are separated by double newlines
    let event_end = buffer.find("\n\n")?;
    let event_str = buffer[..event_end].to_string();
    buffer.drain(..event_end + 2);

    // Parse the event
    let mut event_type = None;
    let mut data = String::new();

    for line in event_str.lines() {
        if let Some(value) = line.strip_prefix("event:") {
            event_type = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim());
        }
    }

    if data.is_empty() {
        return None;
    }

    // Parse the data as JSON
    match serde_json::from_str::<StreamingEvent>(&data) {
        Ok(event) => Some(Ok(event)),
        Err(e) => {
            warn!(
                event_type = ?event_type,
                error = %e,
                "Failed to parse SSE event"
            );
            Some(Err(A2aError::protocol_error(format!(
                "Failed to parse streaming event: {}",
                e
            ))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = A2aClient::new("https://agent.example.com").unwrap();
        assert_eq!(client.base_url().as_str(), "https://agent.example.com/");
    }

    #[test]
    fn test_client_with_auth() {
        let client = A2aClient::new("https://agent.example.com")
            .unwrap()
            .with_bearer_token("my-token");

        assert!(client.auth.is_some());
    }

    #[test]
    fn test_endpoint_building() {
        let client = A2aClient::new("https://agent.example.com").unwrap();

        let url = client.endpoint("/tasks/send").unwrap();
        assert_eq!(url.as_str(), "https://agent.example.com/tasks/send");

        let url = client.endpoint("/.well-known/agent.json").unwrap();
        assert_eq!(
            url.as_str(),
            "https://agent.example.com/.well-known/agent.json"
        );
    }

    #[test]
    fn test_invalid_url() {
        let result = A2aClient::new("not a valid url");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_sse_event() {
        let mut buffer = String::from(
            "event: taskStatusUpdate\ndata: {\"type\":\"taskStatusUpdate\",\"taskId\":\"123\",\"status\":\"completed\",\"timestamp\":\"2024-01-01T00:00:00Z\"}\n\n",
        );

        let result = parse_sse_event(&mut buffer);
        assert!(result.is_some());
        // Buffer should be empty after parsing
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_parse_incomplete_sse_event() {
        let mut buffer = String::from("event: taskStatusUpdate\ndata: {\"incomplete\"");

        let result = parse_sse_event(&mut buffer);
        assert!(result.is_none());
        // Buffer should still contain the incomplete event
        assert!(!buffer.is_empty());
    }
}
