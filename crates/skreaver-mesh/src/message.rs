//! Message types and builders for agent communication

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::AgentId;
use std::marker::PhantomData;

/// Typestate marker: Message routing is not yet determined
pub struct Unrouted;

/// Typestate marker: Message has unicast routing
pub struct UnicastRoute;

/// Typestate marker: Message has broadcast routing
pub struct BroadcastRoute;

/// Typestate marker: Message has system routing
pub struct SystemRoute;

/// Typestate marker: Message has anonymous routing
pub struct AnonymousRoute;

/// Type-safe message routing pattern
///
/// Replaces runtime boolean checks with compile-time routing patterns.
/// Each variant explicitly encodes the routing semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// Direct message from one agent to another
    /// Guarantees: Both sender and recipient are known
    Unicast { from: AgentId, to: AgentId },
    /// Broadcast from an agent to all listeners
    /// Guarantees: Sender is known, no specific recipient
    Broadcast { from: AgentId },
    /// System message to a specific agent
    /// Guarantees: Recipient is known, sender is the system
    System { to: AgentId },
    /// System-wide broadcast (for infrastructure messages)
    /// Guarantees: No specific sender or recipient
    Anonymous,
}

impl Route {
    /// Create a unicast route from one agent to another
    pub fn unicast(from: impl Into<AgentId>, to: impl Into<AgentId>) -> Self {
        Route::Unicast {
            from: from.into(),
            to: to.into(),
        }
    }

    /// Create a broadcast route from an agent
    pub fn broadcast(from: impl Into<AgentId>) -> Self {
        Route::Broadcast { from: from.into() }
    }

    /// Create a system route to a specific agent
    pub fn system(to: impl Into<AgentId>) -> Self {
        Route::System { to: to.into() }
    }

    /// Create an anonymous route (system-wide)
    pub fn anonymous() -> Self {
        Route::Anonymous
    }

    /// Get the sender agent ID if present
    pub fn sender(&self) -> Option<&AgentId> {
        match self {
            Route::Unicast { from, .. } | Route::Broadcast { from } => Some(from),
            Route::System { .. } | Route::Anonymous => None,
        }
    }

    /// Get the recipient agent ID if present
    pub fn recipient(&self) -> Option<&AgentId> {
        match self {
            Route::Unicast { to, .. } | Route::System { to } => Some(to),
            Route::Broadcast { .. } | Route::Anonymous => None,
        }
    }

    /// Check if this route targets a specific recipient
    pub fn has_recipient(&self) -> bool {
        matches!(self, Route::Unicast { .. } | Route::System { .. })
    }

    /// Check if this route has a known sender
    pub fn has_sender(&self) -> bool {
        matches!(self, Route::Unicast { .. } | Route::Broadcast { .. })
    }
}

/// Error type for MessageId validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageIdError {
    /// The provided string is not a valid UUID
    InvalidFormat(String),
}

impl std::fmt::Display for MessageIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageIdError::InvalidFormat(s) => {
                write!(f, "Invalid MessageId format (expected UUID): '{}'", s)
            }
        }
    }
}

impl std::error::Error for MessageIdError {}

/// Unique identifier for a message (UUID v4)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

impl MessageId {
    /// Create a new random message ID (UUID v4)
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Parse and validate a message ID from a string
    ///
    /// Returns an error if the string is not a valid UUID format.
    ///
    /// # Examples
    ///
    /// ```
    /// use skreaver_mesh::MessageId;
    ///
    /// // Valid UUID
    /// let id = MessageId::parse("550e8400-e29b-41d4-a716-446655440000").unwrap();
    ///
    /// // Invalid format
    /// assert!(MessageId::parse("not-a-uuid").is_err());
    /// assert!(MessageId::parse("").is_err());
    /// ```
    pub fn parse(id: impl AsRef<str>) -> Result<Self, MessageIdError> {
        let s = id.as_ref();

        // Validate UUID format
        Uuid::parse_str(s).map_err(|_| MessageIdError::InvalidFormat(s.to_string()))?;

        Ok(Self(s.to_string()))
    }

    /// Create a message ID from a string without validation
    ///
    /// # Panics
    /// Panics if the string is not a valid UUID format.
    /// For non-panicking construction, use `MessageId::parse()` instead.
    pub fn from_string(id: String) -> Self {
        Self::parse(&id).unwrap_or_else(|e| panic!("Invalid MessageId: {}", e))
    }

    /// Get the message ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Message payload - can be any serializable type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MessagePayload {
    /// String payload
    #[serde(rename = "text")]
    Text(String),
    /// JSON payload
    #[serde(rename = "json")]
    Json(serde_json::Value),
    /// Binary payload (base64 encoded in JSON)
    #[serde(rename = "binary")]
    #[serde(with = "base64_serde")]
    Binary(Vec<u8>),
}

mod base64_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        ))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
            .map_err(serde::de::Error::custom)
    }
}

impl From<String> for MessagePayload {
    fn from(s: String) -> Self {
        MessagePayload::Text(s)
    }
}

impl From<&str> for MessagePayload {
    fn from(s: &str) -> Self {
        MessagePayload::Text(s.to_string())
    }
}

impl From<serde_json::Value> for MessagePayload {
    fn from(v: serde_json::Value) -> Self {
        MessagePayload::Json(v)
    }
}

impl From<Vec<u8>> for MessagePayload {
    fn from(v: Vec<u8>) -> Self {
        MessagePayload::Binary(v)
    }
}

/// Message metadata
pub type MessageMetadata = HashMap<String, String>;

/// A message sent between agents in the mesh
///
/// ## Type-Safe Routing
/// Messages now use explicit `Route` enum for compile-time routing guarantees:
/// - **Route::Unicast**: Direct message from agent A to agent B
/// - **Route::Broadcast**: Broadcast from agent A to all listeners
/// - **Route::System**: System message to a specific agent
/// - **Route::Anonymous**: System broadcast (rare, for infrastructure)
///
/// ## Backward Compatibility
/// Legacy `from`/`to` fields are maintained for backward compatibility but deprecated.
/// Use the `route` field for new code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier
    pub id: MessageId,
    /// Type-safe routing information
    pub route: Route,
    /// DEPRECATED: Use route.sender() instead
    /// Sender agent ID (None for system messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[deprecated(since = "0.4.0", note = "Use route.sender() instead")]
    pub from: Option<AgentId>,
    /// DEPRECATED: Use route.recipient() instead
    /// Recipient agent ID (None for broadcasts)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[deprecated(since = "0.4.0", note = "Use route.recipient() instead")]
    pub to: Option<AgentId>,
    /// Message payload
    pub payload: MessagePayload,
    /// Message metadata (arbitrary key-value pairs)
    #[serde(default)]
    pub metadata: MessageMetadata,
    /// Timestamp when message was created
    pub timestamp: DateTime<Utc>,
    /// Optional correlation ID for request/reply patterns
    pub correlation_id: Option<String>,
}

impl Message {
    /// Create a new anonymous message with the given payload
    pub fn new(payload: impl Into<MessagePayload>) -> Self {
        Self {
            id: MessageId::new(),
            route: Route::Anonymous,
            #[allow(deprecated)]
            from: None,
            #[allow(deprecated)]
            to: None,
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            correlation_id: None,
        }
    }

    /// Create a unicast message from one agent to another
    pub fn unicast(
        from: impl Into<AgentId>,
        to: impl Into<AgentId>,
        payload: impl Into<MessagePayload>,
    ) -> Self {
        let from_id = from.into();
        let to_id = to.into();
        Self {
            id: MessageId::new(),
            route: Route::Unicast {
                from: from_id.clone(),
                to: to_id.clone(),
            },
            #[allow(deprecated)]
            from: Some(from_id),
            #[allow(deprecated)]
            to: Some(to_id),
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            correlation_id: None,
        }
    }

    /// Create a broadcast message from an agent
    pub fn broadcast(from: impl Into<AgentId>, payload: impl Into<MessagePayload>) -> Self {
        let from_id = from.into();
        Self {
            id: MessageId::new(),
            route: Route::Broadcast {
                from: from_id.clone(),
            },
            #[allow(deprecated)]
            from: Some(from_id),
            #[allow(deprecated)]
            to: None,
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            correlation_id: None,
        }
    }

    /// Create a system message to a specific agent
    pub fn system(to: impl Into<AgentId>, payload: impl Into<MessagePayload>) -> Self {
        let to_id = to.into();
        Self {
            id: MessageId::new(),
            route: Route::System { to: to_id.clone() },
            #[allow(deprecated)]
            from: None,
            #[allow(deprecated)]
            to: Some(to_id),
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            correlation_id: None,
        }
    }

    /// DEPRECATED: Use Message::unicast, Message::broadcast, or Message::system instead
    #[deprecated(since = "0.4.0", note = "Use Message::unicast() instead")]
    pub fn from(mut self, agent_id: impl Into<AgentId>) -> Self {
        let agent_id = agent_id.into();
        #[allow(deprecated)]
        {
            self.from = Some(agent_id.clone());
        }
        // Update route based on current state
        self.route = match self.route {
            Route::Anonymous => Route::Broadcast { from: agent_id },
            Route::System { to } => Route::Unicast { from: agent_id, to },
            _ => self.route, // Keep existing route
        };
        self
    }

    /// DEPRECATED: Use Message::unicast, Message::broadcast, or Message::system instead
    #[deprecated(
        since = "0.4.0",
        note = "Use Message::unicast() or Message::system() instead"
    )]
    pub fn to(mut self, agent_id: impl Into<AgentId>) -> Self {
        let agent_id = agent_id.into();
        #[allow(deprecated)]
        {
            self.to = Some(agent_id.clone());
        }
        // Update route based on current state
        self.route = match self.route {
            Route::Anonymous => Route::System { to: agent_id },
            Route::Broadcast { from } => Route::Unicast { from, to: agent_id },
            _ => self.route, // Keep existing route
        };
        self
    }

    /// Add metadata to the message
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set correlation ID for request/reply pattern
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Get metadata value by key
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Serialize message to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize message from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    // Routing pattern helpers

    /// Check if this is a unicast message (from agent to agent)
    pub fn is_unicast(&self) -> bool {
        matches!(self.route, Route::Unicast { .. })
    }

    /// Check if this is a broadcast message (from agent to all)
    pub fn is_broadcast(&self) -> bool {
        matches!(self.route, Route::Broadcast { .. })
    }

    /// Check if this is a system message (to specific agent, no sender)
    pub fn is_system(&self) -> bool {
        matches!(self.route, Route::System { .. })
    }

    /// Check if this is an anonymous message (no sender, no recipient)
    pub fn is_anonymous(&self) -> bool {
        matches!(self.route, Route::Anonymous)
    }

    /// Get the sender agent ID from the route
    pub fn sender(&self) -> Option<&AgentId> {
        self.route.sender()
    }

    /// Get the recipient agent ID from the route
    pub fn recipient(&self) -> Option<&AgentId> {
        self.route.recipient()
    }

    /// Get the route information
    pub fn route(&self) -> &Route {
        &self.route
    }
}

/// Builder for creating messages with a fluent API
pub struct MessageBuilder {
    message: Message,
}

impl MessageBuilder {
    /// Create a new anonymous message builder
    pub fn new(payload: impl Into<MessagePayload>) -> Self {
        Self {
            message: Message::new(payload),
        }
    }

    /// Create a unicast message builder
    pub fn unicast(
        from: impl Into<AgentId>,
        to: impl Into<AgentId>,
        payload: impl Into<MessagePayload>,
    ) -> Self {
        Self {
            message: Message::unicast(from, to, payload),
        }
    }

    /// Create a broadcast message builder
    pub fn broadcast(from: impl Into<AgentId>, payload: impl Into<MessagePayload>) -> Self {
        Self {
            message: Message::broadcast(from, payload),
        }
    }

    /// Create a system message builder
    pub fn system(to: impl Into<AgentId>, payload: impl Into<MessagePayload>) -> Self {
        Self {
            message: Message::system(to, payload),
        }
    }

    /// DEPRECATED: Use MessageBuilder::unicast or MessageBuilder::broadcast instead
    #[deprecated(
        since = "0.4.0",
        note = "Use MessageBuilder::unicast() or broadcast() instead"
    )]
    pub fn from(mut self, agent_id: impl Into<AgentId>) -> Self {
        #[allow(deprecated)]
        {
            self.message = self.message.from(agent_id);
        }
        self
    }

    /// DEPRECATED: Use MessageBuilder::unicast or MessageBuilder::system instead
    #[deprecated(
        since = "0.4.0",
        note = "Use MessageBuilder::unicast() or system() instead"
    )]
    pub fn to(mut self, agent_id: impl Into<AgentId>) -> Self {
        #[allow(deprecated)]
        {
            self.message = self.message.to(agent_id);
        }
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.message = self.message.with_metadata(key, value);
        self
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.message = self.message.with_correlation_id(correlation_id);
        self
    }

    /// Build the message
    pub fn build(self) -> Message {
        self.message
    }
}

/// Type-safe message builder using typestate pattern
///
/// This eliminates the possibility of inconsistent routing state by
/// encoding routing information in the type system. Unlike `Message`,
/// `TypedMessage` guarantees at compile time that sender/recipient are
/// present when required.
///
/// # Examples
///
/// ```
/// use skreaver_mesh::TypedMessage;
///
/// // Unicast - both sender and recipient guaranteed
/// let msg = TypedMessage::with_payload("hello")
///     .unicast("agent-1", "agent-2");
/// let sender = msg.sender(); // &AgentId - no Option!
/// let recipient = msg.recipient(); // &AgentId - no Option!
///
/// // Broadcast - only sender guaranteed
/// let broadcast = TypedMessage::with_payload("announcement")
///     .broadcast("coordinator");
/// let from = broadcast.sender(); // &AgentId
/// // broadcast.recipient(); // Compile error - no recipient!
/// ```
pub struct TypedMessage<R> {
    id: MessageId,
    route: Route,
    payload: MessagePayload,
    metadata: MessageMetadata,
    timestamp: DateTime<Utc>,
    correlation_id: Option<String>,
    _phantom: PhantomData<R>,
}

impl TypedMessage<Unrouted> {
    /// Start building a message (routing must be specified)
    pub fn with_payload(payload: impl Into<MessagePayload>) -> Self {
        Self {
            id: MessageId::new(),
            route: Route::Anonymous, // Will be overwritten
            payload: payload.into(),
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            correlation_id: None,
            _phantom: PhantomData,
        }
    }

    /// Convert to unicast message
    pub fn unicast(
        self,
        from: impl Into<AgentId>,
        to: impl Into<AgentId>,
    ) -> TypedMessage<UnicastRoute> {
        TypedMessage {
            id: self.id,
            route: Route::Unicast {
                from: from.into(),
                to: to.into(),
            },
            payload: self.payload,
            metadata: self.metadata,
            timestamp: self.timestamp,
            correlation_id: self.correlation_id,
            _phantom: PhantomData,
        }
    }

    /// Convert to broadcast message
    pub fn broadcast(self, from: impl Into<AgentId>) -> TypedMessage<BroadcastRoute> {
        TypedMessage {
            id: self.id,
            route: Route::Broadcast { from: from.into() },
            payload: self.payload,
            metadata: self.metadata,
            timestamp: self.timestamp,
            correlation_id: self.correlation_id,
            _phantom: PhantomData,
        }
    }

    /// Convert to system message
    pub fn system(self, to: impl Into<AgentId>) -> TypedMessage<SystemRoute> {
        TypedMessage {
            id: self.id,
            route: Route::System { to: to.into() },
            payload: self.payload,
            metadata: self.metadata,
            timestamp: self.timestamp,
            correlation_id: self.correlation_id,
            _phantom: PhantomData,
        }
    }

    /// Convert to anonymous message (rare, for infrastructure)
    pub fn anonymous(self) -> TypedMessage<AnonymousRoute> {
        TypedMessage {
            id: self.id,
            route: Route::Anonymous,
            payload: self.payload,
            metadata: self.metadata,
            timestamp: self.timestamp,
            correlation_id: self.correlation_id,
            _phantom: PhantomData,
        }
    }
}

// Common builder methods available for all routing states
impl<R> TypedMessage<R> {
    /// Add metadata to the message
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set correlation ID for request/reply pattern
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Get the route (available on all typed messages)
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// Get message ID
    pub fn id(&self) -> &MessageId {
        &self.id
    }

    /// Get metadata value by key
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

// Specialized methods for unicast messages
impl TypedMessage<UnicastRoute> {
    /// Get the sender (guaranteed present for unicast)
    pub fn sender(&self) -> &AgentId {
        match &self.route {
            Route::Unicast { from, .. } => from,
            _ => unreachable!("Typestate guarantees unicast route"),
        }
    }

    /// Get the recipient (guaranteed present for unicast)
    pub fn recipient(&self) -> &AgentId {
        match &self.route {
            Route::Unicast { to, .. } => to,
            _ => unreachable!("Typestate guarantees unicast route"),
        }
    }
}

// Specialized methods for broadcast messages
impl TypedMessage<BroadcastRoute> {
    /// Get the sender (guaranteed present for broadcast)
    pub fn sender(&self) -> &AgentId {
        match &self.route {
            Route::Broadcast { from } => from,
            _ => unreachable!("Typestate guarantees broadcast route"),
        }
    }
}

// Specialized methods for system messages
impl TypedMessage<SystemRoute> {
    /// Get the recipient (guaranteed present for system messages)
    pub fn recipient(&self) -> &AgentId {
        match &self.route {
            Route::System { to } => to,
            _ => unreachable!("Typestate guarantees system route"),
        }
    }
}

// Conversions from TypedMessage to Message for backward compatibility
impl<R> From<TypedMessage<R>> for Message {
    fn from(typed: TypedMessage<R>) -> Self {
        let from = typed.route.sender().cloned();
        let to = typed.route.recipient().cloned();

        #[allow(deprecated)]
        Self {
            id: typed.id,
            route: typed.route,
            from,
            to,
            payload: typed.payload,
            metadata: typed.metadata,
            timestamp: typed.timestamp,
            correlation_id: typed.correlation_id,
        }
    }
}

// Conversions from Message to TypedMessage
impl From<Message> for TypedMessage<UnicastRoute> {
    fn from(msg: Message) -> Self {
        match msg.route {
            Route::Unicast { .. } => Self {
                id: msg.id,
                route: msg.route,
                payload: msg.payload,
                metadata: msg.metadata,
                timestamp: msg.timestamp,
                correlation_id: msg.correlation_id,
                _phantom: PhantomData,
            },
            _ => panic!("Cannot convert non-unicast message to TypedMessage<UnicastRoute>"),
        }
    }
}

impl From<Message> for TypedMessage<BroadcastRoute> {
    fn from(msg: Message) -> Self {
        match msg.route {
            Route::Broadcast { .. } => Self {
                id: msg.id,
                route: msg.route,
                payload: msg.payload,
                metadata: msg.metadata,
                timestamp: msg.timestamp,
                correlation_id: msg.correlation_id,
                _phantom: PhantomData,
            },
            _ => panic!("Cannot convert non-broadcast message to TypedMessage<BroadcastRoute>"),
        }
    }
}

impl From<Message> for TypedMessage<SystemRoute> {
    fn from(msg: Message) -> Self {
        match msg.route {
            Route::System { .. } => Self {
                id: msg.id,
                route: msg.route,
                payload: msg.payload,
                metadata: msg.metadata,
                timestamp: msg.timestamp,
                correlation_id: msg.correlation_id,
                _phantom: PhantomData,
            },
            _ => panic!("Cannot convert non-system message to TypedMessage<SystemRoute>"),
        }
    }
}

impl From<Message> for TypedMessage<AnonymousRoute> {
    fn from(msg: Message) -> Self {
        match msg.route {
            Route::Anonymous => Self {
                id: msg.id,
                route: msg.route,
                payload: msg.payload,
                metadata: msg.metadata,
                timestamp: msg.timestamp,
                correlation_id: msg.correlation_id,
                _phantom: PhantomData,
            },
            _ => panic!("Cannot convert non-anonymous message to TypedMessage<AnonymousRoute>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::new("hello");
        assert!(matches!(msg.payload, MessagePayload::Text(_)));
        assert!(msg.is_anonymous());
        assert_eq!(msg.sender(), None);
        assert_eq!(msg.recipient(), None);
    }

    #[test]
    fn test_message_builder_new_api() {
        let msg = MessageBuilder::unicast("agent-1", "agent-2", "test")
            .with_metadata("priority", "high")
            .with_correlation_id("req-123")
            .build();

        assert!(msg.is_unicast());
        assert_eq!(msg.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(msg.recipient().map(|a| a.as_str()), Some("agent-2"));
        assert_eq!(msg.get_metadata("priority"), Some("high"));
        assert_eq!(msg.correlation_id.as_deref(), Some("req-123"));
    }

    #[test]
    #[allow(deprecated)]
    fn test_message_builder_backward_compat() {
        let msg = MessageBuilder::new("test")
            .from("agent-1")
            .to("agent-2")
            .with_metadata("priority", "high")
            .with_correlation_id("req-123")
            .build();

        assert!(msg.is_unicast());
        assert_eq!(msg.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(msg.recipient().map(|a| a.as_str()), Some("agent-2"));
        assert_eq!(msg.get_metadata("priority"), Some("high"));
        assert_eq!(msg.correlation_id.as_deref(), Some("req-123"));
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::unicast("agent-1", "agent-2", "test payload");

        let json = msg.to_json().unwrap();
        let deserialized = Message::from_json(&json).unwrap();

        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.route, deserialized.route);
    }

    #[test]
    fn test_message_payload_types() {
        let text_msg = Message::new("text");
        assert!(matches!(text_msg.payload, MessagePayload::Text(_)));

        let json_msg = Message::new(serde_json::json!({"key": "value"}));
        assert!(matches!(json_msg.payload, MessagePayload::Json(_)));

        let binary_msg = Message::new(vec![1u8, 2, 3]);
        assert!(matches!(binary_msg.payload, MessagePayload::Binary(_)));
    }

    #[test]
    fn test_message_routing_patterns() {
        // Unicast: from agent to agent
        let unicast = Message::unicast("agent-1", "agent-2", "test");
        assert!(unicast.is_unicast());
        assert!(!unicast.is_broadcast());
        assert!(!unicast.is_system());
        assert!(!unicast.is_anonymous());
        assert_eq!(unicast.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(unicast.recipient().map(|a| a.as_str()), Some("agent-2"));

        // Broadcast: from agent to all
        let broadcast = Message::broadcast("agent-1", "announcement");
        assert!(!broadcast.is_unicast());
        assert!(broadcast.is_broadcast());
        assert!(!broadcast.is_system());
        assert!(!broadcast.is_anonymous());
        assert_eq!(broadcast.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(broadcast.recipient(), None);

        // System: to agent, no sender
        let system = Message::system("agent-1", "config update");
        assert!(!system.is_unicast());
        assert!(!system.is_broadcast());
        assert!(system.is_system());
        assert!(!system.is_anonymous());
        assert_eq!(system.sender(), None);
        assert_eq!(system.recipient().map(|a| a.as_str()), Some("agent-1"));

        // Anonymous: no sender, no recipient
        let anonymous = Message::new("infrastructure message");
        assert!(!anonymous.is_unicast());
        assert!(!anonymous.is_broadcast());
        assert!(!anonymous.is_system());
        assert!(anonymous.is_anonymous());
        assert_eq!(anonymous.sender(), None);
        assert_eq!(anonymous.recipient(), None);
    }

    #[test]
    #[allow(deprecated)]
    fn test_message_routing_patterns_backward_compat() {
        // Unicast: from agent to agent (using deprecated API)
        let unicast = Message::new("test").from("agent-1").to("agent-2");
        assert!(unicast.is_unicast());
        assert_eq!(unicast.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(unicast.recipient().map(|a| a.as_str()), Some("agent-2"));

        // Broadcast: from agent to all
        let broadcast = Message::new("announcement").from("agent-1");
        assert!(broadcast.is_broadcast());
        assert_eq!(broadcast.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(broadcast.recipient(), None);

        // System: to agent, no sender
        let system = Message::new("config update").to("agent-1");
        assert!(system.is_system());
        assert_eq!(system.sender(), None);
        assert_eq!(system.recipient().map(|a| a.as_str()), Some("agent-1"));
    }

    #[test]
    fn test_route_helpers() {
        let route = Route::unicast("agent-1", "agent-2");
        assert!(route.has_sender());
        assert!(route.has_recipient());
        assert_eq!(route.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(route.recipient().map(|a| a.as_str()), Some("agent-2"));

        let broadcast = Route::broadcast("agent-1");
        assert!(broadcast.has_sender());
        assert!(!broadcast.has_recipient());

        let system = Route::system("agent-1");
        assert!(!system.has_sender());
        assert!(system.has_recipient());

        let anonymous = Route::anonymous();
        assert!(!anonymous.has_sender());
        assert!(!anonymous.has_recipient());
    }

    #[test]
    fn test_typed_message_unicast() {
        let msg = TypedMessage::with_payload("test")
            .unicast("sender", "receiver")
            .with_metadata("key", "value");

        // Guaranteed methods - no Option unwrapping needed
        assert_eq!(msg.sender().as_str(), "sender");
        assert_eq!(msg.recipient().as_str(), "receiver");

        // Convert to Message for backward compatibility
        let old_msg: Message = msg.into();
        assert!(old_msg.is_unicast());
    }

    #[test]
    fn test_typed_message_broadcast() {
        let msg = TypedMessage::with_payload("announce").broadcast("announcer");

        // Only sender available, no recipient method exists
        assert_eq!(msg.sender().as_str(), "announcer");

        let old_msg: Message = msg.into();
        assert!(old_msg.is_broadcast());
    }

    #[test]
    fn test_typed_message_system() {
        let msg = TypedMessage::with_payload("config").system("agent-1");

        // Only recipient available, no sender method exists
        assert_eq!(msg.recipient().as_str(), "agent-1");

        let old_msg: Message = msg.into();
        assert!(old_msg.is_system());
    }

    #[test]
    fn test_payload_serialization_roundtrip() {
        // Test that each payload type survives serialization round-trip

        // Text payload
        let text = MessagePayload::Text("hello world".to_string());
        let json = serde_json::to_string(&text).unwrap();
        assert_eq!(json, r#"{"type":"text","data":"hello world"}"#);
        let deserialized: MessagePayload = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, MessagePayload::Text(_)));

        // JSON payload
        let json_payload = MessagePayload::Json(serde_json::json!({"key": "value"}));
        let json = serde_json::to_string(&json_payload).unwrap();
        assert!(json.contains(r#""type":"json""#));
        let deserialized: MessagePayload = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, MessagePayload::Json(_)));

        // Binary payload - now properly tagged to prevent Text deserialization
        let binary = MessagePayload::Binary(vec![1, 2, 3, 4, 5]);
        let json = serde_json::to_string(&binary).unwrap();
        assert_eq!(json, r#"{"type":"binary","data":"AQIDBAU="}"#);
        let deserialized: MessagePayload = serde_json::from_str(&json).unwrap();

        // With explicit tagging, binary data is correctly preserved!
        match deserialized {
            MessagePayload::Binary(data) => {
                assert_eq!(data, vec![1, 2, 3, 4, 5]);
            }
            MessagePayload::Text(s) => {
                panic!("Binary payload was deserialized as Text: {}", s);
            }
            MessagePayload::Json(_) => {
                panic!("Binary payload was deserialized as Json");
            }
        }
    }

    #[test]
    fn test_payload_format_examples() {
        // Document the new tagged format for each payload type

        // Text: {"type":"text","data":"..."}
        let text = MessagePayload::Text("hello".to_string());
        let json = serde_json::to_value(&text).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["data"], "hello");

        // Json: {"type":"json","data":{...}}
        let json_payload = MessagePayload::Json(serde_json::json!({"x": 42}));
        let json = serde_json::to_value(&json_payload).unwrap();
        assert_eq!(json["type"], "json");
        assert_eq!(json["data"]["x"], 42);

        // Binary: {"type":"binary","data":"base64..."}
        let binary = MessagePayload::Binary(vec![255, 0, 128]);
        let json = serde_json::to_value(&binary).unwrap();
        assert_eq!(json["type"], "binary");
        assert_eq!(json["data"], "/wCA"); // base64 of [255, 0, 128]
    }

    #[test]
    fn test_message_id_generation() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();

        // IDs should be different
        assert_ne!(id1.as_str(), id2.as_str());

        // Should be valid UUIDs
        assert!(MessageId::parse(id1.as_str()).is_ok());
        assert!(MessageId::parse(id2.as_str()).is_ok());
    }

    #[test]
    fn test_message_id_parse_valid() {
        // Standard UUID v4
        let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";
        let id = MessageId::parse(valid_uuid).unwrap();
        assert_eq!(id.as_str(), valid_uuid);

        // Uppercase UUID
        let uppercase = "550E8400-E29B-41D4-A716-446655440000";
        assert!(MessageId::parse(uppercase).is_ok());
    }

    #[test]
    fn test_message_id_parse_invalid() {
        // Empty string
        assert!(MessageId::parse("").is_err());

        // Not a UUID
        assert!(MessageId::parse("not-a-uuid").is_err());

        // Partial UUID
        assert!(MessageId::parse("550e8400-e29b").is_err());

        // Invalid characters
        assert!(MessageId::parse("xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx").is_err());

        // Too short
        assert!(MessageId::parse("550e8400").is_err());

        // Random string
        assert!(MessageId::parse("hello-world").is_err());
    }

    #[test]
    #[should_panic(expected = "Invalid MessageId")]
    fn test_message_id_from_string_panics() {
        MessageId::from_string("not-a-uuid".to_string());
    }

    #[test]
    fn test_message_id_error_display() {
        let err = MessageIdError::InvalidFormat("bad-id".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Invalid MessageId format"));
        assert!(msg.contains("bad-id"));
    }
}
