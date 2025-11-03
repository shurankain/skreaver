//! Message types and builders for agent communication.
//!
//! This module provides a comprehensive message system with:
//! - Type-safe routing through the `Route` enum
//! - Validated message IDs using UUIDs
//! - Multiple payload types (text, JSON, binary)
//! - Typestate pattern for compile-time routing guarantees
//! - Fluent builder APIs
//!
//! # Message Types
//!
//! - `Message` - The main message type with runtime routing
//! - `TypedMessage<R>` - Compile-time routing guarantees via typestate pattern
//! - `MessageBuilder` - Fluent API for building messages
//!
//! # Routing
//!
//! Messages support four routing patterns through the `Route` enum:
//! - `Unicast` - Direct message from one agent to another
//! - `Broadcast` - Broadcast from an agent to all listeners
//! - `System` - System message to a specific agent
//! - `Anonymous` - System-wide broadcast (rare, for infrastructure)

// Module declarations
mod builder;
mod core;
mod typed;
mod types;

// Re-export all public types for backward compatibility
pub use builder::MessageBuilder;
pub use core::Message;
pub use typed::TypedMessage;
pub use types::{
    AnonymousRoute, BroadcastRoute, MessageId, MessageIdError, MessageMetadata, MessagePayload,
    Route, SystemRoute, UnicastRoute, Unrouted,
};

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
    fn test_message_builder_unicast() {
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
    fn test_message_routing_patterns_direct_constructors() {
        // Unicast: from agent to agent
        let unicast = Message::unicast("agent-1", "agent-2", "test");
        assert!(unicast.is_unicast());
        assert_eq!(unicast.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(unicast.recipient().map(|a| a.as_str()), Some("agent-2"));

        // Broadcast: from agent to all
        let broadcast = Message::broadcast("agent-1", "announcement");
        assert!(broadcast.is_broadcast());
        assert_eq!(broadcast.sender().map(|a| a.as_str()), Some("agent-1"));
        assert_eq!(broadcast.recipient(), None);

        // System: to agent, no sender
        let system = Message::system("agent-1", "config update");
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
