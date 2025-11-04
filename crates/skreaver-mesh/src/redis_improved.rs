//! Improved Redis mesh implementation with type-safe routing

use async_trait::async_trait;
use crate::{
    error::{MeshError, MeshResult},
    message::Route,
    types::AgentId,
};

/// Trait for messages that can be sent to a specific recipient
///
/// Only messages with routing that includes a recipient implement this trait.
/// This prevents accidentally sending broadcasts via the point-to-point API.
pub trait HasRecipient {
    fn recipient(&self) -> &AgentId;
}

/// Trait for messages that can be broadcast
///
/// Only messages with broadcast routing implement this trait.
pub trait Broadcastable {
    fn sender(&self) -> Option<&AgentId>;
}

/// Example of how to implement routing checks at compile time
///
/// This prevents the footgun where `mesh.send(agent, broadcast_msg)` would
/// silently convert a broadcast to a unicast.
pub trait TypeSafeAgentMesh {
    /// Send a message to a specific recipient
    ///
    /// Only accepts messages with Unicast or System routing.
    /// Attempting to send a Broadcast message will fail at compile time.
    async fn send<M: HasRecipient>(&self, message: M) -> MeshResult<()>;

    /// Broadcast a message to all agents
    ///
    /// Only accepts messages with Broadcast or Anonymous routing.
    async fn broadcast<M: Broadcastable>(&self, message: M) -> MeshResult<()>;
}

/// Alternative: Use an enum to make the routing decision explicit
pub enum SendTarget {
    /// Send to a specific agent (requires Unicast or System route)
    Agent(AgentId),
    /// Broadcast to all agents (requires Broadcast or Anonymous route)
    All,
}

/// Helper function to validate that a route matches the intended send target
pub fn validate_route_target(route: &Route, target: &SendTarget) -> MeshResult<()> {
    match (route, target) {
        // Valid combinations
        (Route::Unicast { to, .. }, SendTarget::Agent(expected_to)) if to == expected_to => Ok(()),
        (Route::System { to }, SendTarget::Agent(expected_to)) if to == expected_to => Ok(()),
        (Route::Broadcast { .. }, SendTarget::All) => Ok(()),
        (Route::Anonymous, SendTarget::All) => Ok(()),

        // Invalid combinations
        (Route::Unicast { to, .. }, SendTarget::Agent(expected_to)) => {
            Err(MeshError::InvalidConfig(format!(
                "Route specifies recipient '{}' but send() called with '{}'",
                to, expected_to
            )))
        }
        (Route::System { to }, SendTarget::Agent(expected_to)) => {
            Err(MeshError::InvalidConfig(format!(
                "Route specifies recipient '{}' but send() called with '{}'",
                to, expected_to
            )))
        }
        (Route::Broadcast { .. }, SendTarget::Agent(agent)) => {
            Err(MeshError::InvalidConfig(format!(
                "Cannot send broadcast message to specific agent '{}'",
                agent
            )))
        }
        (Route::Anonymous, SendTarget::Agent(agent)) => {
            Err(MeshError::InvalidConfig(format!(
                "Cannot send anonymous message to specific agent '{}'",
                agent
            )))
        }
        (Route::Unicast { .. }, SendTarget::All) => Err(MeshError::InvalidConfig(
            "Cannot broadcast unicast message".to_string(),
        )),
        (Route::System { .. }, SendTarget::All) => Err(MeshError::InvalidConfig(
            "Cannot broadcast system message".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Route;

    #[test]
    fn test_validate_route_target_unicast_to_agent() {
        let route = Route::unicast("sender", "receiver");
        let target = SendTarget::Agent(AgentId::new_unchecked("receiver"));
        assert!(validate_route_target(&route, &target).is_ok());
    }

    #[test]
    fn test_validate_route_target_unicast_wrong_recipient() {
        let route = Route::unicast("sender", "receiver1");
        let target = SendTarget::Agent(AgentId::new_unchecked("receiver2"));
        assert!(validate_route_target(&route, &target).is_err());
    }

    #[test]
    fn test_validate_route_target_broadcast_to_agent() {
        let route = Route::broadcast("sender");
        let target = SendTarget::Agent(AgentId::new_unchecked("receiver"));
        let result = validate_route_target(&route, &target);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("broadcast"));
    }

    #[test]
    fn test_validate_route_target_broadcast_to_all() {
        let route = Route::broadcast("sender");
        let target = SendTarget::All;
        assert!(validate_route_target(&route, &target).is_ok());
    }

    #[test]
    fn test_validate_route_target_unicast_to_all() {
        let route = Route::unicast("sender", "receiver");
        let target = SendTarget::All;
        let result = validate_route_target(&route, &target);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unicast"));
    }

    #[test]
    fn test_validate_route_target_system_to_agent() {
        let route = Route::system("receiver");
        let target = SendTarget::Agent(AgentId::new_unchecked("receiver"));
        assert!(validate_route_target(&route, &target).is_ok());
    }

    #[test]
    fn test_validate_route_target_anonymous_to_all() {
        let route = Route::anonymous();
        let target = SendTarget::All;
        assert!(validate_route_target(&route, &target).is_ok());
    }
}

/// Example: Improved RedisMesh implementation that validates routing
///
/// Instead of silently converting routes, this implementation validates
/// that the route matches the intended operation.
pub mod example {
    use super::*;
    use crate::message::Message;

    pub struct ImprovedRedisMesh {
        // ... Redis connection pool, etc.
    }

    impl ImprovedRedisMesh {
        /// Send a message to a specific agent
        ///
        /// Validates that the message route is compatible with point-to-point delivery.
        pub async fn send(&self, to: &AgentId, message: Message) -> MeshResult<()> {
            // Validate that the route matches the send target
            validate_route_target(message.route(), &SendTarget::Agent(to.clone()))?;

            // Now we know the route is valid, proceed with sending
            // ... actual Redis implementation ...

            Ok(())
        }

        /// Broadcast a message to all agents
        ///
        /// Validates that the message route is compatible with broadcast delivery.
        pub async fn broadcast(&self, message: Message) -> MeshResult<()> {
            // Validate that the route is broadcastable
            validate_route_target(message.route(), &SendTarget::All)?;

            // Now we know the route is valid, proceed with broadcasting
            // ... actual Redis implementation ...

            Ok(())
        }
    }
}
