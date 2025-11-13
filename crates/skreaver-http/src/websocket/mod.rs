//! WebSocket support for real-time communication
//!
//! This module provides WebSocket functionality for the Skreaver HTTP runtime,
//! enabling real-time bidirectional communication between clients and the server.

use axum::{
    extract::{
        ConnectInfo, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

pub mod handlers;
pub mod lock_ordering;
pub mod manager;
pub mod protocol;

pub use handlers::*;
pub use manager::*;
pub use protocol::*;

/// WebSocket configuration
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Connection timeout in seconds
    pub connection_timeout: Duration,
    /// Ping interval in seconds
    pub ping_interval: Duration,
    /// Pong timeout in seconds (time to wait for pong after ping)
    pub pong_timeout: Duration,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Enable message compression
    pub enable_compression: bool,
    /// Buffer size for incoming messages
    pub buffer_size: usize,
    /// Maximum subscriptions per connection
    pub max_subscriptions_per_connection: usize,
    /// Maximum subscribers per channel
    pub max_subscribers_per_channel: usize,
    /// Maximum connections per IP address
    pub max_connections_per_ip: usize,
    /// Broadcast channel buffer size
    pub broadcast_buffer_size: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            connection_timeout: Duration::from_secs(60),
            ping_interval: Duration::from_secs(30),
            pong_timeout: Duration::from_secs(10),
            max_message_size: 64 * 1024, // 64KB
            enable_compression: true,
            buffer_size: 100,
            max_subscriptions_per_connection: 50,
            max_subscribers_per_channel: 10000,
            max_connections_per_ip: 10,
            broadcast_buffer_size: 1000,
        }
    }
}

/// Validated builder for `WebSocketConfig`
///
/// Ensures configuration validity with proper validation and sensible defaults.
#[derive(Debug, Clone)]
pub struct WebSocketConfigBuilder {
    max_connections: Option<usize>,
    connection_timeout: Option<Duration>,
    ping_interval: Option<Duration>,
    pong_timeout: Option<Duration>,
    max_message_size: Option<usize>,
    enable_compression: bool,
    buffer_size: Option<usize>,
    max_subscriptions_per_connection: Option<usize>,
    max_subscribers_per_channel: Option<usize>,
    max_connections_per_ip: Option<usize>,
    broadcast_buffer_size: Option<usize>,
}

/// Errors that can occur when building a `WebSocketConfig`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketConfigError {
    /// Invalid timeout value
    InvalidTimeout(String),
    /// Invalid buffer/size value
    InvalidSize(String),
    /// Invalid limit value
    InvalidLimit(String),
}

impl std::fmt::Display for WebSocketConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTimeout(reason) => write!(f, "Invalid timeout: {}", reason),
            Self::InvalidSize(reason) => write!(f, "Invalid size: {}", reason),
            Self::InvalidLimit(reason) => write!(f, "Invalid limit: {}", reason),
        }
    }
}

impl std::error::Error for WebSocketConfigError {}

impl Default for WebSocketConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocketConfigBuilder {
    /// Create a new builder with all fields unset (will use defaults on build)
    pub fn new() -> Self {
        Self {
            max_connections: None,
            connection_timeout: None,
            ping_interval: None,
            pong_timeout: None,
            max_message_size: None,
            enable_compression: true,
            buffer_size: None,
            max_subscriptions_per_connection: None,
            max_subscribers_per_channel: None,
            max_connections_per_ip: None,
            broadcast_buffer_size: None,
        }
    }

    /// Set maximum concurrent connections (must be > 0 and <= 100,000)
    pub fn max_connections(mut self, max: usize) -> Result<Self, WebSocketConfigError> {
        if max == 0 {
            return Err(WebSocketConfigError::InvalidLimit(
                "max_connections must be greater than 0".to_string(),
            ));
        }
        if max > 100_000 {
            return Err(WebSocketConfigError::InvalidLimit(
                "max_connections cannot exceed 100,000".to_string(),
            ));
        }
        self.max_connections = Some(max);
        Ok(self)
    }

    /// Set connection timeout (must be between 1s and 300s)
    pub fn connection_timeout(mut self, timeout: Duration) -> Result<Self, WebSocketConfigError> {
        if timeout.as_secs() == 0 {
            return Err(WebSocketConfigError::InvalidTimeout(
                "connection_timeout must be at least 1 second".to_string(),
            ));
        }
        if timeout.as_secs() > 300 {
            return Err(WebSocketConfigError::InvalidTimeout(
                "connection_timeout cannot exceed 300 seconds (5 minutes)".to_string(),
            ));
        }
        self.connection_timeout = Some(timeout);
        Ok(self)
    }

    /// Set ping interval (must be between 5s and 120s)
    pub fn ping_interval(mut self, interval: Duration) -> Result<Self, WebSocketConfigError> {
        if interval.as_secs() < 5 {
            return Err(WebSocketConfigError::InvalidTimeout(
                "ping_interval must be at least 5 seconds".to_string(),
            ));
        }
        if interval.as_secs() > 120 {
            return Err(WebSocketConfigError::InvalidTimeout(
                "ping_interval cannot exceed 120 seconds".to_string(),
            ));
        }
        self.ping_interval = Some(interval);
        Ok(self)
    }

    /// Set pong timeout (must be between 1s and 60s)
    pub fn pong_timeout(mut self, timeout: Duration) -> Result<Self, WebSocketConfigError> {
        if timeout.as_secs() == 0 {
            return Err(WebSocketConfigError::InvalidTimeout(
                "pong_timeout must be at least 1 second".to_string(),
            ));
        }
        if timeout.as_secs() > 60 {
            return Err(WebSocketConfigError::InvalidTimeout(
                "pong_timeout cannot exceed 60 seconds".to_string(),
            ));
        }
        self.pong_timeout = Some(timeout);
        Ok(self)
    }

    /// Set maximum message size (must be between 1KB and 16MB)
    pub fn max_message_size(mut self, size: usize) -> Result<Self, WebSocketConfigError> {
        if size < 1024 {
            return Err(WebSocketConfigError::InvalidSize(
                "max_message_size must be at least 1KB (1024 bytes)".to_string(),
            ));
        }
        if size > 16 * 1024 * 1024 {
            return Err(WebSocketConfigError::InvalidSize(
                "max_message_size cannot exceed 16MB (16,777,216 bytes)".to_string(),
            ));
        }
        self.max_message_size = Some(size);
        Ok(self)
    }

    /// Enable or disable compression
    pub fn enable_compression(mut self, enable: bool) -> Self {
        self.enable_compression = enable;
        self
    }

    /// Set buffer size for incoming messages (must be between 10 and 10,000)
    pub fn buffer_size(mut self, size: usize) -> Result<Self, WebSocketConfigError> {
        if size < 10 {
            return Err(WebSocketConfigError::InvalidSize(
                "buffer_size must be at least 10".to_string(),
            ));
        }
        if size > 10_000 {
            return Err(WebSocketConfigError::InvalidSize(
                "buffer_size cannot exceed 10,000".to_string(),
            ));
        }
        self.buffer_size = Some(size);
        Ok(self)
    }

    /// Set maximum subscriptions per connection (must be between 1 and 1,000)
    pub fn max_subscriptions_per_connection(
        mut self,
        max: usize,
    ) -> Result<Self, WebSocketConfigError> {
        if max == 0 {
            return Err(WebSocketConfigError::InvalidLimit(
                "max_subscriptions_per_connection must be at least 1".to_string(),
            ));
        }
        if max > 1000 {
            return Err(WebSocketConfigError::InvalidLimit(
                "max_subscriptions_per_connection cannot exceed 1,000".to_string(),
            ));
        }
        self.max_subscriptions_per_connection = Some(max);
        Ok(self)
    }

    /// Set maximum subscribers per channel (must be between 1 and 100,000)
    pub fn max_subscribers_per_channel(mut self, max: usize) -> Result<Self, WebSocketConfigError> {
        if max == 0 {
            return Err(WebSocketConfigError::InvalidLimit(
                "max_subscribers_per_channel must be at least 1".to_string(),
            ));
        }
        if max > 100_000 {
            return Err(WebSocketConfigError::InvalidLimit(
                "max_subscribers_per_channel cannot exceed 100,000".to_string(),
            ));
        }
        self.max_subscribers_per_channel = Some(max);
        Ok(self)
    }

    /// Set maximum connections per IP address (must be between 1 and 1,000)
    pub fn max_connections_per_ip(mut self, max: usize) -> Result<Self, WebSocketConfigError> {
        if max == 0 {
            return Err(WebSocketConfigError::InvalidLimit(
                "max_connections_per_ip must be at least 1".to_string(),
            ));
        }
        if max > 1000 {
            return Err(WebSocketConfigError::InvalidLimit(
                "max_connections_per_ip cannot exceed 1,000".to_string(),
            ));
        }
        self.max_connections_per_ip = Some(max);
        Ok(self)
    }

    /// Set broadcast channel buffer size (must be between 10 and 100,000)
    pub fn broadcast_buffer_size(mut self, size: usize) -> Result<Self, WebSocketConfigError> {
        if size < 10 {
            return Err(WebSocketConfigError::InvalidSize(
                "broadcast_buffer_size must be at least 10".to_string(),
            ));
        }
        if size > 100_000 {
            return Err(WebSocketConfigError::InvalidSize(
                "broadcast_buffer_size cannot exceed 100,000".to_string(),
            ));
        }
        self.broadcast_buffer_size = Some(size);
        Ok(self)
    }

    /// Build the `WebSocketConfig` (uses defaults for unset fields)
    pub fn build(self) -> WebSocketConfig {
        let defaults = WebSocketConfig::default();

        WebSocketConfig {
            max_connections: self.max_connections.unwrap_or(defaults.max_connections),
            connection_timeout: self
                .connection_timeout
                .unwrap_or(defaults.connection_timeout),
            ping_interval: self.ping_interval.unwrap_or(defaults.ping_interval),
            pong_timeout: self.pong_timeout.unwrap_or(defaults.pong_timeout),
            max_message_size: self.max_message_size.unwrap_or(defaults.max_message_size),
            enable_compression: self.enable_compression,
            buffer_size: self.buffer_size.unwrap_or(defaults.buffer_size),
            max_subscriptions_per_connection: self
                .max_subscriptions_per_connection
                .unwrap_or(defaults.max_subscriptions_per_connection),
            max_subscribers_per_channel: self
                .max_subscribers_per_channel
                .unwrap_or(defaults.max_subscribers_per_channel),
            max_connections_per_ip: self
                .max_connections_per_ip
                .unwrap_or(defaults.max_connections_per_ip),
            broadcast_buffer_size: self
                .broadcast_buffer_size
                .unwrap_or(defaults.broadcast_buffer_size),
        }
    }
}

impl WebSocketConfig {
    /// Create a builder for constructing a validated `WebSocketConfig`
    pub fn builder() -> WebSocketConfigBuilder {
        WebSocketConfigBuilder::new()
    }
}

/// Typestate marker for unauthenticated connections
#[derive(Debug, Clone)]
pub struct Unauthenticated;

/// Typestate marker for authenticated connections
#[derive(Debug, Clone)]
pub struct Authenticated {
    /// User ID from authentication
    pub user_id: String,
}

/// WebSocket connection information with typestate pattern
///
/// This struct uses the typestate pattern to enforce authentication states at compile time.
/// - `ConnectionInfo<Unauthenticated>`: New connections that haven't been authenticated
/// - `ConnectionInfo<Authenticated>`: Authenticated connections with a user ID
#[derive(Debug, Clone)]
pub struct ConnectionInfo<State = Unauthenticated> {
    /// Unique connection ID
    pub id: Uuid,
    /// Client IP address
    pub addr: SocketAddr,
    /// Connection timestamp
    pub connected_at: Instant,
    /// Last activity timestamp
    pub last_activity: Instant,
    /// Connection metadata
    pub metadata: HashMap<String, String>,
    /// Authentication state (phantom marker)
    state: State,
}

// Common methods available in all states
impl<State> ConnectionInfo<State> {
    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if connection has expired
    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// Get connection ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get connection address
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get connection age
    pub fn age(&self) -> Duration {
        self.connected_at.elapsed()
    }

    /// Get time since last activity
    pub fn idle_time(&self) -> Duration {
        self.last_activity.elapsed()
    }
}

// Methods only available for unauthenticated connections
impl ConnectionInfo<Unauthenticated> {
    /// Create a new unauthenticated connection
    pub fn new(addr: SocketAddr) -> Self {
        let now = Instant::now();
        Self {
            id: Uuid::new_v4(),
            addr,
            connected_at: now,
            last_activity: now,
            metadata: HashMap::new(),
            state: Unauthenticated,
        }
    }

    /// Authenticate the connection, transitioning to authenticated state
    pub fn authenticate(self, user_id: String) -> ConnectionInfo<Authenticated> {
        ConnectionInfo {
            id: self.id,
            addr: self.addr,
            connected_at: self.connected_at,
            last_activity: self.last_activity,
            metadata: self.metadata,
            state: Authenticated { user_id },
        }
    }
}

// Methods only available for authenticated connections
impl ConnectionInfo<Authenticated> {
    /// Get the authenticated user ID
    pub fn user_id(&self) -> &str {
        &self.state.user_id
    }

    /// Check if this connection belongs to a specific user
    pub fn is_user(&self, user_id: &str) -> bool {
        self.state.user_id == user_id
    }
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WsMessage {
    /// Ping message
    Ping { timestamp: i64 },
    /// Pong message
    Pong { timestamp: i64 },
    /// Authentication message
    Auth { token: String },
    /// Subscribe to events
    Subscribe { channels: Vec<String> },
    /// Unsubscribe from events
    Unsubscribe { channels: Vec<String> },
    /// Event notification
    Event {
        channel: String,
        data: serde_json::Value,
        timestamp: i64,
    },
    /// Error message
    Error { code: String, message: String },
    /// Success acknowledgment
    Success { message: String },
}

impl WsMessage {
    pub fn ping() -> Self {
        Self::Ping {
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    pub fn pong() -> Self {
        Self::Pong {
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        Self::Error {
            code: code.to_string(),
            message: message.to_string(),
        }
    }

    pub fn success(message: &str) -> Self {
        Self::Success {
            message: message.to_string(),
        }
    }

    pub fn event(channel: &str, data: serde_json::Value) -> Self {
        Self::Event {
            channel: channel.to_string(),
            data,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

/// WebSocket handler result
pub type WsResult<T> = Result<T, WsError>;

/// WebSocket errors
#[derive(Debug, thiserror::Error)]
pub enum WsError {
    #[error("Connection limit exceeded")]
    ConnectionLimitExceeded,

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Message too large: {size} bytes (max: {max})")]
    MessageTooLarge { size: usize, max: usize },

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Subscription limit exceeded: {current} subscriptions (max: {max})")]
    SubscriptionLimitExceeded { current: usize, max: usize },

    #[error("Channel subscriber limit exceeded: {current} subscribers (max: {max})")]
    ChannelSubscriberLimitExceeded { current: usize, max: usize },

    #[error("Rate limit exceeded for IP address")]
    RateLimitExceeded,
}

impl WsError {
    pub fn to_message(&self) -> WsMessage {
        match self {
            WsError::ConnectionLimitExceeded => {
                WsMessage::error("CONNECTION_LIMIT_EXCEEDED", "Too many connections")
            }
            WsError::AuthenticationFailed(msg) => WsMessage::error("AUTHENTICATION_FAILED", msg),
            WsError::MessageTooLarge { size, max } => WsMessage::error(
                "MESSAGE_TOO_LARGE",
                &format!("Message size {} exceeds limit {}", size, max),
            ),
            WsError::InvalidMessage(msg) => WsMessage::error("INVALID_MESSAGE", msg),
            WsError::ConnectionClosed => {
                WsMessage::error("CONNECTION_CLOSED", "Connection was closed")
            }
            WsError::ChannelNotFound(channel) => WsMessage::error(
                "CHANNEL_NOT_FOUND",
                &format!("Channel '{}' not found", channel),
            ),
            WsError::PermissionDenied => WsMessage::error("PERMISSION_DENIED", "Permission denied"),
            WsError::Internal(msg) => WsMessage::error("INTERNAL_ERROR", msg),
            WsError::SubscriptionLimitExceeded { current, max } => WsMessage::error(
                "SUBSCRIPTION_LIMIT_EXCEEDED",
                &format!(
                    "Subscription limit exceeded: {} subscriptions (max: {})",
                    current, max
                ),
            ),
            WsError::ChannelSubscriberLimitExceeded { current, max } => WsMessage::error(
                "CHANNEL_SUBSCRIBER_LIMIT_EXCEEDED",
                &format!(
                    "Channel subscriber limit exceeded: {} subscribers (max: {})",
                    current, max
                ),
            ),
            WsError::RateLimitExceeded => {
                WsMessage::error("RATE_LIMIT_EXCEEDED", "Rate limit exceeded for IP address")
            }
        }
    }
}

/// WebSocket upgrade handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(manager): State<Arc<WebSocketManager>>,
) -> Response {
    info!("WebSocket connection request from {}", addr);

    ws.on_upgrade(move |socket| handle_socket(socket, addr, manager))
}

/// Handle individual WebSocket connection
async fn handle_socket(socket: WebSocket, addr: SocketAddr, manager: Arc<WebSocketManager>) {
    let conn_info = ConnectionInfo::new(addr);
    let conn_id = conn_info.id();

    info!(
        "WebSocket connection established: {} from {}",
        conn_id, addr
    );

    // Register connection with manager
    if let Err(e) = manager.add_connection(conn_id, conn_info).await {
        error!("Failed to register connection {}: {}", conn_id, e);
        return;
    }

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<WsMessage>(manager.config.buffer_size);

    // Start background tasks
    let manager_clone = Arc::clone(&manager);
    let tx_ping = tx.clone();
    let ping_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(manager_clone.config.ping_interval);
        loop {
            interval.tick().await;
            if tx_ping.send(WsMessage::ping()).await.is_err() {
                break;
            }
        }
    });

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json_msg = match serde_json::to_string(&msg) {
                Ok(json) => json,
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                    continue;
                }
            };

            if sender.send(Message::Text(json_msg.into())).await.is_err() {
                break;
            }
        }
    });

    let manager_clone = Arc::clone(&manager);
    let max_message_size = manager.config.max_message_size;
    let receive_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Validate message size before deserialization
                    if text.len() > max_message_size {
                        error!("Message too large from {}: {} bytes", conn_id, text.len());
                        let error_msg = WsError::MessageTooLarge {
                            size: text.len(),
                            max: max_message_size,
                        }
                        .to_message();
                        if tx.send(error_msg).await.is_err() {
                            break;
                        }
                        continue;
                    }

                    match serde_json::from_str::<WsMessage>(&text) {
                        Ok(ws_msg) => {
                            if let Err(e) = manager_clone.handle_message(conn_id, ws_msg).await {
                                error!("Error handling message from {}: {}", conn_id, e);
                                let error_msg = e.to_message();
                                if tx.send(error_msg).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Invalid JSON message from {}: {}", conn_id, e);
                            let error_msg = WsError::InvalidMessage(e.to_string()).to_message();
                            if tx.send(error_msg).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Ok(Message::Binary(_)) => {
                    warn!("Binary messages not supported from {}", conn_id);
                }
                Ok(Message::Close(_)) => {
                    info!("Connection {} closed by client", conn_id);
                    break;
                }
                Ok(Message::Ping(_data)) => {
                    if tx.send(WsMessage::pong()).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Pong(_)) => {
                    // Update last activity
                    manager_clone.update_activity(conn_id).await;
                }
                Err(e) => {
                    error!("WebSocket error from {}: {}", conn_id, e);
                    break;
                }
            }
        }
    });

    // Wait for any task to complete and handle panics
    tokio::select! {
        result = ping_task => {
            if let Err(e) = result {
                error!("Ping task panicked for connection {}: {:?}", conn_id, e);
            }
        }
        result = send_task => {
            if let Err(e) = result {
                error!("Send task panicked for connection {}: {:?}", conn_id, e);
            }
        }
        result = receive_task => {
            if let Err(e) = result {
                error!("Receive task panicked for connection {}: {:?}", conn_id, e);
            }
        }
    }

    // Cleanup
    info!("WebSocket connection {} closed", conn_id);
    manager.remove_connection(conn_id).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_config_default() {
        let config = WebSocketConfig::default();
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.connection_timeout, Duration::from_secs(60));
        assert_eq!(config.ping_interval, Duration::from_secs(30));
        assert_eq!(config.pong_timeout, Duration::from_secs(10));
        assert_eq!(config.max_message_size, 64 * 1024);
        assert!(config.enable_compression);
        assert_eq!(config.buffer_size, 100);
        assert_eq!(config.max_subscriptions_per_connection, 50);
        assert_eq!(config.max_subscribers_per_channel, 10000);
        assert_eq!(config.max_connections_per_ip, 10);
        assert_eq!(config.broadcast_buffer_size, 1000);
    }

    #[test]
    fn test_connection_info() {
        let addr = "127.0.0.1:8080".parse().unwrap();
        let mut conn_info = ConnectionInfo::new(addr);

        assert_eq!(conn_info.addr(), addr);
        assert!(!conn_info.is_expired(Duration::from_secs(1)));

        conn_info.update_activity();
        assert!(!conn_info.is_expired(Duration::from_secs(1)));
    }

    #[test]
    fn test_connection_typestate() {
        let addr = "127.0.0.1:8080".parse().unwrap();

        // Create unauthenticated connection
        let conn = ConnectionInfo::new(addr);
        assert_eq!(conn.addr(), addr);

        // Authenticate the connection
        let authed_conn = conn.authenticate("user123".to_string());
        assert_eq!(authed_conn.user_id(), "user123");
        assert!(authed_conn.is_user("user123"));
        assert!(!authed_conn.is_user("user456"));
    }

    #[test]
    fn test_ws_message_creation() {
        let ping = WsMessage::ping();
        assert!(matches!(ping, WsMessage::Ping { .. }));

        let pong = WsMessage::pong();
        assert!(matches!(pong, WsMessage::Pong { .. }));

        let error = WsMessage::error("TEST", "test error");
        assert!(matches!(error, WsMessage::Error { .. }));

        let success = WsMessage::success("test success");
        assert!(matches!(success, WsMessage::Success { .. }));

        let event = WsMessage::event("test", serde_json::json!({"key": "value"}));
        assert!(matches!(event, WsMessage::Event { .. }));
    }

    #[test]
    fn test_ws_error_to_message() {
        let error = WsError::ConnectionLimitExceeded;
        let msg = error.to_message();
        assert!(matches!(msg, WsMessage::Error { .. }));

        let error = WsError::AuthenticationFailed("invalid token".to_string());
        let msg = error.to_message();
        assert!(matches!(msg, WsMessage::Error { .. }));
    }

    #[test]
    fn test_ws_message_serialization() {
        let ping = WsMessage::ping();
        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("\"type\":\"ping\""));

        let event = WsMessage::event("test", serde_json::json!({"data": "value"}));
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"event\""));
        assert!(json.contains("\"channel\":\"test\""));
    }
}
