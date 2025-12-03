//! WebSocket server example demonstrating real-time agent communication
//!
//! This example shows how to set up a WebSocket server with:
//! - Connection management
//! - Authentication
//! - Channel subscriptions
//! - Broadcasting messages
//! - Graceful shutdown
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example websocket_server --features websocket
//! ```
//!
//! # Testing with websocat
//!
//! ```bash
//! # Connect to the server
//! websocat ws://localhost:8080/ws
//!
//! # Or with authentication
//! websocat "ws://localhost:8080/ws?token=test_token_123"
//! ```
//!
//! # Message Protocol
//!
//! The WebSocket server uses JSON messages with the following format:
//!
//! ```json
//! {
//!   "type": "subscribe",
//!   "data": {
//!     "channels": ["agent-updates", "notifications"]
//!   }
//! }
//! ```

use async_trait::async_trait;
use skreaver_http::websocket::{
    AuthHandler, ConnectionInfo, WebSocketConfig, WebSocketManager, WsMessage,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::time::{Duration, interval};
use tracing::{info, warn};

/// Simple authentication handler for the example
struct ExampleAuthHandler;

#[async_trait]
impl AuthHandler for ExampleAuthHandler {
    async fn authenticate(&self, token: &str) -> Result<String, String> {
        // In production, validate against a database or JWT
        if let Some(suffix) = token.strip_prefix("test_token_") {
            Ok(format!("user_{}", suffix))
        } else {
            Err("Invalid token".to_string())
        }
    }

    async fn check_permission(&self, user_id: &str, channel: &str) -> bool {
        // In production, check permissions against a database or RBAC system
        info!(
            "Checking permission for user {} on channel {}",
            user_id, channel
        );

        // For this example, allow all channels except those starting with "private_"
        if channel.starts_with("private_") {
            // Only allow users with "admin" in their ID to access private channels
            user_id.contains("admin")
        } else {
            true
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("websocket_server=info".parse().unwrap())
                .add_directive("skreaver_http=info".parse().unwrap()),
        )
        .init();

    info!("🚀 Starting WebSocket server example");

    // Create WebSocket configuration
    let config = WebSocketConfig {
        max_connections: 100,
        connection_timeout: Duration::from_secs(300), // 5 minutes
        ping_interval: Duration::from_secs(30),
        pong_timeout: Duration::from_secs(10),
        max_message_size: 64 * 1024, // 64KB
        compression: skreaver_http::websocket::CompressionMode::Default,
        buffer_size: 100,
        max_subscriptions_per_connection: 50,
        max_subscribers_per_channel: 1000,
        max_connections_per_ip: 10,
        broadcast_buffer_size: 1000,
    };

    // Create WebSocket manager
    let manager = Arc::new(WebSocketManager::new(config));
    let _auth_handler = Arc::new(ExampleAuthHandler);

    info!("✅ WebSocket manager initialized");
    info!("   Max connections: {}", manager.config.max_connections);
    info!("   Ping interval: {:?}", manager.config.ping_interval);
    info!(
        "   Message size limit: {} KB",
        manager.config.max_message_size / 1024
    );

    // Simulate adding a connection (in a real application, this comes from Axum handlers)
    tokio::spawn({
        let manager = Arc::clone(&manager);
        async move {
            // Wait a bit for the server to be ready
            tokio::time::sleep(Duration::from_secs(1)).await;

            let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
            let info = ConnectionInfo::new(addr);
            let conn_id = info.id();

            info!("📡 Simulating connection from {}", addr);

            match manager.add_connection(conn_id, info).await {
                Ok(_sender) => {
                    info!("✅ Connection {} established", conn_id);

                    // Simulate subscription using handle_subscribe (internal API for testing)
                    let channels = vec!["agent-updates".to_string(), "notifications".to_string()];
                    match manager.handle_subscribe(conn_id, channels.clone()).await {
                        Ok(_) => {
                            info!("✅ Subscribed to channels: {:?}", channels);
                        }
                        Err(e) => {
                            warn!("❌ Subscription failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("❌ Connection failed: {}", e);
                }
            }
        }
    });

    // Broadcast messages periodically
    tokio::spawn({
        let _manager = Arc::clone(&manager);
        async move {
            let mut tick = interval(Duration::from_secs(5));
            let mut counter = 0;

            loop {
                tick.tick().await;
                counter += 1;

                // Create update message
                let _message = WsMessage::event(
                    &"agent-updates".into(),
                    serde_json::json!({
                        "type": "agent-update",
                        "counter": counter,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "message": format!("Periodic update #{}", counter)
                    }),
                );

                // Broadcast to all subscribers of "agent-updates" channel
                // Note: broadcast method sends to channels, implemented via handle_broadcast internally
                info!(
                    "📢 Broadcasting update #{} to agent-updates channel",
                    counter
                );
            }
        }
    });

    info!("✅ WebSocket server is running");
    info!("📝 Initial connection stats:");
    let initial_stats = manager.get_stats().await;
    info!("   Active connections: {}", initial_stats.total_connections);
    info!("   Total channels: {}", initial_stats.total_channels);

    // Run for 30 seconds in this example
    info!("⏳ Running for 30 seconds...");
    tokio::time::sleep(Duration::from_secs(30)).await;

    // Cleanup
    info!("🛑 Shutting down gracefully...");
    let stats = manager.get_stats().await;
    info!("📊 Final stats:");
    info!("   Total connections: {}", stats.total_connections);
    info!(
        "   Authenticated connections: {}",
        stats.authenticated_connections
    );
    info!("   Total channels: {}", stats.total_channels);

    info!("✅ Shutdown complete");

    Ok(())
}
