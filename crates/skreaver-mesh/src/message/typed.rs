//! Type-safe message builder using the typestate pattern.
//!
//! This module provides `TypedMessage<R>` which uses phantom types to enforce
//! routing guarantees at compile time. Unlike the regular `Message` type,
//! `TypedMessage` ensures that sender/recipient information is only accessible
//! when the routing state guarantees their presence.
//!
//! # Examples
//!
//! ```
//! use skreaver_mesh::TypedMessage;
//!
//! // Unicast - both sender and recipient guaranteed
//! let msg = TypedMessage::with_payload("hello")
//!     .unicast("agent-1", "agent-2");
//! let sender = msg.sender(); // &AgentId - no Option!
//! let recipient = msg.recipient(); // &AgentId - no Option!
//!
//! // Broadcast - only sender guaranteed
//! let broadcast = TypedMessage::with_payload("announcement")
//!     .broadcast("coordinator");
//! let from = broadcast.sender(); // &AgentId
//! // broadcast.recipient(); // Compile error - no recipient!
//! ```

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::marker::PhantomData;

use super::core::Message;
use super::types::{
    AnonymousRoute, BroadcastRoute, MessageId, MessageMetadata, MessagePayload, Route, SystemRoute,
    UnicastRoute, Unrouted,
};
use crate::types::AgentId;

/// Type-safe message builder using typestate pattern
///
/// This eliminates the possibility of inconsistent routing state by
/// encoding routing information in the type system. Unlike `Message`,
/// `TypedMessage` guarantees at compile time that sender/recipient are
/// present when required.
pub struct TypedMessage<R> {
    id: MessageId,
    route: Route,
    payload: MessagePayload,
    metadata: MessageMetadata,
    timestamp: DateTime<Utc>,
    correlation_id: Option<String>,
    _phantom: PhantomData<R>,
}

// ============================================================================
// Unrouted State - Initial Construction
// ============================================================================

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
    pub fn unicast(self, from: AgentId, to: AgentId) -> TypedMessage<UnicastRoute> {
        TypedMessage {
            id: self.id,
            route: Route::Unicast { from, to },
            payload: self.payload,
            metadata: self.metadata,
            timestamp: self.timestamp,
            correlation_id: self.correlation_id,
            _phantom: PhantomData,
        }
    }

    /// Convert to broadcast message
    pub fn broadcast(self, from: AgentId) -> TypedMessage<BroadcastRoute> {
        TypedMessage {
            id: self.id,
            route: Route::Broadcast { from },
            payload: self.payload,
            metadata: self.metadata,
            timestamp: self.timestamp,
            correlation_id: self.correlation_id,
            _phantom: PhantomData,
        }
    }

    /// Convert to system message
    pub fn system(self, to: AgentId) -> TypedMessage<SystemRoute> {
        TypedMessage {
            id: self.id,
            route: Route::System { to },
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

// ============================================================================
// Common Methods - Available for All Routing States
// ============================================================================

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

// ============================================================================
// Unicast Route - Both Sender and Recipient Guaranteed
// ============================================================================

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

// ============================================================================
// Broadcast Route - Only Sender Guaranteed
// ============================================================================

impl TypedMessage<BroadcastRoute> {
    /// Get the sender (guaranteed present for broadcast)
    pub fn sender(&self) -> &AgentId {
        match &self.route {
            Route::Broadcast { from } => from,
            _ => unreachable!("Typestate guarantees broadcast route"),
        }
    }
}

// ============================================================================
// System Route - Only Recipient Guaranteed
// ============================================================================

impl TypedMessage<SystemRoute> {
    /// Get the recipient (guaranteed present for system messages)
    pub fn recipient(&self) -> &AgentId {
        match &self.route {
            Route::System { to } => to,
            _ => unreachable!("Typestate guarantees system route"),
        }
    }
}

// ============================================================================
// Conversions - TypedMessage <-> Message
// ============================================================================

impl<R> From<TypedMessage<R>> for Message {
    fn from(typed: TypedMessage<R>) -> Self {
        Self {
            id: typed.id,
            route: typed.route,
            payload: typed.payload,
            metadata: typed.metadata,
            timestamp: typed.timestamp,
            correlation_id: typed.correlation_id,
        }
    }
}

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
