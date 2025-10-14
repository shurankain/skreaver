//! Improved message routing with typestate pattern
//!
//! This design uses phantom types to enforce routing invariants at compile time,
//! eliminating the need for deprecated fields and preventing inconsistent state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::types::AgentId;
use crate::message::{MessageId, MessagePayload, MessageMetadata, Route};

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

/// Type-safe message builder using typestate pattern
///
/// This eliminates the possibility of inconsistent routing state by
/// encoding routing information in the type system.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typestate_prevents_missing_routing() {
        // This won't compile - must specify routing!
        // let msg = TypedMessage::with_payload("test");
        // mesh.send(msg); // ERROR: R is Unrouted

        // Must explicitly choose routing:
        let msg = TypedMessage::with_payload("test")
            .unicast("agent-1", "agent-2");

        // Now sender() and recipient() are guaranteed available
        assert_eq!(msg.sender().as_str(), "agent-1");
        assert_eq!(msg.recipient().as_str(), "agent-2");
    }

    #[test]
    fn test_unicast_guarantees() {
        let msg = TypedMessage::with_payload("test")
            .unicast("sender", "receiver");

        // These methods only exist on UnicastRoute - no Option unwrapping!
        let _sender = msg.sender(); // &AgentId, not Option<&AgentId>
        let _recipient = msg.recipient(); // &AgentId, not Option<&AgentId>
    }

    #[test]
    fn test_broadcast_has_no_recipient() {
        let msg = TypedMessage::with_payload("announce")
            .broadcast("announcer");

        let _sender = msg.sender(); // Available
        // msg.recipient(); // Does not compile! Broadcast has no recipient
    }

    #[test]
    fn test_system_has_no_sender() {
        let msg = TypedMessage::with_payload("config")
            .system("agent-1");

        let _recipient = msg.recipient(); // Available
        // msg.sender(); // Does not compile! System has no sender
    }
}
