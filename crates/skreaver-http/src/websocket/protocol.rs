//! WebSocket protocol definitions and utilities

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// WebSocket protocol version
pub const PROTOCOL_VERSION: &str = "1.0";

/// Maximum message size (1MB)
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Standard channel names
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
    pub channel: String,
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
    pub channel: String,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseData {
    /// Response status
    pub status: ResponseStatus,
    /// Response result (for success)
    pub result: Option<serde_json::Value>,
    /// Error information (for failure)
    pub error: Option<ErrorInfo>,
}

/// Response status
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
        let channel_subs = channels.into_iter()
            .map(|channel| ChannelSubscription {
                channel,
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
    pub fn event(channel: &str, event_type: &str, data: serde_json::Value) -> Self {
        Self::new(MessagePayload::Event(EventData {
            channel: channel.to_string(),
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
        Self::new(MessagePayload::Response(ResponseData {
            status: ResponseStatus::Success,
            result: Some(result),
            error: None,
        }))
    }
    
    /// Create an error response message
    pub fn error_response(code: &str, message: &str) -> Self {
        Self::new(MessagePayload::Response(ResponseData {
            status: ResponseStatus::Error,
            result: None,
            error: Some(ErrorInfo {
                code: code.to_string(),
                message: message.to_string(),
                details: None,
            }),
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
            assert_eq!(data.channels[0].channel, "test-channel");
        } else {
            panic!("Expected Subscribe payload");
        }
    }
    
    #[test]
    fn test_event_message() {
        let data = serde_json::json!({"key": "value"});
        let envelope = MessageEnvelope::event("test-channel", "test-event", data.clone());
        
        if let MessagePayload::Event(event_data) = envelope.payload {
            assert_eq!(event_data.channel, "test-channel");
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
        
        if let MessagePayload::Response(resp_data) = response.payload {
            assert!(matches!(resp_data.status, ResponseStatus::Success));
            assert_eq!(resp_data.result, Some(result));
        } else {
            panic!("Expected Response payload");
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
        
        if let MessagePayload::Response(resp_data) = error_response.payload {
            assert!(matches!(resp_data.status, ResponseStatus::Error));
            assert!(resp_data.error.is_some());
        } else {
            panic!("Expected Response payload");
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
        let envelope = MessageEnvelope::ping()
            .with_correlation_id("test-correlation-id".to_string());
        
        assert_eq!(envelope.correlation_id, Some("test-correlation-id".to_string()));
    }
    
    #[test]
    fn test_serialization() {
        let envelope = MessageEnvelope::event(
            "test-channel",
            "test-event",
            serde_json::json!({"data": "test"})
        );
        
        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: MessageEnvelope = serde_json::from_str(&json).unwrap();
        
        assert_eq!(envelope.version, deserialized.version);
        assert_eq!(envelope.message_id, deserialized.message_id);
    }
}