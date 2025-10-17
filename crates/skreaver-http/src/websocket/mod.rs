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

/// WebSocket connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
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
}

impl ConnectionInfo {
    pub fn new(addr: SocketAddr) -> Self {
        let now = Instant::now();
        Self {
            id: Uuid::new_v4(),
            addr,
            connected_at: now,
            last_activity: now,
            metadata: HashMap::new(),
        }
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
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
    let conn_id = conn_info.id;

    info!(
        "WebSocket connection established: {} from {}",
        conn_id, addr
    );

    // Register connection with manager
    if let Err(e) = manager.add_connection(conn_id, conn_info.clone()).await {
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

        assert_eq!(conn_info.addr, addr);
        assert!(!conn_info.is_expired(Duration::from_secs(1)));

        conn_info.update_activity();
        assert!(!conn_info.is_expired(Duration::from_secs(1)));
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
