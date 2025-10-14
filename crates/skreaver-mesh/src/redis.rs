//! Redis-based implementation of AgentMesh

use async_trait::async_trait;
use futures::StreamExt;
use redis::AsyncCommands;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

use crate::{
    error::{MeshError, MeshResult},
    mesh::{AgentMesh, MessageStream},
    message::{Message, Route},
    types::{AgentId, Topic},
};

/// Validates that a message route is compatible with the send operation
fn validate_send_route(route: &Route, to: &AgentId) -> MeshResult<()> {
    match route {
        Route::Unicast { to: route_to, .. } => {
            if route_to != to {
                Err(MeshError::InvalidConfig(format!(
                    "Route specifies recipient '{}' but send() called with '{}'",
                    route_to, to
                )))
            } else {
                Ok(())
            }
        }
        Route::System { to: route_to } => {
            if route_to != to {
                Err(MeshError::InvalidConfig(format!(
                    "Route specifies recipient '{}' but send() called with '{}'",
                    route_to, to
                )))
            } else {
                Ok(())
            }
        }
        Route::Broadcast { from } => Err(MeshError::InvalidConfig(format!(
            "Cannot send broadcast message from '{}' to specific agent '{}'. Use broadcast() instead",
            from, to
        ))),
        Route::Anonymous => Err(MeshError::InvalidConfig(format!(
            "Cannot send anonymous message to specific agent '{}'. Anonymous messages should use broadcast()",
            to
        ))),
    }
}

/// Validates that a message route is compatible with the broadcast operation
fn validate_broadcast_route(route: &Route) -> MeshResult<()> {
    match route {
        Route::Broadcast { .. } | Route::Anonymous => Ok(()),
        Route::Unicast { from, to } => Err(MeshError::InvalidConfig(format!(
            "Cannot broadcast unicast message from '{}' to '{}'. Use send() instead",
            from, to
        ))),
        Route::System { to } => Err(MeshError::InvalidConfig(format!(
            "Cannot broadcast system message to '{}'. Use send() instead",
            to
        ))),
    }
}

/// Redis connection configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis connection URL (e.g., "redis://localhost:6379")
    pub url: String,
    /// Maximum number of connections in the pool
    pub pool_size: usize,
    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,
    /// Command timeout in seconds
    pub command_timeout_secs: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
            connect_timeout_secs: 5,
            command_timeout_secs: 3,
        }
    }
}

impl RedisConfig {
    /// Create a new Redis configuration
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set the pool size
    pub fn with_pool_size(mut self, pool_size: usize) -> Self {
        self.pool_size = pool_size;
        self
    }

    /// Set connection timeout
    pub fn with_connect_timeout(mut self, secs: u64) -> Self {
        self.connect_timeout_secs = secs;
        self
    }

    /// Set command timeout
    pub fn with_command_timeout(mut self, secs: u64) -> Self {
        self.command_timeout_secs = secs;
        self
    }
}

/// Redis-based agent mesh implementation
pub struct RedisMesh {
    pool: deadpool_redis::Pool,
    config: RedisConfig,
    /// Active subscriptions (topic -> subscription handle)
    subscriptions: Arc<RwLock<std::collections::HashMap<Topic, tokio::task::JoinHandle<()>>>>,
}

impl RedisMesh {
    /// Create a new Redis mesh with default configuration
    pub async fn new(url: impl Into<String>) -> MeshResult<Self> {
        let config = RedisConfig::new(url);
        Self::with_config(config).await
    }

    /// Create a new Redis mesh with custom configuration
    pub async fn with_config(config: RedisConfig) -> MeshResult<Self> {
        // Create Redis pool configuration
        let redis_config = deadpool_redis::Config::from_url(&config.url);

        let pool = redis_config
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .map_err(|e| MeshError::ConnectionFailed(e.to_string()))?;

        // Test connection
        let mut conn = pool
            .get()
            .await
            .map_err(|e| MeshError::ConnectionFailed(e.to_string()))?;

        redis::cmd("PING")
            .query_async::<String>(&mut *conn)
            .await
            .map_err(|e| MeshError::ConnectionFailed(format!("PING failed: {}", e)))?;

        debug!("Redis mesh connected to {}", config.url);

        Ok(Self {
            pool,
            config,
            subscriptions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Get a connection from the pool
    async fn get_connection(&self) -> MeshResult<deadpool_redis::Connection> {
        self.pool
            .get()
            .await
            .map_err(|e| MeshError::ConnectionFailed(e.to_string()))
    }

    /// Build Redis key for agent mailbox
    fn agent_key(agent_id: &AgentId) -> String {
        format!("skreaver:agent:{}:mailbox", agent_id)
    }

    /// Build Redis key for broadcast channel
    fn broadcast_key() -> String {
        "skreaver:broadcast".to_string()
    }

    /// Build Redis key for topic
    fn topic_key(topic: &Topic) -> String {
        format!("skreaver:topic:{}", topic)
    }

    /// Build Redis key for agent presence
    fn presence_key(agent_id: &AgentId) -> String {
        format!("skreaver:presence:{}", agent_id)
    }

    /// Build Redis key for agent list
    fn agents_set_key() -> String {
        "skreaver:agents".to_string()
    }
}

#[async_trait]
impl AgentMesh for RedisMesh {
    async fn send(&self, to: &AgentId, message: Message) -> MeshResult<()> {
        // Validate that the route is compatible with send operation
        validate_send_route(&message.route, to)?;

        // Serialize message
        let json = message.to_json()?;

        // Push to agent's mailbox (Redis list)
        let mut conn = self.get_connection().await?;
        let key = Self::agent_key(to);

        conn.lpush::<_, _, ()>(&key, json)
            .await
            .map_err(|e| MeshError::SendFailed(e.to_string()))?;

        debug!("Sent message {} to agent {}", message.id, to);
        Ok(())
    }

    async fn broadcast(&self, message: Message) -> MeshResult<()> {
        // Validate that the route is compatible with broadcast operation
        validate_broadcast_route(&message.route)?;

        // Serialize message
        let json = message.to_json()?;

        // Publish to broadcast channel
        let mut conn = self.get_connection().await?;
        let channel = Self::broadcast_key();

        conn.publish::<_, _, ()>(&channel, json)
            .await
            .map_err(|e| MeshError::SendFailed(e.to_string()))?;

        debug!("Broadcast message {}", message.id);
        Ok(())
    }

    async fn subscribe(&self, topic: &Topic) -> MeshResult<MessageStream> {
        // For pub/sub we need a dedicated connection
        let client = redis::Client::open(self.config.url.as_str())
            .map_err(|e| MeshError::ConnectionFailed(e.to_string()))?;

        let channel = Self::topic_key(topic);

        // Create Redis pub/sub connection
        let mut pubsub = client
            .get_async_pubsub()
            .await
            .map_err(|e| MeshError::ConnectionFailed(e.to_string()))?;

        pubsub
            .subscribe(&channel)
            .await
            .map_err(|e| MeshError::SubscribeFailed(e.to_string()))?;

        debug!("Subscribed to topic {}", topic);

        // Convert Redis message stream to our message stream
        let stream = pubsub.into_on_message().map(|msg| {
            let payload: String = msg.get_payload().map_err(|e| {
                error!("Failed to get message payload: {}", e);
                MeshError::DeserializationFailed(e.to_string())
            })?;

            Message::from_json(&payload).map_err(|e| {
                error!("Failed to deserialize message: {}", e);
                MeshError::DeserializationFailed(e.to_string())
            })
        });

        Ok(Box::pin(stream))
    }

    async fn publish(&self, topic: &Topic, message: Message) -> MeshResult<()> {
        // Validate that the route is compatible with broadcast/publish operation
        validate_broadcast_route(&message.route)?;

        // Serialize message
        let json = message.to_json()?;

        // Publish to topic channel
        let mut conn = self.get_connection().await?;
        let channel = Self::topic_key(topic);

        conn.publish::<_, _, ()>(&channel, json)
            .await
            .map_err(|e| MeshError::SendFailed(e.to_string()))?;

        debug!("Published message {} to topic {}", message.id, topic);
        Ok(())
    }

    async fn unsubscribe(&self, topic: &Topic) -> MeshResult<()> {
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(handle) = subscriptions.remove(topic) {
            handle.abort();
            debug!("Unsubscribed from topic {}", topic);
        } else {
            warn!(
                "Attempted to unsubscribe from topic {} but no subscription found",
                topic
            );
        }

        Ok(())
    }

    async fn queue_depth(&self) -> MeshResult<usize> {
        // Get total messages across all agent mailboxes
        let mut conn = self.get_connection().await?;

        // This is a simple implementation - in production you'd want to track
        // specific agent queues or use Redis SCAN for better performance
        let pattern = "skreaver:agent:*:mailbox";
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| MeshError::BackendError(e.to_string()))?;

        let mut total = 0;
        for key in keys {
            let len: usize = conn
                .llen(&key)
                .await
                .map_err(|e| MeshError::BackendError(e.to_string()))?;
            total += len;
        }

        Ok(total)
    }

    async fn is_reachable(&self, agent_id: &AgentId) -> bool {
        let mut conn = match self.get_connection().await {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to get connection: {}", e);
                return false;
            }
        };

        let key = Self::presence_key(agent_id);

        // Check if agent has presence key (with TTL)
        conn.exists::<_, bool>(&key).await.unwrap_or(false)
    }

    async fn list_agents(&self) -> MeshResult<Vec<AgentId>> {
        let mut conn = self.get_connection().await?;
        let key = Self::agents_set_key();

        // Get all agent IDs from the set
        let agent_ids: Vec<String> = conn
            .smembers(&key)
            .await
            .map_err(|e| MeshError::BackendError(e.to_string()))?;

        Ok(agent_ids.into_iter().map(AgentId::from).collect())
    }
}

impl RedisMesh {
    /// Register an agent as present in the mesh
    ///
    /// This sets a presence key with TTL. Agents should periodically
    /// refresh their presence to stay active.
    pub async fn register_presence(&self, agent_id: &AgentId, ttl_secs: u64) -> MeshResult<()> {
        let mut conn = self.get_connection().await?;
        let presence_key = Self::presence_key(agent_id);
        let agents_key = Self::agents_set_key();

        // Set presence with TTL
        conn.set_ex::<_, _, ()>(&presence_key, "1", ttl_secs)
            .await
            .map_err(|e| MeshError::BackendError(e.to_string()))?;

        // Add to agents set
        conn.sadd::<_, _, ()>(&agents_key, agent_id.as_str())
            .await
            .map_err(|e| MeshError::BackendError(e.to_string()))?;

        debug!(
            "Registered presence for agent {} (TTL: {}s)",
            agent_id, ttl_secs
        );
        Ok(())
    }

    /// Deregister an agent from the mesh
    pub async fn deregister_presence(&self, agent_id: &AgentId) -> MeshResult<()> {
        let mut conn = self.get_connection().await?;
        let presence_key = Self::presence_key(agent_id);
        let agents_key = Self::agents_set_key();

        // Remove presence key
        conn.del::<_, ()>(&presence_key)
            .await
            .map_err(|e| MeshError::BackendError(e.to_string()))?;

        // Remove from agents set
        conn.srem::<_, _, ()>(&agents_key, agent_id.as_str())
            .await
            .map_err(|e| MeshError::BackendError(e.to_string()))?;

        debug!("Deregistered presence for agent {}", agent_id);
        Ok(())
    }

    /// Receive messages from agent's mailbox
    ///
    /// This pops messages from the agent's mailbox (blocking with timeout).
    pub async fn receive(
        &self,
        agent_id: &AgentId,
        timeout_secs: u64,
    ) -> MeshResult<Option<Message>> {
        let mut conn = self.get_connection().await?;
        let key = Self::agent_key(agent_id);

        // BRPOP: blocking right pop with timeout
        let result: Option<(String, String)> = conn
            .brpop(&key, timeout_secs as f64)
            .await
            .map_err(|e| MeshError::ReceiveFailed(e.to_string()))?;

        match result {
            Some((_key, json)) => {
                let message = Message::from_json(&json)?;
                debug!("Received message {} for agent {}", message.id, agent_id);
                Ok(Some(message))
            }
            None => Ok(None), // Timeout
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_config() {
        let config = RedisConfig::new("redis://localhost:6379")
            .with_pool_size(20)
            .with_connect_timeout(10);

        assert_eq!(config.pool_size, 20);
        assert_eq!(config.connect_timeout_secs, 10);
    }

    #[test]
    fn test_key_generation() {
        let agent_id = AgentId::from("agent-1");
        let key = RedisMesh::agent_key(&agent_id);
        assert_eq!(key, "skreaver:agent:agent-1:mailbox");

        let topic = Topic::from("notifications");
        let topic_key = RedisMesh::topic_key(&topic);
        assert_eq!(topic_key, "skreaver:topic:notifications");
    }

    #[test]
    fn test_validate_send_route_unicast_valid() {
        let route = Route::unicast("sender", "recipient");
        let to = AgentId::from("recipient");
        assert!(validate_send_route(&route, &to).is_ok());
    }

    #[test]
    fn test_validate_send_route_unicast_mismatch() {
        let route = Route::unicast("sender", "recipient");
        let to = AgentId::from("wrong-recipient");
        let result = validate_send_route(&route, &to);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Route specifies recipient"));
    }

    #[test]
    fn test_validate_send_route_broadcast_invalid() {
        let route = Route::broadcast("sender");
        let to = AgentId::from("recipient");
        let result = validate_send_route(&route, &to);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot send broadcast message"));
    }

    #[test]
    fn test_validate_send_route_anonymous_invalid() {
        let route = Route::anonymous();
        let to = AgentId::from("recipient");
        let result = validate_send_route(&route, &to);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot send anonymous message"));
    }

    #[test]
    fn test_validate_send_route_system_valid() {
        let route = Route::system("recipient");
        let to = AgentId::from("recipient");
        assert!(validate_send_route(&route, &to).is_ok());
    }

    #[test]
    fn test_validate_send_route_system_mismatch() {
        let route = Route::system("recipient");
        let to = AgentId::from("wrong-recipient");
        let result = validate_send_route(&route, &to);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_broadcast_route_broadcast_valid() {
        let route = Route::broadcast("sender");
        assert!(validate_broadcast_route(&route).is_ok());
    }

    #[test]
    fn test_validate_broadcast_route_anonymous_valid() {
        let route = Route::anonymous();
        assert!(validate_broadcast_route(&route).is_ok());
    }

    #[test]
    fn test_validate_broadcast_route_unicast_invalid() {
        let route = Route::unicast("sender", "recipient");
        let result = validate_broadcast_route(&route);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot broadcast unicast message"));
    }

    #[test]
    fn test_validate_broadcast_route_system_invalid() {
        let route = Route::system("recipient");
        let result = validate_broadcast_route(&route);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot broadcast system message"));
    }
}
