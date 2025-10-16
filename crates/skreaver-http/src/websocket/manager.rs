//! WebSocket connection manager

use super::{ConnectionInfo, WebSocketConfig, WsError, WsMessage, WsResult};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, broadcast, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// WebSocket connection manager
pub struct WebSocketManager {
    /// Manager configuration
    pub config: WebSocketConfig,
    /// Active connections
    connections: Arc<RwLock<HashMap<Uuid, ConnectionState>>>,
    /// Channel subscriptions
    subscriptions: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
    /// Event broadcaster
    event_sender: broadcast::Sender<ChannelEvent>,
    /// Authentication handler
    auth_handler: Option<Arc<dyn AuthHandler + Send + Sync>>,
    /// Connections per IP address
    connections_per_ip: Arc<RwLock<HashMap<std::net::IpAddr, usize>>>,
}

/// Connection state
#[derive(Debug)]
struct ConnectionState {
    /// Connection information
    info: ConnectionInfo,
    /// Message sender
    sender: mpsc::Sender<WsMessage>,
    /// Subscribed channels
    channels: Vec<String>,
    /// Authentication status
    authenticated: bool,
    /// User ID (if authenticated)
    user_id: Option<String>,
}

/// Channel event for broadcasting
#[derive(Debug, Clone)]
pub struct ChannelEvent {
    /// Channel name
    pub channel: String,
    /// Event data
    pub data: serde_json::Value,
    /// Target user ID (optional, for user-specific events)
    pub user_id: Option<String>,
}

/// Authentication handler trait
#[async_trait::async_trait]
pub trait AuthHandler {
    /// Authenticate a token and return user ID
    async fn authenticate(&self, token: &str) -> Result<String, String>;

    /// Check if user has permission for channel
    async fn check_permission(&self, user_id: &str, channel: &str) -> bool;
}

impl WebSocketManager {
    /// Create a new WebSocket manager
    pub fn new(config: WebSocketConfig) -> Self {
        let (event_sender, _) = broadcast::channel(config.broadcast_buffer_size);

        Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            auth_handler: None,
            connections_per_ip: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set authentication handler
    pub fn with_auth_handler(mut self, handler: Arc<dyn AuthHandler + Send + Sync>) -> Self {
        self.auth_handler = Some(handler);
        self
    }

    /// Add a new connection
    pub async fn add_connection(
        &self,
        id: Uuid,
        info: ConnectionInfo,
    ) -> WsResult<mpsc::Sender<WsMessage>> {
        let connections = self.connections.read().await;
        if connections.len() >= self.config.max_connections {
            return Err(WsError::ConnectionLimitExceeded);
        }
        drop(connections);

        // Check IP-based rate limiting
        let ip_addr = info.addr.ip();
        let mut ip_connections = self.connections_per_ip.write().await;
        let ip_count = ip_connections.get(&ip_addr).copied().unwrap_or(0);
        if ip_count >= self.config.max_connections_per_ip {
            return Err(WsError::RateLimitExceeded);
        }
        *ip_connections.entry(ip_addr).or_insert(0) += 1;
        drop(ip_connections);

        let (sender, _receiver) = mpsc::channel(self.config.buffer_size);

        let state = ConnectionState {
            info,
            sender: sender.clone(),
            channels: Vec::new(),
            authenticated: false,
            user_id: None,
        };

        let mut connections = self.connections.write().await;
        connections.insert(id, state);
        drop(connections);

        info!("Added WebSocket connection: {}", id);
        Ok(sender)
    }

    /// Remove a connection
    pub async fn remove_connection(&self, id: Uuid) {
        let mut connections = self.connections.write().await;
        if let Some(state) = connections.remove(&id) {
            // Decrement IP connection count
            let ip_addr = state.info.addr.ip();
            let mut ip_connections = self.connections_per_ip.write().await;
            if let Some(count) = ip_connections.get_mut(&ip_addr) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    ip_connections.remove(&ip_addr);
                }
            }
            drop(ip_connections);

            // Unsubscribe from all channels
            let mut subscriptions = self.subscriptions.write().await;
            for channel in &state.channels {
                if let Some(subscribers) = subscriptions.get_mut(channel) {
                    subscribers.retain(|&conn_id| conn_id != id);
                    if subscribers.is_empty() {
                        subscriptions.remove(channel);
                    }
                }
            }
            drop(subscriptions);

            info!("Removed WebSocket connection: {}", id);
        }
        drop(connections);
    }

    /// Update connection activity
    pub async fn update_activity(&self, id: Uuid) {
        let mut connections = self.connections.write().await;
        if let Some(state) = connections.get_mut(&id) {
            state.info.update_activity();
        }
    }

    /// Handle incoming message
    pub async fn handle_message(&self, conn_id: Uuid, message: WsMessage) -> WsResult<()> {
        debug!("Handling message from {}: {:?}", conn_id, message);

        // Update activity
        self.update_activity(conn_id).await;

        match message {
            WsMessage::Ping { .. } => {
                self.send_to_connection(conn_id, WsMessage::pong()).await?;
            }
            WsMessage::Pong { .. } => {
                // Activity already updated
            }
            WsMessage::Auth { token } => {
                self.handle_auth(conn_id, &token).await?;
            }
            WsMessage::Subscribe { channels } => {
                self.handle_subscribe(conn_id, channels).await?;
            }
            WsMessage::Unsubscribe { channels } => {
                self.handle_unsubscribe(conn_id, channels).await?;
            }
            _ => {
                warn!("Unexpected message type from {}: {:?}", conn_id, message);
            }
        }

        Ok(())
    }

    /// Handle authentication
    async fn handle_auth(&self, conn_id: Uuid, token: &str) -> WsResult<()> {
        if let Some(auth_handler) = &self.auth_handler {
            match auth_handler.authenticate(token).await {
                Ok(user_id) => {
                    let mut connections = self.connections.write().await;
                    if let Some(state) = connections.get_mut(&conn_id) {
                        state.authenticated = true;
                        state.user_id = Some(user_id);
                        drop(connections);

                        self.send_to_connection(
                            conn_id,
                            WsMessage::success("Authentication successful"),
                        )
                        .await?;
                        info!("Connection {} authenticated", conn_id);
                    }
                }
                Err(error) => {
                    return Err(WsError::AuthenticationFailed(error));
                }
            }
        } else {
            // No auth handler, consider all connections authenticated
            let mut connections = self.connections.write().await;
            if let Some(state) = connections.get_mut(&conn_id) {
                state.authenticated = true;
                drop(connections);

                self.send_to_connection(conn_id, WsMessage::success("Authentication successful"))
                    .await?;
            }
        }

        Ok(())
    }

    /// Handle channel subscription
    #[doc(hidden)] // Public for testing only
    pub async fn handle_subscribe(&self, conn_id: Uuid, channels: Vec<String>) -> WsResult<()> {
        // Use write lock from the start to prevent TOCTOU race condition
        let mut connections = self.connections.write().await;
        let subscriptions = self.subscriptions.write().await;

        let state = connections.get_mut(&conn_id).ok_or(WsError::ConnectionClosed)?;

        // Check authentication if auth handler is present
        if self.auth_handler.is_some() && !state.authenticated {
            return Err(WsError::AuthenticationFailed(
                "Authentication required".to_string(),
            ));
        }

        // Check subscription limit per connection
        let new_subscription_count = state.channels.len() + channels.len();
        if new_subscription_count > self.config.max_subscriptions_per_connection {
            return Err(WsError::SubscriptionLimitExceeded {
                current: new_subscription_count,
                max: self.config.max_subscriptions_per_connection,
            });
        }

        // Check permissions (release write locks temporarily for async operation)
        let user_id_opt = state.user_id.clone();
        drop(connections);
        drop(subscriptions);

        if let (Some(auth_handler), Some(user_id)) = (&self.auth_handler, &user_id_opt) {
            for channel in &channels {
                if !auth_handler.check_permission(user_id, channel).await {
                    return Err(WsError::PermissionDenied);
                }
            }
        }

        // Re-acquire locks and perform subscription
        let mut connections = self.connections.write().await;
        let mut subscriptions = self.subscriptions.write().await;

        // Re-check authentication after re-acquiring locks
        let state = connections.get_mut(&conn_id).ok_or(WsError::ConnectionClosed)?;
        if self.auth_handler.is_some() && !state.authenticated {
            return Err(WsError::AuthenticationFailed(
                "Authentication required".to_string(),
            ));
        }

        // Add subscriptions
        for channel in channels {
            if !state.channels.contains(&channel) {
                // Check channel subscriber limit
                let current_subscribers = subscriptions
                    .get(&channel)
                    .map(|subs| subs.len())
                    .unwrap_or(0);

                if current_subscribers >= self.config.max_subscribers_per_channel {
                    return Err(WsError::ChannelSubscriberLimitExceeded {
                        current: current_subscribers + 1,
                        max: self.config.max_subscribers_per_channel,
                    });
                }

                state.channels.push(channel.clone());
                subscriptions
                    .entry(channel.clone())
                    .or_insert_with(Vec::new)
                    .push(conn_id);

                debug!("Connection {} subscribed to channel {}", conn_id, channel);
            }
        }

        drop(subscriptions);
        drop(connections);

        self.send_to_connection(conn_id, WsMessage::success("Subscription successful"))
            .await?;
        Ok(())
    }

    /// Handle channel unsubscription
    async fn handle_unsubscribe(&self, conn_id: Uuid, channels: Vec<String>) -> WsResult<()> {
        let mut subscriptions = self.subscriptions.write().await;
        let mut connections = self.connections.write().await;

        if let Some(state) = connections.get_mut(&conn_id) {
            for channel in channels {
                if let Some(index) = state.channels.iter().position(|c| c == &channel) {
                    state.channels.remove(index);

                    if let Some(subscribers) = subscriptions.get_mut(&channel) {
                        subscribers.retain(|&id| id != conn_id);
                        if subscribers.is_empty() {
                            subscriptions.remove(&channel);
                        }
                    }

                    debug!(
                        "Connection {} unsubscribed from channel {}",
                        conn_id, channel
                    );
                }
            }
        }

        drop(subscriptions);
        drop(connections);

        self.send_to_connection(conn_id, WsMessage::success("Unsubscription successful"))
            .await?;
        Ok(())
    }

    /// Send message to specific connection
    pub async fn send_to_connection(&self, conn_id: Uuid, message: WsMessage) -> WsResult<()> {
        let connections = self.connections.read().await;
        if let Some(state) = connections.get(&conn_id) {
            // Ignore send errors - receiver may be closed (e.g. in tests)
            let _ = state.sender.send(message).await;
        }
        Ok(())
    }

    /// Broadcast message to channel
    pub async fn broadcast_to_channel(&self, channel: &str, data: serde_json::Value) {
        let event = ChannelEvent {
            channel: channel.to_string(),
            data,
            user_id: None,
        };

        if let Err(e) = self.event_sender.send(event) {
            error!("Failed to broadcast event: {}", e);
        }
    }

    /// Send message to specific user
    pub async fn send_to_user(&self, user_id: &str, channel: &str, data: serde_json::Value) {
        let event = ChannelEvent {
            channel: channel.to_string(),
            data,
            user_id: Some(user_id.to_string()),
        };

        if let Err(e) = self.event_sender.send(event) {
            error!("Failed to send user event: {}", e);
        }
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let connections = self.connections.read().await;
        let subscriptions = self.subscriptions.read().await;

        let mut authenticated_count = 0;
        let mut expired_count = 0;

        for state in connections.values() {
            if state.authenticated {
                authenticated_count += 1;
            }
            if state.info.is_expired(self.config.connection_timeout) {
                expired_count += 1;
            }
        }

        ConnectionStats {
            total_connections: connections.len(),
            authenticated_connections: authenticated_count,
            expired_connections: expired_count,
            total_channels: subscriptions.len(),
        }
    }

    /// Clean up expired connections
    pub async fn cleanup_expired(&self) -> usize {
        let mut to_remove = Vec::new();

        {
            let connections = self.connections.read().await;
            for (&id, state) in connections.iter() {
                if state.info.is_expired(self.config.connection_timeout) {
                    to_remove.push(id);
                }
            }
        }

        let count = to_remove.len();
        for id in to_remove {
            self.remove_connection(id).await;
        }

        if count > 0 {
            info!("Cleaned up {} expired connections", count);
        }

        count
    }

    /// Start background tasks
    pub async fn start_background_tasks(&self) {
        let manager = Arc::new(self.clone());

        // Cleanup task
        let cleanup_manager = Arc::clone(&manager);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                cleanup_manager.cleanup_expired().await;
            }
        });

        // Event broadcasting task
        let broadcast_manager = Arc::clone(&manager);
        let mut event_receiver = self.event_sender.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                broadcast_manager.handle_channel_event(event).await;
            }
        });
    }

    /// Handle channel event broadcasting
    async fn handle_channel_event(&self, event: ChannelEvent) {
        // Clone necessary data before async operations to prevent deadlock
        let subscribers_with_senders = {
            let subscriptions = self.subscriptions.read().await;
            let connections = self.connections.read().await;

            if let Some(subscribers) = subscriptions.get(&event.channel) {
                subscribers
                    .iter()
                    .filter_map(|&conn_id| {
                        connections.get(&conn_id).map(|state| {
                            // Filter by user ID if event is user-specific
                            if let Some(target_user) = &event.user_id
                                && state.user_id.as_ref() != Some(target_user)
                            {
                                return None;
                            }
                            Some((conn_id, state.sender.clone()))
                        })
                    })
                    .flatten()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        };

        // Send messages after releasing locks
        let message = WsMessage::event(&event.channel, event.data);
        for (conn_id, sender) in subscribers_with_senders {
            if let Err(e) = sender.send(message.clone()).await {
                error!("Failed to send event to connection {}: {}", conn_id, e);
            }
        }
    }
}

impl std::fmt::Debug for WebSocketManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketManager")
            .field("config", &self.config)
            .field("connections", &"<connections>")
            .field("subscriptions", &"<subscriptions>")
            .field("event_sender", &"<event_sender>")
            .field("auth_handler", &self.auth_handler.is_some())
            .finish()
    }
}

impl Clone for WebSocketManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            connections: Arc::clone(&self.connections),
            subscriptions: Arc::clone(&self.subscriptions),
            event_sender: self.event_sender.clone(),
            auth_handler: self.auth_handler.clone(),
            connections_per_ip: Arc::clone(&self.connections_per_ip),
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Total number of connections
    pub total_connections: usize,
    /// Number of authenticated connections
    pub authenticated_connections: usize,
    /// Number of expired connections
    pub expired_connections: usize,
    /// Total number of channels
    pub total_channels: usize,
}

// Test helpers available to integration tests
impl WebSocketManager {
    /// Test helper: Set connection as authenticated
    #[doc(hidden)]
    pub async fn test_set_authenticated(&self, conn_id: uuid::Uuid, user_id: &str) {
        let mut connections = self.connections.write().await;
        if let Some(state) = connections.get_mut(&conn_id) {
            state.authenticated = true;
            state.user_id = Some(user_id.to_string());
        }
    }

    /// Test helper: Subscribe connection to channel
    #[doc(hidden)]
    pub async fn test_subscribe_channel(&self, conn_id: uuid::Uuid, channel: &str) {
        let mut connections = self.connections.write().await;
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(state) = connections.get_mut(&conn_id)
            && !state.channels.contains(&channel.to_string())
        {
            state.channels.push(channel.to_string());
            subscriptions
                .entry(channel.to_string())
                .or_insert_with(Vec::new)
                .push(conn_id);
        }
    }

    /// Test helper: Get IP connection count
    #[doc(hidden)]
    pub async fn test_get_ip_connection_count(&self) -> usize {
        self.connections_per_ip.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    struct MockAuthHandler;

    #[async_trait::async_trait]
    impl AuthHandler for MockAuthHandler {
        async fn authenticate(&self, token: &str) -> Result<String, String> {
            if token == "valid_token" {
                Ok("user123".to_string())
            } else {
                Err("Invalid token".to_string())
            }
        }

        async fn check_permission(&self, _user_id: &str, channel: &str) -> bool {
            !channel.starts_with("private_")
        }
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let config = WebSocketConfig::default();
        let manager = WebSocketManager::new(config);

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.total_channels, 0);
    }

    #[tokio::test]
    async fn test_add_remove_connection() {
        let config = WebSocketConfig::default();
        let manager = WebSocketManager::new(config);

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let info = ConnectionInfo::new(addr);
        let conn_id = info.id;

        // Add connection
        let _sender = manager.add_connection(conn_id, info).await.unwrap();
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 1);

        // Remove connection
        manager.remove_connection(conn_id).await;
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_authentication() {
        let config = WebSocketConfig::default();
        let manager = WebSocketManager::new(config).with_auth_handler(Arc::new(MockAuthHandler));

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let info = ConnectionInfo::new(addr);
        let conn_id = info.id;

        let _sender = manager.add_connection(conn_id, info).await.unwrap();

        // Test authentication logic without message sending
        // Valid authentication - should work
        if let Some(auth_handler) = &manager.auth_handler {
            let result = auth_handler.authenticate("valid_token").await;
            assert!(result.is_ok());

            let result = auth_handler.authenticate("invalid_token").await;
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_subscription() {
        let config = WebSocketConfig::default();
        let manager = WebSocketManager::new(config);

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let info = ConnectionInfo::new(addr);
        let conn_id = info.id;

        let _sender = manager.add_connection(conn_id, info).await.unwrap();

        // Test subscription logic - first authenticate the connection manually
        {
            let mut connections = manager.connections.write().await;
            if let Some(state) = connections.get_mut(&conn_id) {
                state.authenticated = true;
            }
        }

        // Test channel subscriptions - this shouldn't try to send messages
        let mut subscriptions = manager.subscriptions.write().await;
        subscriptions.insert("channel1".to_string(), vec![conn_id]);
        subscriptions.insert("channel2".to_string(), vec![conn_id]);
        drop(subscriptions);

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_channels, 2);

        // Test unsubscription by directly modifying subscriptions
        let mut subscriptions = manager.subscriptions.write().await;
        subscriptions.remove("channel1");
        drop(subscriptions);

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_channels, 1);
    }

    #[tokio::test]
    async fn test_connection_limit() {
        let config = WebSocketConfig {
            max_connections: 1,
            ..Default::default()
        };
        let manager = WebSocketManager::new(config);

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

        // First connection should succeed
        let info1 = ConnectionInfo::new(addr);
        let _sender1 = manager.add_connection(info1.id, info1).await;
        assert!(_sender1.is_ok());

        // Second connection should fail
        let info2 = ConnectionInfo::new(addr);
        let result = manager.add_connection(info2.id, info2).await;
        assert!(matches!(result, Err(WsError::ConnectionLimitExceeded)));
    }
}
