//! Connection Registry
//!
//! This module provides a registry for tracking active protocol connections,
//! enabling protocol routing and connection management.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::detection::Protocol;
use crate::error::{GatewayError, GatewayResult};

/// Connection metadata
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Unique connection identifier
    pub id: String,
    /// Protocol type for this connection
    pub protocol: Protocol,
    /// Remote endpoint (URL or address)
    pub endpoint: String,
    /// Connection state
    pub state: ConnectionState,
    /// When the connection was established
    pub connected_at: chrono::DateTime<chrono::Utc>,
    /// Last activity timestamp
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Additional connection metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ConnectionInfo {
    /// Create a new connection info
    pub fn new(id: impl Into<String>, protocol: Protocol, endpoint: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: id.into(),
            protocol,
            endpoint: endpoint.into(),
            state: ConnectionState::Connected,
            connected_at: now,
            last_activity: now,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the connection
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Update the last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = chrono::Utc::now();
    }

    /// Check if the connection is active
    pub fn is_active(&self) -> bool {
        matches!(self.state, ConnectionState::Connected)
    }

    /// Get connection duration
    pub fn duration(&self) -> chrono::Duration {
        chrono::Utc::now().signed_duration_since(self.connected_at)
    }

    /// Get time since last activity
    pub fn idle_duration(&self) -> chrono::Duration {
        chrono::Utc::now().signed_duration_since(self.last_activity)
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is active
    Connected,
    /// Connection is being established
    Connecting,
    /// Connection is being closed
    Disconnecting,
    /// Connection has been closed
    Disconnected,
    /// Connection failed
    Failed,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Connected => write!(f, "connected"),
            ConnectionState::Connecting => write!(f, "connecting"),
            ConnectionState::Disconnecting => write!(f, "disconnecting"),
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Failed => write!(f, "failed"),
        }
    }
}

/// Statistics for the connection registry
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
    /// Total number of connections
    pub total_connections: usize,
    /// Number of active connections
    pub active_connections: usize,
    /// Connections by protocol
    pub by_protocol: HashMap<String, usize>,
    /// Connections by state
    pub by_state: HashMap<String, usize>,
}

/// Registry for tracking active protocol connections
#[derive(Debug, Clone)]
pub struct ConnectionRegistry {
    /// Active connections indexed by ID
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    /// Connections indexed by endpoint for quick lookup
    by_endpoint: Arc<RwLock<HashMap<String, String>>>,
    /// Maximum allowed connections (0 = unlimited)
    max_connections: usize,
    /// Idle timeout in seconds (0 = no timeout)
    idle_timeout_secs: u64,
}

impl ConnectionRegistry {
    /// Create a new connection registry
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            by_endpoint: Arc::new(RwLock::new(HashMap::new())),
            max_connections: 0,
            idle_timeout_secs: 0,
        }
    }

    /// Set maximum number of connections
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// Set idle timeout in seconds
    pub fn with_idle_timeout(mut self, timeout_secs: u64) -> Self {
        self.idle_timeout_secs = timeout_secs;
        self
    }

    /// Register a new connection
    pub async fn register(&self, info: ConnectionInfo) -> GatewayResult<()> {
        let mut connections = self.connections.write().await;

        // Check max connections
        if self.max_connections > 0 && connections.len() >= self.max_connections {
            return Err(GatewayError::Internal(format!(
                "Maximum connections ({}) reached",
                self.max_connections
            )));
        }

        // Check for duplicate ID
        if connections.contains_key(&info.id) {
            return Err(GatewayError::ConnectionAlreadyExists(info.id.clone()));
        }

        let id = info.id.clone();
        let endpoint = info.endpoint.clone();

        debug!(id = %id, protocol = %info.protocol, endpoint = %endpoint, "Registering connection");

        connections.insert(id.clone(), info);

        // Also index by endpoint
        let mut by_endpoint = self.by_endpoint.write().await;
        by_endpoint.insert(endpoint, id.clone());

        info!(id = %id, "Connection registered");

        Ok(())
    }

    /// Unregister a connection
    pub async fn unregister(&self, connection_id: &str) -> GatewayResult<ConnectionInfo> {
        let mut connections = self.connections.write().await;

        let info = connections
            .remove(connection_id)
            .ok_or_else(|| GatewayError::ConnectionNotFound(connection_id.to_string()))?;

        // Also remove from endpoint index
        let mut by_endpoint = self.by_endpoint.write().await;
        by_endpoint.remove(&info.endpoint);

        info!(id = %connection_id, "Connection unregistered");

        Ok(info)
    }

    /// Get connection info by ID
    pub async fn get(&self, connection_id: &str) -> GatewayResult<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections
            .get(connection_id)
            .cloned()
            .ok_or_else(|| GatewayError::ConnectionNotFound(connection_id.to_string()))
    }

    /// Get connection by endpoint
    pub async fn get_by_endpoint(&self, endpoint: &str) -> Option<ConnectionInfo> {
        let by_endpoint = self.by_endpoint.read().await;
        if let Some(id) = by_endpoint.get(endpoint) {
            let connections = self.connections.read().await;
            return connections.get(id).cloned();
        }
        None
    }

    /// Update connection state
    pub async fn update_state(
        &self,
        connection_id: &str,
        state: ConnectionState,
    ) -> GatewayResult<()> {
        let mut connections = self.connections.write().await;

        let info = connections
            .get_mut(connection_id)
            .ok_or_else(|| GatewayError::ConnectionNotFound(connection_id.to_string()))?;

        debug!(id = %connection_id, old_state = %info.state, new_state = %state, "Updating connection state");

        info.state = state;
        info.touch();

        Ok(())
    }

    /// Touch a connection (update last activity)
    pub async fn touch(&self, connection_id: &str) -> GatewayResult<()> {
        let mut connections = self.connections.write().await;

        let info = connections
            .get_mut(connection_id)
            .ok_or_else(|| GatewayError::ConnectionNotFound(connection_id.to_string()))?;

        info.touch();
        Ok(())
    }

    /// List all connections
    pub async fn list(&self) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections.values().cloned().collect()
    }

    /// List connections by protocol
    pub async fn list_by_protocol(&self, protocol: Protocol) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections
            .values()
            .filter(|c| c.protocol == protocol)
            .cloned()
            .collect()
    }

    /// List active connections
    pub async fn list_active(&self) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections
            .values()
            .filter(|c| c.is_active())
            .cloned()
            .collect()
    }

    /// Get registry statistics
    pub async fn stats(&self) -> RegistryStats {
        let connections = self.connections.read().await;

        let mut by_protocol: HashMap<String, usize> = HashMap::new();
        let mut by_state: HashMap<String, usize> = HashMap::new();
        let mut active_count = 0;

        for conn in connections.values() {
            *by_protocol.entry(conn.protocol.to_string()).or_default() += 1;
            *by_state.entry(conn.state.to_string()).or_default() += 1;

            if conn.is_active() {
                active_count += 1;
            }
        }

        RegistryStats {
            total_connections: connections.len(),
            active_connections: active_count,
            by_protocol,
            by_state,
        }
    }

    /// Get number of active connections
    pub async fn active_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.values().filter(|c| c.is_active()).count()
    }

    /// Get total number of connections
    pub async fn total_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Clean up idle connections
    pub async fn cleanup_idle(&self) -> usize {
        if self.idle_timeout_secs == 0 {
            return 0;
        }

        let timeout = chrono::Duration::seconds(self.idle_timeout_secs as i64);
        let mut connections = self.connections.write().await;
        let mut by_endpoint = self.by_endpoint.write().await;

        let idle_ids: Vec<String> = connections
            .iter()
            .filter(|(_, c)| c.idle_duration() > timeout && c.is_active())
            .map(|(id, _)| id.clone())
            .collect();

        let count = idle_ids.len();

        for id in &idle_ids {
            if let Some(info) = connections.remove(id) {
                by_endpoint.remove(&info.endpoint);
                warn!(id = %id, idle_secs = ?info.idle_duration().num_seconds(), "Cleaned up idle connection");
            }
        }

        if count > 0 {
            info!(count, "Cleaned up idle connections");
        }

        count
    }

    /// Clean up disconnected connections
    pub async fn cleanup_disconnected(&self) -> usize {
        let mut connections = self.connections.write().await;
        let mut by_endpoint = self.by_endpoint.write().await;

        let disconnected_ids: Vec<String> = connections
            .iter()
            .filter(|(_, c)| {
                matches!(
                    c.state,
                    ConnectionState::Disconnected | ConnectionState::Failed
                )
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = disconnected_ids.len();

        for id in &disconnected_ids {
            if let Some(info) = connections.remove(id) {
                by_endpoint.remove(&info.endpoint);
                debug!(id = %id, state = %info.state, "Removed disconnected connection");
            }
        }

        if count > 0 {
            info!(count, "Cleaned up disconnected connections");
        }

        count
    }
}

impl Default for ConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_register_connection() {
        let registry = ConnectionRegistry::new();

        let info = ConnectionInfo::new("conn-1", Protocol::Mcp, "http://localhost:3000");

        registry.register(info).await.unwrap();

        let retrieved = registry.get("conn-1").await.unwrap();
        assert_eq!(retrieved.protocol, Protocol::Mcp);
        assert_eq!(retrieved.endpoint, "http://localhost:3000");
    }

    #[tokio::test]
    async fn test_duplicate_registration_fails() {
        let registry = ConnectionRegistry::new();

        let info1 = ConnectionInfo::new("conn-1", Protocol::Mcp, "http://localhost:3000");
        let info2 = ConnectionInfo::new("conn-1", Protocol::A2a, "http://localhost:3001");

        registry.register(info1).await.unwrap();
        assert!(registry.register(info2).await.is_err());
    }

    #[tokio::test]
    async fn test_unregister_connection() {
        let registry = ConnectionRegistry::new();

        let info = ConnectionInfo::new("conn-1", Protocol::A2a, "http://localhost:3000");
        registry.register(info).await.unwrap();

        let unregistered = registry.unregister("conn-1").await.unwrap();
        assert_eq!(unregistered.id, "conn-1");

        assert!(registry.get("conn-1").await.is_err());
    }

    #[tokio::test]
    async fn test_get_by_endpoint() {
        let registry = ConnectionRegistry::new();

        let info = ConnectionInfo::new("conn-1", Protocol::Mcp, "http://localhost:3000");
        registry.register(info).await.unwrap();

        let conn = registry
            .get_by_endpoint("http://localhost:3000")
            .await
            .unwrap();
        assert_eq!(conn.id, "conn-1");

        let not_found = registry.get_by_endpoint("http://localhost:9999").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_update_state() {
        let registry = ConnectionRegistry::new();

        let info = ConnectionInfo::new("conn-1", Protocol::Mcp, "http://localhost:3000");
        registry.register(info).await.unwrap();

        registry
            .update_state("conn-1", ConnectionState::Disconnecting)
            .await
            .unwrap();

        let conn = registry.get("conn-1").await.unwrap();
        assert_eq!(conn.state, ConnectionState::Disconnecting);
        assert!(!conn.is_active());
    }

    #[tokio::test]
    async fn test_list_by_protocol() {
        let registry = ConnectionRegistry::new();

        registry
            .register(ConnectionInfo::new(
                "mcp-1",
                Protocol::Mcp,
                "http://localhost:3001",
            ))
            .await
            .unwrap();
        registry
            .register(ConnectionInfo::new(
                "mcp-2",
                Protocol::Mcp,
                "http://localhost:3002",
            ))
            .await
            .unwrap();
        registry
            .register(ConnectionInfo::new(
                "a2a-1",
                Protocol::A2a,
                "http://localhost:3003",
            ))
            .await
            .unwrap();

        let mcp_conns = registry.list_by_protocol(Protocol::Mcp).await;
        assert_eq!(mcp_conns.len(), 2);

        let a2a_conns = registry.list_by_protocol(Protocol::A2a).await;
        assert_eq!(a2a_conns.len(), 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let registry = ConnectionRegistry::new();

        registry
            .register(ConnectionInfo::new(
                "conn-1",
                Protocol::Mcp,
                "http://localhost:3001",
            ))
            .await
            .unwrap();
        registry
            .register(ConnectionInfo::new(
                "conn-2",
                Protocol::A2a,
                "http://localhost:3002",
            ))
            .await
            .unwrap();

        let stats = registry.stats().await;
        assert_eq!(stats.total_connections, 2);
        assert_eq!(stats.active_connections, 2);
        assert_eq!(stats.by_protocol.get("MCP"), Some(&1));
        assert_eq!(stats.by_protocol.get("A2A"), Some(&1));
    }

    #[tokio::test]
    async fn test_max_connections() {
        let registry = ConnectionRegistry::new().with_max_connections(2);

        registry
            .register(ConnectionInfo::new(
                "conn-1",
                Protocol::Mcp,
                "http://localhost:3001",
            ))
            .await
            .unwrap();
        registry
            .register(ConnectionInfo::new(
                "conn-2",
                Protocol::Mcp,
                "http://localhost:3002",
            ))
            .await
            .unwrap();

        let result = registry
            .register(ConnectionInfo::new(
                "conn-3",
                Protocol::Mcp,
                "http://localhost:3003",
            ))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cleanup_disconnected() {
        let registry = ConnectionRegistry::new();

        registry
            .register(ConnectionInfo::new(
                "conn-1",
                Protocol::Mcp,
                "http://localhost:3001",
            ))
            .await
            .unwrap();
        registry
            .register(ConnectionInfo::new(
                "conn-2",
                Protocol::Mcp,
                "http://localhost:3002",
            ))
            .await
            .unwrap();

        registry
            .update_state("conn-1", ConnectionState::Disconnected)
            .await
            .unwrap();

        let cleaned = registry.cleanup_disconnected().await;
        assert_eq!(cleaned, 1);
        assert_eq!(registry.total_count().await, 1);
    }

    #[tokio::test]
    async fn test_connection_with_metadata() {
        let registry = ConnectionRegistry::new();

        let info = ConnectionInfo::new("conn-1", Protocol::Mcp, "http://localhost:3000")
            .with_metadata("agent_name", json!("TestAgent"))
            .with_metadata("version", json!("1.0.0"));

        registry.register(info).await.unwrap();

        let conn = registry.get("conn-1").await.unwrap();
        assert_eq!(conn.metadata.get("agent_name"), Some(&json!("TestAgent")));
        assert_eq!(conn.metadata.get("version"), Some(&json!("1.0.0")));
    }

    #[test]
    fn test_connection_state_display() {
        assert_eq!(ConnectionState::Connected.to_string(), "connected");
        assert_eq!(ConnectionState::Disconnected.to_string(), "disconnected");
        assert_eq!(ConnectionState::Failed.to_string(), "failed");
    }
}
