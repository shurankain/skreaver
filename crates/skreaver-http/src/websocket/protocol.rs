//! WebSocket protocol definitions and utilities

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// WebSocket protocol version
pub const PROTOCOL_VERSION: &str = "1.0";

/// Maximum message size (1MB)
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Type-safe WebSocket channel names
///
/// Represents the available channels for WebSocket subscriptions.
/// Using an enum ensures compile-time validation of channel names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// System events (server status, maintenance, etc.)
    System,
    /// Agent events (lifecycle, status changes, etc.)
    Agents,
    /// Task events (creation, updates, completion, etc.)
    Tasks,
    /// User-specific notifications
    Notifications,
    /// Real-time metrics and monitoring
    Metrics,
    /// Debug and development events
    Debug,
    /// Custom channel (for extensibility)
    #[serde(untagged)]
    Custom(String),
}

impl Channel {
    /// Get the canonical string name for this channel
    pub fn as_str(&self) -> &str {
        match self {
            Self::System => "system",
            Self::Agents => "agents",
            Self::Tasks => "tasks",
            Self::Notifications => "notifications",
            Self::Metrics => "metrics",
            Self::Debug => "debug",
            Self::Custom(name) => name,
        }
    }

    /// Check if this is a custom channel
    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }

    /// Check if this channel requires admin privileges
    ///
    /// SECURITY: This method checks BOTH the enum variant AND custom channel names
    /// that shadow privileged channel names to prevent authorization bypass attacks.
    /// An attacker cannot create Custom("system") to bypass admin requirements.
    pub fn requires_admin(&self) -> bool {
        match self {
            Self::System | Self::Metrics | Self::Debug => true,
            Self::Custom(name) => {
                // SECURITY: Prevent custom channels from shadowing privileged channels
                // This blocks Custom("system"), Custom("metrics"), Custom("debug")
                matches!(name.to_lowercase().as_str(), "system" | "metrics" | "debug")
            }
            _ => false,
        }
    }

    /// Validate that a custom channel name is safe to use
    ///
    /// Returns false if the channel name shadows a privileged channel
    /// or contains potentially dangerous characters.
    pub fn is_valid_custom_name(name: &str) -> bool {
        // Reject names that shadow privileged channels (case-insensitive)
        let lower = name.to_lowercase();
        if matches!(
            lower.as_str(),
            "system" | "metrics" | "debug" | "agents" | "tasks" | "notifications"
        ) {
            return false;
        }

        // Reject empty names
        if name.is_empty() {
            return false;
        }

        // Reject names that are too long
        if name.len() > 100 {
            return false;
        }

        // Only allow alphanumeric, dash, underscore, and dot
        name.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Channel {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "system" => Self::System,
            "agents" => Self::Agents,
            "tasks" => Self::Tasks,
            "notifications" => Self::Notifications,
            "metrics" => Self::Metrics,
            "debug" => Self::Debug,
            other => Self::Custom(other.to_string()),
        })
    }
}

impl From<&str> for Channel {
    fn from(s: &str) -> Self {
        s.parse().unwrap() // Infallible
    }
}

impl From<String> for Channel {
    fn from(s: String) -> Self {
        s.parse().unwrap() // Infallible
    }
}

/// Standard channel names (for backward compatibility)
pub mod channels {
    /// System events (server status, maintenance, etc.)
    pub const SYSTEM: &str = "system";

    /// Agent events (lifecycle, status changes, etc.)
    pub const AGENTS: &str = "agents";

    /// Task events (creation, updates, completion, etc.)
    pub const TASKS: &str = "tasks";

    /// User-specific notifications
    pub const NOTIFICATIONS: &str = "notifications";

    /// Real-time metrics and monitoring
    pub const METRICS: &str = "metrics";

    /// Debug and development events
    pub const DEBUG: &str = "debug";
}

/// Message envelope for protocol communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEnvelope {
    /// Protocol version
    pub version: String,
    /// Message ID for tracking
    pub message_id: String,
    /// Timestamp when message was created
    pub timestamp: i64,
    /// Message correlation ID (for request/response)
    pub correlation_id: Option<String>,
    /// Message payload
    #[serde(flatten)]
    pub payload: MessagePayload,
}

impl MessageEnvelope {
    pub fn new(payload: MessagePayload) -> Self {
        Self {
            version: PROTOCOL_VERSION.to_string(),
            message_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            correlation_id: None,
            payload,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

/// Message payload types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum MessagePayload {
    /// Connection handshake
    Handshake(HandshakeData),
    /// Authentication request/response
    Auth(AuthData),
    /// Subscription management
    Subscribe(SubscribeData),
    /// Event notification
    Event(EventData),
    /// Request/response pattern
    Request(RequestData),
    Response(ResponseData),
    /// Error notification
    Error(ErrorData),
    /// Health check
    Ping(PingData),
    Pong(PongData),
}

/// Handshake data for connection establishment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeData {
    /// Client name/identifier
    pub client_name: String,
    /// Client version
    pub client_version: String,
    /// Supported protocol versions
    pub supported_versions: Vec<String>,
    /// Client capabilities
    pub capabilities: Vec<String>,
    /// Optional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Authentication data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthData {
    /// Authentication method
    pub method: AuthMethod,
    /// Authentication token/credentials
    pub token: String,
    /// Optional user information
    pub user_info: Option<UserInfo>,
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethod {
    /// Bearer token (JWT)
    Bearer,
    /// API key
    ApiKey,
    /// Basic authentication
    Basic,
    /// Custom authentication
    Custom(String),
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    /// User ID
    pub user_id: String,
    /// User name
    pub username: Option<String>,
    /// User email
    pub email: Option<String>,
    /// User roles
    pub roles: Vec<String>,
    /// User permissions
    pub permissions: Vec<String>,
}

/// Subscription data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeData {
    /// Subscription action
    pub action: SubscriptionAction,
    /// Channel names
    pub channels: Vec<ChannelSubscription>,
}

/// Subscription actions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubscriptionAction {
    /// Subscribe to channels
    Subscribe,
    /// Unsubscribe from channels
    Unsubscribe,
    /// List current subscriptions
    List,
}

/// Channel subscription with filters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSubscription {
    /// Channel name
    pub channel: Channel,
    /// Optional filters
    pub filters: Option<HashMap<String, serde_json::Value>>,
    /// Quality of service level
    pub qos: QosLevel,
}

/// Quality of service levels
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QosLevel {
    /// At most once delivery
    AtMostOnce,
    /// At least once delivery
    AtLeastOnce,
    /// Exactly once delivery
    ExactlyOnce,
}

impl Default for QosLevel {
    fn default() -> Self {
        Self::AtMostOnce
    }
}

/// Event data for notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventData {
    /// Event source channel
    pub channel: Channel,
    /// Event type
    pub event_type: String,
    /// Event payload
    pub data: serde_json::Value,
    /// Event metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// Event sequence number
    pub sequence: Option<u64>,
}

/// Request data for RPC-style communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestData {
    /// Request method/operation
    pub method: String,
    /// Request parameters
    pub params: serde_json::Value,
    /// Timeout in milliseconds
    pub timeout: Option<u64>,
}

/// Response data for RPC-style communication
/// Response data with type-safe status variants
/// This design makes invalid states unrepresentable - you cannot have
/// a Success response with an error, or an Error response with a result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ResponseData {
    /// Request completed successfully with a result
    Success {
        /// The successful result value
        result: serde_json::Value,
    },
    /// Request failed with an error
    Error {
        /// Error information
        #[serde(flatten)]
        error: ErrorInfo,
    },
    /// Request is still processing
    Pending,
    /// Request was cancelled
    Cancelled,
}

// Keep ResponseStatus for backward compatibility if needed elsewhere
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResponseStatus {
    /// Request completed successfully
    Success,
    /// Request failed with error
    Error,
    /// Request is still processing
    Pending,
    /// Request was cancelled
    Cancelled,
}

/// Error information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
    /// Additional error details
    pub details: Option<serde_json::Value>,
}

/// Error data for error notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    /// Error information
    #[serde(flatten)]
    pub error: ErrorInfo,
    /// Context where error occurred
    pub context: Option<String>,
}

/// Ping data for health checks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingData {
    /// Ping timestamp
    pub timestamp: i64,
    /// Optional ping payload
    pub payload: Option<serde_json::Value>,
}

/// Pong data for health check responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PongData {
    /// Original ping timestamp
    pub ping_timestamp: i64,
    /// Pong timestamp
    pub pong_timestamp: i64,
    /// Optional pong payload
    pub payload: Option<serde_json::Value>,
}

/// System event types
pub mod events {
    /// Server startup event
    pub const SERVER_STARTED: &str = "server.started";
    /// Server shutdown event
    pub const SERVER_SHUTDOWN: &str = "server.shutdown";
    /// Server maintenance mode
    pub const SERVER_MAINTENANCE: &str = "server.maintenance";

    /// Agent created
    pub const AGENT_CREATED: &str = "agent.created";
    /// Agent updated
    pub const AGENT_UPDATED: &str = "agent.updated";
    /// Agent deleted
    pub const AGENT_DELETED: &str = "agent.deleted";
    /// Agent started
    pub const AGENT_STARTED: &str = "agent.started";
    /// Agent stopped
    pub const AGENT_STOPPED: &str = "agent.stopped";
    /// Agent error
    pub const AGENT_ERROR: &str = "agent.error";

    /// Task created
    pub const TASK_CREATED: &str = "task.created";
    /// Task updated
    pub const TASK_UPDATED: &str = "task.updated";
    /// Task completed
    pub const TASK_COMPLETED: &str = "task.completed";
    /// Task failed
    pub const TASK_FAILED: &str = "task.failed";
    /// Task cancelled
    pub const TASK_CANCELLED: &str = "task.cancelled";

    /// User notification
    pub const USER_NOTIFICATION: &str = "user.notification";
    /// System notification
    pub const SYSTEM_NOTIFICATION: &str = "system.notification";

    /// Metrics update
    pub const METRICS_UPDATE: &str = "metrics.update";
    /// Health check result
    pub const HEALTH_CHECK: &str = "health.check";

    /// Debug log
    pub const DEBUG_LOG: &str = "debug.log";
    /// Debug event
    pub const DEBUG_EVENT: &str = "debug.event";
}

/// Protocol utility functions
impl MessageEnvelope {
    /// Create a handshake message
    pub fn handshake(data: HandshakeData) -> Self {
        Self::new(MessagePayload::Handshake(data))
    }

    /// Create an auth message
    pub fn auth(data: AuthData) -> Self {
        Self::new(MessagePayload::Auth(data))
    }

    /// Create a subscribe message
    pub fn subscribe(channels: Vec<String>) -> Self {
        let channel_subs = channels
            .into_iter()
            .map(|channel| ChannelSubscription {
                channel: channel.into(),
                filters: None,
                qos: QosLevel::default(),
            })
            .collect();

        Self::new(MessagePayload::Subscribe(SubscribeData {
            action: SubscriptionAction::Subscribe,
            channels: channel_subs,
        }))
    }

    /// Create an event message
    pub fn event(channel: Channel, event_type: &str, data: serde_json::Value) -> Self {
        Self::new(MessagePayload::Event(EventData {
            channel,
            event_type: event_type.to_string(),
            data,
            metadata: None,
            sequence: None,
        }))
    }

    /// Create a request message
    pub fn request(method: &str, params: serde_json::Value) -> Self {
        Self::new(MessagePayload::Request(RequestData {
            method: method.to_string(),
            params,
            timeout: None,
        }))
    }

    /// Create a success response message
    pub fn success_response(result: serde_json::Value) -> Self {
        Self::new(MessagePayload::Response(ResponseData::Success { result }))
    }

    /// Create an error response message
    pub fn error_response(code: &str, message: &str) -> Self {
        Self::new(MessagePayload::Response(ResponseData::Error {
            error: ErrorInfo {
                code: code.to_string(),
                message: message.to_string(),
                details: None,
            },
        }))
    }

    /// Create an error message
    pub fn error(code: &str, message: &str) -> Self {
        Self::new(MessagePayload::Error(ErrorData {
            error: ErrorInfo {
                code: code.to_string(),
                message: message.to_string(),
                details: None,
            },
            context: None,
        }))
    }

    /// Create a ping message
    pub fn ping() -> Self {
        Self::new(MessagePayload::Ping(PingData {
            timestamp: chrono::Utc::now().timestamp_millis(),
            payload: None,
        }))
    }

    /// Create a pong message
    pub fn pong(ping_timestamp: i64) -> Self {
        Self::new(MessagePayload::Pong(PongData {
            ping_timestamp,
            pong_timestamp: chrono::Utc::now().timestamp_millis(),
            payload: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_envelope_creation() {
        let envelope = MessageEnvelope::ping();
        assert_eq!(envelope.version, PROTOCOL_VERSION);
        assert!(!envelope.message_id.is_empty());
        assert!(envelope.timestamp > 0);
        assert!(matches!(envelope.payload, MessagePayload::Ping(_)));
    }

    #[test]
    fn test_handshake_message() {
        let handshake_data = HandshakeData {
            client_name: "test-client".to_string(),
            client_version: "1.0.0".to_string(),
            supported_versions: vec!["1.0".to_string()],
            capabilities: vec!["auth".to_string(), "events".to_string()],
            metadata: HashMap::new(),
        };

        let envelope = MessageEnvelope::handshake(handshake_data);
        assert!(matches!(envelope.payload, MessagePayload::Handshake(_)));
    }

    #[test]
    fn test_auth_message() {
        let auth_data = AuthData {
            method: AuthMethod::Bearer,
            token: "test-token".to_string(),
            user_info: None,
        };

        let envelope = MessageEnvelope::auth(auth_data);
        assert!(matches!(envelope.payload, MessagePayload::Auth(_)));
    }

    #[test]
    fn test_subscribe_message() {
        let channels = vec!["test-channel".to_string()];
        let envelope = MessageEnvelope::subscribe(channels);

        if let MessagePayload::Subscribe(data) = envelope.payload {
            assert!(matches!(data.action, SubscriptionAction::Subscribe));
            assert_eq!(data.channels.len(), 1);
            assert_eq!(data.channels[0].channel, "test-channel".into());
        } else {
            panic!("Expected Subscribe payload");
        }
    }

    #[test]
    fn test_event_message() {
        let data = serde_json::json!({"key": "value"});
        let envelope = MessageEnvelope::event("test-channel".into(), "test-event", data.clone());

        if let MessagePayload::Event(event_data) = envelope.payload {
            assert_eq!(event_data.channel, "test-channel".into());
            assert_eq!(event_data.event_type, "test-event");
            assert_eq!(event_data.data, data);
        } else {
            panic!("Expected Event payload");
        }
    }

    #[test]
    fn test_request_response() {
        let params = serde_json::json!({"param": "value"});
        let request = MessageEnvelope::request("test-method", params);

        if let MessagePayload::Request(req_data) = request.payload {
            assert_eq!(req_data.method, "test-method");
        } else {
            panic!("Expected Request payload");
        }

        let result = serde_json::json!({"result": "success"});
        let response = MessageEnvelope::success_response(result.clone());

        if let MessagePayload::Response(ResponseData::Success { result: res }) = response.payload {
            assert_eq!(res, result);
        } else {
            panic!("Expected Success response");
        }
    }

    #[test]
    fn test_error_messages() {
        let error = MessageEnvelope::error("TEST_ERROR", "Test error message");

        if let MessagePayload::Error(error_data) = error.payload {
            assert_eq!(error_data.error.code, "TEST_ERROR");
            assert_eq!(error_data.error.message, "Test error message");
        } else {
            panic!("Expected Error payload");
        }

        let error_response = MessageEnvelope::error_response("REQUEST_FAILED", "Request failed");

        if let MessagePayload::Response(ResponseData::Error { error: err }) = error_response.payload
        {
            assert_eq!(err.code, "REQUEST_FAILED");
            assert_eq!(err.message, "Request failed");
        } else {
            panic!("Expected Error response");
        }
    }

    #[test]
    fn test_ping_pong() {
        let ping = MessageEnvelope::ping();

        let ping_timestamp = if let MessagePayload::Ping(ping_data) = ping.payload {
            ping_data.timestamp
        } else {
            panic!("Expected Ping payload");
        };

        let pong = MessageEnvelope::pong(ping_timestamp);

        if let MessagePayload::Pong(pong_data) = pong.payload {
            assert_eq!(pong_data.ping_timestamp, ping_timestamp);
            assert!(pong_data.pong_timestamp >= ping_timestamp);
        } else {
            panic!("Expected Pong payload");
        }
    }

    #[test]
    fn test_qos_level_default() {
        let qos = QosLevel::default();
        assert!(matches!(qos, QosLevel::AtMostOnce));
    }

    #[test]
    fn test_correlation_id() {
        let envelope =
            MessageEnvelope::ping().with_correlation_id("test-correlation-id".to_string());

        assert_eq!(
            envelope.correlation_id,
            Some("test-correlation-id".to_string())
        );
    }

    #[test]
    fn test_serialization() {
        let envelope = MessageEnvelope::event(
            "test-channel".into(),
            "test-event",
            serde_json::json!({"data": "test"}),
        );

        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: MessageEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(envelope.version, deserialized.version);
        assert_eq!(envelope.message_id, deserialized.message_id);
    }

    #[test]
    fn test_channel_enum_standard() {
        assert_eq!(Channel::System.as_str(), "system");
        assert_eq!(Channel::Agents.as_str(), "agents");
        assert_eq!(Channel::Tasks.as_str(), "tasks");
        assert_eq!(Channel::Notifications.as_str(), "notifications");
        assert_eq!(Channel::Metrics.as_str(), "metrics");
        assert_eq!(Channel::Debug.as_str(), "debug");
    }

    #[test]
    fn test_channel_enum_custom() {
        let custom = Channel::Custom("my-channel".to_string());
        assert_eq!(custom.as_str(), "my-channel");
        assert!(custom.is_custom());
        assert!(!Channel::System.is_custom());
    }

    #[test]
    fn test_channel_enum_from_str() {
        assert_eq!("system".parse::<Channel>().unwrap(), Channel::System);
        assert_eq!("agents".parse::<Channel>().unwrap(), Channel::Agents);
        assert_eq!(
            "custom-channel".parse::<Channel>().unwrap(),
            Channel::Custom("custom-channel".to_string())
        );
    }

    #[test]
    fn test_channel_enum_admin_check() {
        assert!(Channel::System.requires_admin());
        assert!(Channel::Metrics.requires_admin());
        assert!(Channel::Debug.requires_admin());
        assert!(!Channel::Agents.requires_admin());
        assert!(!Channel::Tasks.requires_admin());
        assert!(!Channel::Notifications.requires_admin());
    }

    #[test]
    fn test_channel_custom_shadowing_blocked() {
        // SECURITY: Custom channels that shadow privileged names must require admin
        assert!(Channel::Custom("system".to_string()).requires_admin());
        assert!(Channel::Custom("System".to_string()).requires_admin());
        assert!(Channel::Custom("SYSTEM".to_string()).requires_admin());
        assert!(Channel::Custom("metrics".to_string()).requires_admin());
        assert!(Channel::Custom("Metrics".to_string()).requires_admin());
        assert!(Channel::Custom("debug".to_string()).requires_admin());
        assert!(Channel::Custom("Debug".to_string()).requires_admin());

        // Valid custom channels should NOT require admin
        assert!(!Channel::Custom("my-channel".to_string()).requires_admin());
        assert!(!Channel::Custom("user-events".to_string()).requires_admin());
    }

    #[test]
    fn test_channel_custom_name_validation() {
        // Valid custom names
        assert!(Channel::is_valid_custom_name("my-channel"));
        assert!(Channel::is_valid_custom_name("user_events"));
        assert!(Channel::is_valid_custom_name("channel.v1"));
        assert!(Channel::is_valid_custom_name("Channel123"));

        // Invalid: shadow privileged channels
        assert!(!Channel::is_valid_custom_name("system"));
        assert!(!Channel::is_valid_custom_name("System"));
        assert!(!Channel::is_valid_custom_name("SYSTEM"));
        assert!(!Channel::is_valid_custom_name("metrics"));
        assert!(!Channel::is_valid_custom_name("debug"));
        assert!(!Channel::is_valid_custom_name("agents"));
        assert!(!Channel::is_valid_custom_name("tasks"));
        assert!(!Channel::is_valid_custom_name("notifications"));

        // Invalid: empty or too long
        assert!(!Channel::is_valid_custom_name(""));
        assert!(!Channel::is_valid_custom_name(&"x".repeat(101)));

        // Invalid: special characters
        assert!(!Channel::is_valid_custom_name("channel<script>"));
        assert!(!Channel::is_valid_custom_name("channel name")); // spaces
        assert!(!Channel::is_valid_custom_name("channel@test"));
    }

    #[test]
    fn test_channel_enum_display() {
        assert_eq!(Channel::System.to_string(), "system");
        assert_eq!(Channel::Agents.to_string(), "agents");
        let custom = Channel::Custom("test".to_string());
        assert_eq!(custom.to_string(), "test");
    }

    #[test]
    fn test_channel_enum_serialization() {
        let channel = Channel::Agents;
        let json = serde_json::to_string(&channel).unwrap();
        assert_eq!(json, "\"agents\"");

        let deserialized: Channel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Channel::Agents);
    }

    #[test]
    fn test_channel_subscription_with_enum() {
        let subscription = ChannelSubscription {
            channel: Channel::Agents,
            filters: None,
            qos: QosLevel::default(),
        };

        let json = serde_json::to_string(&subscription).unwrap();
        let deserialized: ChannelSubscription = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.channel, Channel::Agents);
    }
}

/// Result of attempting to send a message to a WebSocket connection
///
/// This type provides explicit information about message delivery status,
/// preventing silent failures and enabling proper error handling.
///
/// # Examples
///
/// ```
/// use skreaver_http::websocket::SendResult;
///
/// let send_result = SendResult::Sent;
/// match send_result {
///     SendResult::Sent => {
///         // Message was delivered to the send queue
///     }
///     SendResult::Queued { queue_size } => {
///         // Message queued but queue is getting full
///         if queue_size > 100 {
///             // Consider backpressure
///         }
///     }
///     SendResult::ConnectionClosed => {
///         // Connection no longer exists - can't send
///     }
///     SendResult::BufferFull => {
///         // Queue is full - message was NOT sent
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendResult {
    /// Message was successfully sent to the connection's send queue
    Sent,

    /// Message was queued but the queue is filling up
    ///
    /// The caller should consider implementing backpressure if queue_size
    /// exceeds expected thresholds.
    Queued {
        /// Current number of messages in the send queue
        queue_size: usize,
    },

    /// Connection does not exist or has been closed
    ///
    /// The message was NOT delivered.
    ConnectionClosed,

    /// The send buffer is full and cannot accept more messages
    ///
    /// The message was NOT delivered. The caller should implement
    /// backpressure or retry logic.
    BufferFull,
}

impl SendResult {
    /// Returns true if the message was successfully sent or queued
    pub fn is_success(&self) -> bool {
        matches!(self, SendResult::Sent | SendResult::Queued { .. })
    }

    /// Returns true if the message was NOT delivered
    pub fn is_failure(&self) -> bool {
        !self.is_success()
    }

    /// Returns the queue size if the message was queued
    pub fn queue_size(&self) -> Option<usize> {
        match self {
            SendResult::Queued { queue_size } => Some(*queue_size),
            _ => None,
        }
    }
}

impl std::fmt::Display for SendResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendResult::Sent => write!(f, "sent"),
            SendResult::Queued { queue_size } => write!(f, "queued (size: {})", queue_size),
            SendResult::ConnectionClosed => write!(f, "connection closed"),
            SendResult::BufferFull => write!(f, "buffer full"),
        }
    }
}
