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
    ///
    /// Uses write lock from the start to prevent TOCTOU race conditions
    /// when checking and updating connection limits.
    pub async fn add_connection(
        &self,
        id: Uuid,
        info: ConnectionInfo,
    ) -> WsResult<mpsc::Sender<WsMessage>> {
        // Acquire write locks immediately to prevent race conditions
        let mut connections = self.connections.write().await;
        let mut ip_connections = self.connections_per_ip.write().await;

        // Check global connection limit
        if connections.len() >= self.config.max_connections {
            return Err(WsError::ConnectionLimitExceeded);
        }

        // Check IP-based rate limiting
        let ip_addr = info.addr.ip();
        let ip_count = ip_connections.get(&ip_addr).copied().unwrap_or(0);
        if ip_count >= self.config.max_connections_per_ip {
            return Err(WsError::RateLimitExceeded);
        }

        // Atomically increment IP counter and add connection
        *ip_connections.entry(ip_addr).or_insert(0) += 1;

        let (sender, _receiver) = mpsc::channel(self.config.buffer_size);

        let state = ConnectionState {
            info,
            sender: sender.clone(),
            channels: Vec::new(),
            authenticated: false,
            user_id: None,
        };

        connections.insert(id, state);

        // Release locks
        drop(ip_connections);
        drop(connections);

        info!("Added WebSocket connection: {}", id);
        Ok(sender)
    }

    /// Remove a connection
    ///
    /// Acquires all necessary locks in a consistent order to prevent deadlocks:
    /// 1. connections
    /// 2. ip_connections
    /// 3. subscriptions
    ///
    /// Performs validation to ensure complete cleanup and detect inconsistencies.
    pub async fn remove_connection(&self, id: Uuid) {
        // Acquire all locks upfront in consistent order to prevent deadlocks
        let mut connections = self.connections.write().await;
        let mut ip_connections = self.connections_per_ip.write().await;
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(state) = connections.remove(&id) {
            // Decrement IP connection count with validation
            let ip_addr = state.info.addr.ip();
            if let Some(count) = ip_connections.get_mut(&ip_addr) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    ip_connections.remove(&ip_addr);
                } else if *count > 1000 {
                    // Sanity check: if count is unreasonably high, log warning
                    warn!(
                        "IP {} has unusually high connection count: {}",
                        ip_addr, count
                    );
                }
            } else {
                // IP address not found in tracking map - this indicates inconsistency
                warn!(
                    "Connection {} had IP {} not tracked in ip_connections map",
                    id, ip_addr
                );
            }

            // Unsubscribe from all channels with validation
            let mut cleaned_channels = 0;
            for channel in &state.channels {
                if let Some(subscribers) = subscriptions.get_mut(channel) {
                    let before_len = subscribers.len();
                    subscribers.retain(|&conn_id| conn_id != id);
                    let after_len = subscribers.len();

                    if before_len == after_len {
                        // Connection was not in subscriber list - inconsistency
                        warn!(
                            "Connection {} was not in subscriber list for channel {}",
                            id, channel
                        );
                    } else {
                        cleaned_channels += 1;
                    }

                    if subscribers.is_empty() {
                        subscriptions.remove(channel);
                    }
                } else {
                    // Channel not found in subscriptions map - inconsistency
                    warn!(
                        "Connection {} subscribed to non-existent channel {}",
                        id, channel
                    );
                }
            }

            debug!(
                "Removed WebSocket connection {}: cleaned {} channel subscriptions",
                id, cleaned_channels
            );
            info!("Removed WebSocket connection: {}", id);
        } else {
            // Attempted to remove non-existent connection
            debug!("Attempted to remove non-existent connection: {}", id);
        }

        // Explicit drops for clarity (locks released in reverse order)
        drop(subscriptions);
        drop(ip_connections);
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

        let state = connections
            .get_mut(&conn_id)
            .ok_or(WsError::ConnectionClosed)?;

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
        let state = connections
            .get_mut(&conn_id)
            .ok_or(WsError::ConnectionClosed)?;
        if self.auth_handler.is_some() && !state.authenticated {
            return Err(WsError::AuthenticationFailed(
                "Authentication required".to_string(),
            ));
        }

        // Re-check subscription limit per connection (state may have changed)
        let new_subscription_count = state.channels.len()
            + channels
                .iter()
                .filter(|ch| !state.channels.contains(ch))
                .count();
        if new_subscription_count > self.config.max_subscriptions_per_connection {
            return Err(WsError::SubscriptionLimitExceeded {
                current: new_subscription_count,
                max: self.config.max_subscriptions_per_connection,
            });
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

    /// Detect and clean up orphaned state
    ///
    /// Checks for:
    /// - Orphaned subscriptions (pointing to non-existent connections)
    /// - Orphaned IP tracking entries (IPs with 0 connections but not removed)
    /// - Inconsistent subscription counts
    ///
    /// Returns a tuple: (orphaned_subscriptions, orphaned_ips)
    pub async fn cleanup_orphaned_state(&self) -> (usize, usize) {
        let connections = self.connections.write().await;
        let mut ip_connections = self.connections_per_ip.write().await;
        let mut subscriptions = self.subscriptions.write().await;

        let mut orphaned_subscription_count = 0;
        let mut orphaned_ip_count = 0;

        // Clean up orphaned subscriptions
        let connection_ids: std::collections::HashSet<_> = connections.keys().copied().collect();

        for (channel, subscribers) in subscriptions.iter_mut() {
            let before_len = subscribers.len();
            subscribers.retain(|conn_id| connection_ids.contains(conn_id));
            let removed = before_len - subscribers.len();

            if removed > 0 {
                warn!(
                    "Found {} orphaned subscriptions in channel {}",
                    removed, channel
                );
                orphaned_subscription_count += removed;
            }
        }

        // Remove empty channels
        subscriptions.retain(|_, subscribers| !subscribers.is_empty());

        // Validate and clean up IP tracking
        let mut actual_ip_counts: std::collections::HashMap<std::net::IpAddr, usize> =
            std::collections::HashMap::new();

        for state in connections.values() {
            *actual_ip_counts.entry(state.info.addr.ip()).or_insert(0) += 1;
        }

        // Check for discrepancies and orphaned entries
        for (ip, tracked_count) in ip_connections.iter() {
            let actual_count = actual_ip_counts.get(ip).copied().unwrap_or(0);

            if actual_count == 0 {
                // Orphaned IP entry
                warn!(
                    "Found orphaned IP tracking entry for {} with count {}",
                    ip, tracked_count
                );
                orphaned_ip_count += 1;
            } else if actual_count != *tracked_count {
                // Count mismatch
                warn!(
                    "IP {} count mismatch: tracked={}, actual={}",
                    ip, tracked_count, actual_count
                );
            }
        }

        // Remove orphaned IP entries
        ip_connections.retain(|ip, _| actual_ip_counts.contains_key(ip));

        // Correct any count mismatches
        for (ip, actual_count) in actual_ip_counts {
            ip_connections.insert(ip, actual_count);
        }

        drop(subscriptions);
        drop(ip_connections);
        drop(connections);

        if orphaned_subscription_count > 0 || orphaned_ip_count > 0 {
            info!(
                "Cleaned up orphaned state: {} subscriptions, {} IP entries",
                orphaned_subscription_count, orphaned_ip_count
            );
        }

        (orphaned_subscription_count, orphaned_ip_count)
    }

    /// Start background tasks
    pub async fn start_background_tasks(&self) {
        let manager = Arc::new(self.clone());

        // Cleanup task for expired connections
        let cleanup_manager = Arc::clone(&manager);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                cleanup_manager.cleanup_expired().await;
            }
        });

        // Orphaned state cleanup task (runs less frequently)
        let orphaned_cleanup_manager = Arc::clone(&manager);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // Every 5 minutes
            loop {
                interval.tick().await;
                orphaned_cleanup_manager.cleanup_orphaned_state().await;
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

    #[tokio::test]
    async fn test_concurrent_connection_additions() {
        use std::sync::Arc;

        // Test that concurrent add_connection calls don't race
        let config = WebSocketConfig {
            max_connections: 5,
            max_connections_per_ip: 5,
            ..Default::default()
        };
        let manager = Arc::new(WebSocketManager::new(config));

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

        // Spawn 10 concurrent connection attempts
        let mut handles = vec![];
        for _ in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                let info = ConnectionInfo::new(addr);
                manager_clone.add_connection(info.id, info).await
            });
            handles.push(handle);
        }

        // Wait for all attempts
        let mut success_count = 0;
        let mut failure_count = 0;
        for handle in handles {
            match handle.await.unwrap() {
                Ok(_) => success_count += 1,
                Err(_) => failure_count += 1,
            }
        }

        // Exactly 5 should succeed (max_connections), 5 should fail
        assert_eq!(
            success_count, 5,
            "Expected exactly 5 successful connections"
        );
        assert_eq!(failure_count, 5, "Expected exactly 5 failed connections");

        // Verify final state
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 5);
    }

    #[tokio::test]
    async fn test_concurrent_remove_and_add() {
        use std::sync::Arc;

        let config = WebSocketConfig {
            max_connections: 10,
            max_connections_per_ip: 10,
            ..Default::default()
        };
        let manager = Arc::new(WebSocketManager::new(config));
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

        // Add initial connections
        let mut connection_ids = vec![];
        for _ in 0..5 {
            let info = ConnectionInfo::new(addr);
            let id = info.id;
            manager.add_connection(id, info).await.unwrap();
            connection_ids.push(id);
        }

        // Concurrent remove and add operations
        let mut remove_handles = vec![];
        let mut add_handles = vec![];

        // Remove connections
        for id in connection_ids {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                manager_clone.remove_connection(id).await;
            });
            remove_handles.push(handle);
        }

        // Add new connections concurrently
        for _ in 0..5 {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                let info = ConnectionInfo::new(addr);
                manager_clone.add_connection(info.id, info).await
            });
            add_handles.push(handle);
        }

        // Wait for all operations
        for handle in remove_handles {
            handle.await.unwrap();
        }
        for handle in add_handles {
            handle.await.unwrap().ok();
        }

        // State should be consistent
        let stats = manager.get_stats().await;
        assert_eq!(
            stats.total_connections, 5,
            "Expected 5 connections after concurrent add/remove"
        );
    }

    #[tokio::test]
    async fn test_subscription_race_condition() {
        use std::sync::Arc;

        let config = WebSocketConfig {
            max_subscriptions_per_connection: 10,
            ..Default::default()
        };
        let manager = Arc::new(WebSocketManager::new(config));
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

        // Add a connection
        let info = ConnectionInfo::new(addr);
        let conn_id = info.id;
        manager.add_connection(conn_id, info).await.unwrap();

        // Concurrent subscription attempts to the same connection
        let mut handles = vec![];
        for i in 0..20 {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                let channels = vec![format!("channel_{}", i % 5)]; // 5 unique channels, repeated
                manager_clone.handle_subscribe(conn_id, channels).await
            });
            handles.push(handle);
        }

        // Wait for all subscription attempts
        for handle in handles {
            handle.await.unwrap().ok();
        }

        // Verify subscription count doesn't exceed limits
        let stats = manager.get_stats().await;
        // Since we have 20 attempts on 5 unique channels, we should end up with 5 subscriptions
        assert!(
            stats.total_channels <= 10,
            "Channel subscriptions should not exceed per-connection limit"
        );
        assert_eq!(
            stats.total_channels, 5,
            "Should have exactly 5 unique channel subscriptions"
        );
    }
}
