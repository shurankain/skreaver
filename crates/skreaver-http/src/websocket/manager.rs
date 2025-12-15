//! WebSocket connection manager

use super::lock_ordering::ManagerLocks;
use super::{WebSocketConfig, WsError, WsMessage, WsResult};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Background task handles for graceful shutdown
struct BackgroundTasks {
    cleanup_task: Option<JoinHandle<()>>,
    orphaned_cleanup_task: Option<JoinHandle<()>>,
    broadcast_task: Option<JoinHandle<()>>,
    shutdown_signal: Arc<AtomicBool>,
}

impl BackgroundTasks {
    fn new() -> Self {
        Self {
            cleanup_task: None,
            orphaned_cleanup_task: None,
            broadcast_task: None,
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Shutdown all background tasks
    fn shutdown(&mut self) {
        self.shutdown_signal.store(true, Ordering::Release);

        if let Some(handle) = self.cleanup_task.take() {
            handle.abort();
        }
        if let Some(handle) = self.orphaned_cleanup_task.take() {
            handle.abort();
        }
        if let Some(handle) = self.broadcast_task.take() {
            handle.abort();
        }
    }
}

impl Drop for BackgroundTasks {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// WebSocket connection manager
pub struct WebSocketManager {
    /// Manager configuration
    pub config: WebSocketConfig,
    /// Type-safe locks with enforced ordering
    locks: ManagerLocks,
    /// Event broadcaster
    event_sender: broadcast::Sender<ChannelEvent>,
    /// Authentication handler
    auth_handler: Option<Arc<dyn AuthHandler + Send + Sync>>,
    /// Background task handles for lifecycle management
    background_tasks: Arc<Mutex<BackgroundTasks>>,
}

/// Connection state with typestate pattern
#[derive(Debug)]
pub(super) enum ConnectionState {
    /// Unauthenticated connection
    Unauthenticated {
        info: super::ConnectionInfo<super::Unauthenticated>,
        sender: mpsc::Sender<WsMessage>,
        channels: Vec<String>,
    },
    /// Authenticated connection
    Authenticated {
        info: super::ConnectionInfo<super::Authenticated>,
        sender: mpsc::Sender<WsMessage>,
        channels: Vec<String>,
    },
}

impl ConnectionState {
    /// Create new unauthenticated connection state
    fn new_unauthenticated(
        info: super::ConnectionInfo<super::Unauthenticated>,
        sender: mpsc::Sender<WsMessage>,
    ) -> Self {
        Self::Unauthenticated {
            info,
            sender,
            channels: Vec::new(),
        }
    }

    /// Get connection info (works for both states)
    fn info(&self) -> &dyn InfoAccess {
        match self {
            Self::Unauthenticated { info, .. } => info,
            Self::Authenticated { info, .. } => info,
        }
    }

    /// Get mutable connection info (works for both states)
    fn info_mut(&mut self) -> &mut dyn InfoAccessMut {
        match self {
            Self::Unauthenticated { info, .. } => info,
            Self::Authenticated { info, .. } => info,
        }
    }

    /// Get message sender
    fn sender(&self) -> &mpsc::Sender<WsMessage> {
        match self {
            Self::Unauthenticated { sender, .. } => sender,
            Self::Authenticated { sender, .. } => sender,
        }
    }

    /// Get subscribed channels
    fn channels(&self) -> &[String] {
        match self {
            Self::Unauthenticated { channels, .. } => channels,
            Self::Authenticated { channels, .. } => channels,
        }
    }

    /// Get mutable subscribed channels
    fn channels_mut(&mut self) -> &mut Vec<String> {
        match self {
            Self::Unauthenticated { channels, .. } => channels,
            Self::Authenticated { channels, .. } => channels,
        }
    }

    /// Check if connection is authenticated
    fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated { .. })
    }

    /// Get user ID if authenticated
    fn user_id(&self) -> Option<&str> {
        match self {
            Self::Authenticated { info, .. } => Some(info.user_id()),
            Self::Unauthenticated { .. } => None,
        }
    }

    /// Authenticate the connection
    fn authenticate(&mut self, user_id: String) {
        // Take ownership of the current state to transition
        // Create a placeholder address - this is safe as it's a temporary value
        // that will never be used (the actual info is moved from old_state)
        let placeholder_addr = std::net::SocketAddr::from(([0, 0, 0, 0], 0));

        let old_state = std::mem::replace(
            self,
            Self::Unauthenticated {
                info: super::ConnectionInfo::new(placeholder_addr),
                sender: mpsc::channel(1).0,
                channels: Vec::new(),
            },
        );

        *self = match old_state {
            Self::Unauthenticated {
                info,
                sender,
                channels,
            } => Self::Authenticated {
                info: info.authenticate(user_id),
                sender,
                channels,
            },
            Self::Authenticated {
                info,
                sender,
                channels,
            } => {
                // Already authenticated, update user_id by re-creating
                Self::Authenticated {
                    info: super::ConnectionInfo {
                        id: info.id,
                        addr: info.addr,
                        connected_at: info.connected_at,
                        last_activity: info.last_activity,
                        metadata: info.metadata,
                        state: super::Authenticated { user_id },
                    },
                    sender,
                    channels,
                }
            }
        };
    }
}

/// Trait for accessing connection info across states
trait InfoAccess {
    fn addr(&self) -> std::net::SocketAddr;
    fn is_expired(&self, timeout: std::time::Duration) -> bool;
}

impl<State> InfoAccess for super::ConnectionInfo<State> {
    fn addr(&self) -> std::net::SocketAddr {
        self.addr()
    }
    fn is_expired(&self, timeout: std::time::Duration) -> bool {
        self.is_expired(timeout)
    }
}

/// Trait for mutably accessing connection info across states
trait InfoAccessMut: InfoAccess {
    fn update_activity(&mut self);
}

impl<State> InfoAccessMut for super::ConnectionInfo<State> {
    fn update_activity(&mut self) {
        self.update_activity();
    }
}

/// Channel event for broadcasting
#[derive(Debug, Clone)]
pub struct ChannelEvent {
    /// Channel name
    pub channel: crate::websocket::protocol::Channel,
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
            locks: ManagerLocks::new(),
            event_sender,
            auth_handler: None,
            background_tasks: Arc::new(Mutex::new(BackgroundTasks::new())),
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
        info: super::ConnectionInfo<super::Unauthenticated>,
    ) -> WsResult<mpsc::Sender<WsMessage>> {
        // Acquire write locks with enforced ordering (connections + ip_connections)
        let mut guards = self.locks.level2_write().await;

        // Check global connection limit
        if guards.connections.len() >= self.config.max_connections {
            return Err(WsError::ConnectionLimitExceeded);
        }

        // Check IP-based rate limiting
        let ip_addr = info.addr().ip();
        let ip_count = guards.ip_connections.get(&ip_addr).copied().unwrap_or(0);
        if ip_count >= self.config.max_connections_per_ip {
            return Err(WsError::RateLimitExceeded);
        }

        // Atomically increment IP counter and add connection
        *guards.ip_connections.entry(ip_addr).or_insert(0) += 1;

        let (sender, _receiver) = mpsc::channel(self.config.buffer_size);

        let state = ConnectionState::new_unauthenticated(info, sender.clone());

        guards.connections.insert(id, state);

        // Locks released automatically when guards drop
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
        let mut guards = self.locks.level3_write().await;

        if let Some(state) = guards.connections.remove(&id) {
            // Decrement IP connection count with validation
            let ip_addr = state.info().addr().ip();
            if let Some(count) = guards.ip_connections.get_mut(&ip_addr) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    guards.ip_connections.remove(&ip_addr);
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
            for channel in state.channels() {
                if let Some(subscribers) = guards.subscriptions.get_mut(channel) {
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
                        guards.subscriptions.remove(channel);
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

        // Guards automatically drop in reverse order (subscriptions, ip_connections, connections)
    }

    /// Update connection activity
    pub async fn update_activity(&self, id: Uuid) {
        let mut guard = self.locks.level1_write().await;
        if let Some(state) = guard.connections.get_mut(&id) {
            state.info_mut().update_activity();
        }
    }

    /// Store handshake information in connection metadata
    pub async fn store_handshake_info(
        &self,
        id: Uuid,
        client_name: String,
        client_version: String,
        capabilities: Vec<String>,
    ) {
        let mut guard = self.locks.level1_write().await;
        if let Some(state) = guard.connections.get_mut(&id) {
            // Store handshake information in metadata for both connection types
            match state {
                ConnectionState::Unauthenticated { info, .. } => {
                    info.metadata.insert("client_name".to_string(), client_name);
                    info.metadata
                        .insert("client_version".to_string(), client_version);
                    info.metadata
                        .insert("capabilities".to_string(), capabilities.join(","));
                }
                ConnectionState::Authenticated { info, .. } => {
                    info.metadata.insert("client_name".to_string(), client_name);
                    info.metadata
                        .insert("client_version".to_string(), client_version);
                    info.metadata
                        .insert("capabilities".to_string(), capabilities.join(","));
                }
            }
        }
    }

    /// Handle incoming message
    pub async fn handle_message(&self, conn_id: Uuid, message: WsMessage) -> WsResult<()> {
        debug!("Handling message from {}: {:?}", conn_id, message);

        // Update activity
        self.update_activity(conn_id).await;

        match message {
            WsMessage::Ping { .. } => {
                let result = self.send_to_connection(conn_id, WsMessage::pong()).await?;
                if result.is_failure() {
                    tracing::warn!(
                        connection_id = %conn_id,
                        send_result = %result,
                        "Failed to send pong response"
                    );
                }
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
                    let mut guard = self.locks.level1_write().await;
                    if let Some(state) = guard.connections.get_mut(&conn_id) {
                        state.authenticate(user_id);
                        drop(guard);

                        let result = self
                            .send_to_connection(
                                conn_id,
                                WsMessage::success("Authentication successful"),
                            )
                            .await?;

                        if result.is_failure() {
                            tracing::warn!(
                                connection_id = %conn_id,
                                send_result = %result,
                                "Failed to send authentication success message"
                            );
                        }
                        info!("Connection {} authenticated", conn_id);
                    }
                }
                Err(error) => {
                    return Err(WsError::AuthenticationFailed(error));
                }
            }
        } else {
            // No auth handler, consider all connections authenticated with anonymous ID
            let mut guard = self.locks.level1_write().await;
            if let Some(state) = guard.connections.get_mut(&conn_id) {
                state.authenticate(format!("anonymous_{}", conn_id));
                drop(guard);

                let result = self
                    .send_to_connection(conn_id, WsMessage::success("Authentication successful"))
                    .await?;

                if result.is_failure() {
                    tracing::warn!(
                        connection_id = %conn_id,
                        send_result = %result,
                        "Failed to send authentication success message (anonymous)"
                    );
                }
            }
        }

        Ok(())
    }

    /// Handle channel subscription
    #[doc(hidden)] // Public for testing only
    pub async fn handle_subscribe(&self, conn_id: Uuid, channels: Vec<String>) -> WsResult<()> {
        // Phase 1: Check permissions outside critical section (read-only snapshot)
        // This prevents race conditions by doing async operations before acquiring write lock
        let user_id_opt = {
            let guard = self.locks.level1_read().await;
            let state = guard
                .connections
                .get(&conn_id)
                .ok_or(WsError::ConnectionClosed)?;

            // Check authentication
            if self.auth_handler.is_some() && !state.is_authenticated() {
                return Err(WsError::AuthenticationFailed(
                    "Authentication required".to_string(),
                ));
            }

            state.user_id().map(|s| s.to_string())
        };

        // Perform async permission checks outside locks
        if let (Some(auth_handler), Some(user_id)) = (&self.auth_handler, &user_id_opt) {
            for channel in &channels {
                if !auth_handler.check_permission(user_id, channel).await {
                    return Err(WsError::PermissionDenied);
                }
            }
        }

        // Phase 2: Perform subscription atomically with write lock
        let mut guards = self.locks.level3_write().await;

        let state = guards
            .connections
            .get_mut(&conn_id)
            .ok_or(WsError::ConnectionClosed)?;

        // Re-check authentication (connection state may have changed)
        if self.auth_handler.is_some() && !state.is_authenticated() {
            return Err(WsError::AuthenticationFailed(
                "Authentication required".to_string(),
            ));
        }

        // Check subscription limit per connection (only counting new subscriptions)
        let new_channels: Vec<_> = channels
            .iter()
            .filter(|ch| !state.channels().contains(ch))
            .collect();

        let new_subscription_count = state.channels().len() + new_channels.len();
        if new_subscription_count > self.config.max_subscriptions_per_connection {
            return Err(WsError::SubscriptionLimitExceeded {
                current: new_subscription_count,
                max: self.config.max_subscriptions_per_connection,
            });
        }

        // Add subscriptions atomically
        for channel in new_channels {
            // Check channel subscriber limit
            let current_subscribers = guards
                .subscriptions
                .get(channel)
                .map(|subs| subs.len())
                .unwrap_or(0);

            if current_subscribers >= self.config.max_subscribers_per_channel {
                return Err(WsError::ChannelSubscriberLimitExceeded {
                    current: current_subscribers + 1,
                    max: self.config.max_subscribers_per_channel,
                });
            }

            state.channels_mut().push(channel.clone());
            guards
                .subscriptions
                .entry(channel.clone())
                .or_insert_with(Vec::new)
                .push(conn_id);

            debug!("Connection {} subscribed to channel {}", conn_id, channel);
        }

        drop(guards);

        let result = self
            .send_to_connection(conn_id, WsMessage::success("Subscription successful"))
            .await?;

        if result.is_failure() {
            tracing::warn!(
                connection_id = %conn_id,
                send_result = %result,
                "Failed to send subscription success message"
            );
        }

        Ok(())
    }

    /// Handle channel unsubscription
    async fn handle_unsubscribe(&self, conn_id: Uuid, channels: Vec<String>) -> WsResult<()> {
        // Fixed: Was acquiring subscriptions before connections (wrong order!)
        // Now uses level3_write() to acquire in correct order
        let mut guards = self.locks.level3_write().await;

        if let Some(state) = guards.connections.get_mut(&conn_id) {
            for channel in channels {
                if let Some(index) = state.channels().iter().position(|c| c == &channel) {
                    state.channels_mut().remove(index);

                    if let Some(subscribers) = guards.subscriptions.get_mut(&channel) {
                        subscribers.retain(|&id| id != conn_id);
                        if subscribers.is_empty() {
                            guards.subscriptions.remove(&channel);
                        }
                    }

                    debug!(
                        "Connection {} unsubscribed from channel {}",
                        conn_id, channel
                    );
                }
            }
        }

        drop(guards);

        let result = self
            .send_to_connection(conn_id, WsMessage::success("Unsubscription successful"))
            .await?;

        if result.is_failure() {
            tracing::warn!(
                connection_id = %conn_id,
                send_result = %result,
                "Failed to send unsubscription success message"
            );
        }

        Ok(())
    }

    /// Send a message to a specific connection
    ///
    /// Returns a [`SendResult`] indicating the delivery status:
    /// - `SendResult::Sent` - Message successfully sent
    /// - `SendResult::Queued { queue_size }` - Message queued (queue filling up)
    /// - `SendResult::ConnectionClosed` - Connection doesn't exist
    /// - `SendResult::BufferFull` - Send buffer is full
    ///
    /// # Examples
    ///
    /// ```ignore
    /// match manager.send_to_connection(conn_id, message).await? {
    ///     SendResult::Sent => {
    ///         // Success!
    ///     }
    ///     SendResult::Queued { queue_size } if queue_size > 100 => {
    ///         // Implement backpressure
    ///     }
    ///     SendResult::ConnectionClosed => {
    ///         // Clean up and stop sending
    ///     }
    ///     SendResult::BufferFull => {
    ///         // Retry or drop message
    ///     }
    ///     _ => {}
    /// }
    /// ```
    pub async fn send_to_connection(
        &self,
        conn_id: Uuid,
        message: WsMessage,
    ) -> WsResult<super::SendResult> {
        use super::SendResult;

        let guard = self.locks.level1_read().await;
        if let Some(state) = guard.connections.get(&conn_id) {
            let sender = state.sender();

            // Check queue capacity before sending
            let capacity = sender.capacity();
            let max_capacity = sender.max_capacity();

            match sender.try_send(message) {
                Ok(()) => {
                    // Calculate current queue size
                    let queue_size = max_capacity - capacity;

                    if queue_size == 0 {
                        Ok(SendResult::Sent)
                    } else {
                        // Queue has messages - return queue size for backpressure
                        Ok(SendResult::Queued { queue_size })
                    }
                }
                Err(e) => {
                    use tokio::sync::mpsc::error::TrySendError;

                    match e {
                        TrySendError::Full(_) => {
                            tracing::warn!(
                                connection_id = %conn_id,
                                "Send buffer full - message dropped"
                            );
                            Ok(SendResult::BufferFull)
                        }
                        TrySendError::Closed(_) => {
                            // Channel closed
                            tracing::warn!(
                                connection_id = %conn_id,
                                "Connection closed - cannot send message"
                            );
                            Ok(SendResult::ConnectionClosed)
                        }
                    }
                }
            }
        } else {
            // Connection doesn't exist
            Ok(SendResult::ConnectionClosed)
        }
    }

    /// Broadcast message to channel
    pub async fn broadcast_to_channel(
        &self,
        channel: &crate::websocket::protocol::Channel,
        data: serde_json::Value,
    ) {
        let event = ChannelEvent {
            channel: channel.clone(),
            data,
            user_id: None,
        };

        if let Err(e) = self.event_sender.send(event) {
            error!("Failed to broadcast event: {}", e);
        }
    }

    /// Send message to specific user
    pub async fn send_to_user(
        &self,
        user_id: &str,
        channel: &crate::websocket::protocol::Channel,
        data: serde_json::Value,
    ) {
        let event = ChannelEvent {
            channel: channel.clone(),
            data,
            user_id: Some(user_id.to_string()),
        };

        if let Err(e) = self.event_sender.send(event) {
            error!("Failed to send user event: {}", e);
        }
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let guard = self.locks.level3_read().await;

        let mut authenticated_count = 0;
        let mut expired_count = 0;

        for state in guard.connections.values() {
            if state.is_authenticated() {
                authenticated_count += 1;
            }
            if state.info().is_expired(self.config.connection_timeout) {
                expired_count += 1;
            }
        }

        ConnectionStats {
            total_connections: guard.connections.len(),
            authenticated_connections: authenticated_count,
            expired_connections: expired_count,
            total_channels: guard.subscriptions.len(),
        }
    }

    /// Clean up expired connections
    pub async fn cleanup_expired(&self) -> usize {
        let mut to_remove = Vec::new();

        {
            let guard = self.locks.level1_read().await;
            for (&id, state) in guard.connections.iter() {
                if state.info().is_expired(self.config.connection_timeout) {
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
        let mut guards = self.locks.level3_write().await;

        let mut orphaned_subscription_count = 0;
        let mut orphaned_ip_count = 0;

        // Clean up orphaned subscriptions
        let connection_ids: std::collections::HashSet<_> =
            guards.connections.keys().copied().collect();

        for (channel, subscribers) in guards.subscriptions.iter_mut() {
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
        guards
            .subscriptions
            .retain(|_, subscribers| !subscribers.is_empty());

        // Validate and clean up IP tracking
        let mut actual_ip_counts: std::collections::HashMap<std::net::IpAddr, usize> =
            std::collections::HashMap::new();

        for state in guards.connections.values() {
            *actual_ip_counts
                .entry(state.info().addr().ip())
                .or_insert(0) += 1;
        }

        // Check for discrepancies and orphaned entries
        for (ip, tracked_count) in guards.ip_connections.iter() {
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
        guards
            .ip_connections
            .retain(|ip, _| actual_ip_counts.contains_key(ip));

        // Correct any count mismatches
        for (ip, actual_count) in actual_ip_counts {
            guards.ip_connections.insert(ip, actual_count);
        }

        drop(guards);

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
        let mut tasks = self.background_tasks.lock().await;

        // Stop existing tasks if any and wait for them to complete
        if tasks.cleanup_task.is_some()
            || tasks.orphaned_cleanup_task.is_some()
            || tasks.broadcast_task.is_some()
        {
            tasks.shutdown();
            // Release lock and wait briefly for tasks to stop
            drop(tasks);
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            tasks = self.background_tasks.lock().await;
        }

        // Create fresh shutdown signal
        let shutdown = Arc::new(AtomicBool::new(false));
        tasks.shutdown_signal = Arc::clone(&shutdown);

        let manager = Arc::new(self.clone());

        // Cleanup task for expired connections
        let cleanup_manager = Arc::clone(&manager);
        let shutdown_clone = Arc::clone(&shutdown);
        let cleanup_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        cleanup_manager.cleanup_expired().await;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                        if shutdown_clone.load(Ordering::Acquire) {
                            info!("Cleanup task shutting down");
                            break;
                        }
                    }
                }
            }
        });
        tasks.cleanup_task = Some(cleanup_handle);

        // Orphaned state cleanup task (runs less frequently)
        let orphaned_cleanup_manager = Arc::clone(&manager);
        let shutdown_clone = Arc::clone(&shutdown);
        let orphaned_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // Every 5 minutes
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        orphaned_cleanup_manager.cleanup_orphaned_state().await;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                        if shutdown_clone.load(Ordering::Acquire) {
                            info!("Orphaned cleanup task shutting down");
                            break;
                        }
                    }
                }
            }
        });
        tasks.orphaned_cleanup_task = Some(orphaned_handle);

        // Event broadcasting task
        let broadcast_manager = Arc::clone(&manager);
        let mut event_receiver = self.event_sender.subscribe();
        let shutdown_clone = Arc::clone(&shutdown);
        let broadcast_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = event_receiver.recv() => {
                        if let Ok(event) = event {
                            broadcast_manager.handle_channel_event(event).await;
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                        if shutdown_clone.load(Ordering::Acquire) {
                            info!("Event broadcast task shutting down");
                            break;
                        }
                    }
                }
            }
        });
        tasks.broadcast_task = Some(broadcast_handle);
    }

    /// Shutdown all background tasks gracefully
    pub async fn shutdown(&self) {
        let mut tasks = self.background_tasks.lock().await;
        tasks.shutdown();
    }

    /// Handle channel event broadcasting
    async fn handle_channel_event(&self, event: ChannelEvent) {
        // Clone necessary data before async operations to prevent deadlock
        let subscribers_with_senders = {
            let guard = self.locks.level3_read().await;

            if let Some(subscribers) = guard.subscriptions.get(&event.channel.to_string()) {
                subscribers
                    .iter()
                    .filter_map(|&conn_id| {
                        guard.connections.get(&conn_id).and_then(|state| {
                            // Filter by user ID if event is user-specific
                            if let Some(target_user) = &event.user_id
                                && state.user_id() != Some(target_user)
                            {
                                return None;
                            }
                            Some((conn_id, state.sender().clone()))
                        })
                    })
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
            locks: self.locks.clone(),
            event_sender: self.event_sender.clone(),
            auth_handler: self.auth_handler.clone(),
            background_tasks: Arc::clone(&self.background_tasks),
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
        let mut guard = self.locks.level1_write().await;
        if let Some(state) = guard.connections.get_mut(&conn_id) {
            state.authenticate(user_id.to_string());
        }
    }

    /// Test helper: Subscribe connection to channel
    #[doc(hidden)]
    pub async fn test_subscribe_channel(&self, conn_id: uuid::Uuid, channel: &str) {
        let mut guards = self.locks.level3_write().await;

        if let Some(state) = guards.connections.get_mut(&conn_id)
            && !state.channels().contains(&channel.to_string())
        {
            state.channels_mut().push(channel.to_string());
            guards
                .subscriptions
                .entry(channel.to_string())
                .or_insert_with(Vec::new)
                .push(conn_id);
        }
    }

    /// Get the number of active connections
    pub async fn connection_count(&self) -> usize {
        let guard = self.locks.level1_read().await;
        guard.connections.len()
    }

    /// Test helper: Get IP connection count
    #[doc(hidden)]
    pub async fn test_get_ip_connection_count(&self) -> usize {
        let guard = self.locks.level2_read().await;
        guard.ip_connections.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::ConnectionInfo;
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
        let conn_id = info.id();

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
        let conn_id = info.id();

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
        let conn_id = info.id();

        let _sender = manager.add_connection(conn_id, info).await.unwrap();

        // Test subscription logic - first authenticate the connection manually
        {
            let mut guard = manager.locks.level1_write().await;
            if let Some(state) = guard.connections.get_mut(&conn_id) {
                state.authenticate("test_user".to_string());
            }
        }

        // Test channel subscriptions - this shouldn't try to send messages
        {
            let mut guard = manager.locks.level3_write().await;
            guard
                .subscriptions
                .insert("channel1".to_string(), vec![conn_id]);
            guard
                .subscriptions
                .insert("channel2".to_string(), vec![conn_id]);
        }

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_channels, 2);

        // Test unsubscription by directly modifying subscriptions
        {
            let mut guard = manager.locks.level3_write().await;
            guard.subscriptions.remove("channel1");
        }

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
            let id = info.id();
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
        let conn_id = info.id();
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
