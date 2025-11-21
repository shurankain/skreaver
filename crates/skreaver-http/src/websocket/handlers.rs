//! WebSocket route handlers and middleware

use super::{
    ConnectionInfo, WebSocketManager, WsError, WsMessage,
    protocol::{MessageEnvelope, MessagePayload, ResponseData, channels, events},
};
use axum::{
    Json,
    extract::{ConnectInfo, Query, State, ws::WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tracing::{debug, error, info, warn};

/// WebSocket upgrade query parameters
#[derive(Debug, Deserialize)]
pub struct WsUpgradeQuery {
    /// Optional authentication token
    pub token: Option<String>,
    /// Client identifier
    pub client: Option<String>,
    /// Client version
    pub version: Option<String>,
}

/// WebSocket connection status response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsStatusResponse {
    /// WebSocket endpoint URL
    pub websocket_url: String,
    /// Supported protocol version
    pub protocol_version: String,
    /// Available channels
    pub available_channels: Vec<String>,
    /// Connection statistics
    pub stats: WsStats,
}

/// WebSocket statistics
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsStats {
    /// Total active connections
    pub active_connections: usize,
    /// Authenticated connections
    pub authenticated_connections: usize,
    /// Total channels
    pub total_channels: usize,
    /// Server uptime in seconds
    pub uptime_seconds: u64,
}

/// WebSocket upgrade handler with query parameters
pub async fn websocket_upgrade_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(manager): State<Arc<WebSocketManager>>,
    Query(query): Query<WsUpgradeQuery>,
) -> Response {
    info!(
        "WebSocket upgrade request from {} (client: {:?}, version: {:?})",
        addr, query.client, query.version
    );

    // Validate client information
    if let Some(client) = &query.client
        && !is_valid_client_name(client)
    {
        warn!("Invalid client name from {}: {}", addr, client);
        return (StatusCode::BAD_REQUEST, "Invalid client name").into_response();
    }

    // Check connection limits before upgrade
    let stats = manager.get_stats().await;
    if stats.total_connections >= manager.config.max_connections {
        warn!("Connection limit exceeded for {}", addr);
        return (StatusCode::TOO_MANY_REQUESTS, "Connection limit exceeded").into_response();
    }

    ws.on_upgrade(move |socket| handle_websocket_connection(socket, addr, manager, query))
}

/// Handle individual WebSocket connection with enhanced protocol
async fn handle_websocket_connection(
    socket: axum::extract::ws::WebSocket,
    addr: SocketAddr,
    manager: Arc<WebSocketManager>,
    query: WsUpgradeQuery,
) {
    let mut conn_info = ConnectionInfo::new(addr);

    // Add client metadata from query
    if let Some(client) = query.client {
        conn_info.metadata.insert("client".to_string(), client);
    }
    if let Some(version) = query.version {
        conn_info.metadata.insert("version".to_string(), version);
    }

    let conn_id = conn_info.id();
    info!(
        "WebSocket connection established: {} from {}",
        conn_id, addr
    );

    // Register connection with manager
    let _sender = match manager.add_connection(conn_id, conn_info).await {
        Ok(sender) => sender,
        Err(e) => {
            error!("Failed to register connection {}: {}", conn_id, e);
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send welcome message
    let welcome = MessageEnvelope::event(
        channels::SYSTEM,
        events::SERVER_STARTED,
        serde_json::json!({
            "connectionId": conn_id,
            "serverVersion": env!("CARGO_PKG_VERSION"),
            "protocolVersion": super::protocol::PROTOCOL_VERSION,
            "availableChannels": [
                channels::SYSTEM,
                channels::AGENTS,
                channels::TASKS,
                channels::NOTIFICATIONS,
                channels::METRICS,
                channels::DEBUG
            ]
        }),
    );

    if let Ok(welcome_json) = serde_json::to_string(&welcome)
        && ws_sender
            .send(axum::extract::ws::Message::Text(welcome_json.into()))
            .await
            .is_err()
    {
        warn!("Failed to send welcome message to {}", conn_id);
    }

    // Auto-authenticate if token provided
    if let Some(token) = query.token {
        let auth_msg = WsMessage::Auth { token };
        if let Err(e) = manager.handle_message(conn_id, auth_msg).await {
            error!("Auto-authentication failed for {}: {}", conn_id, e);
        }
    }

    // Message handling loop
    use futures::{SinkExt, StreamExt};

    let manager_clone = Arc::clone(&manager);
    let receive_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Text(text)) => {
                    // Try parsing as MessageEnvelope first
                    if let Ok(envelope) = serde_json::from_str::<MessageEnvelope>(&text) {
                        if let Err(e) =
                            handle_protocol_message(&manager_clone, conn_id, envelope).await
                        {
                            error!("Error handling protocol message from {}: {}", conn_id, e);
                        }
                    }
                    // Fallback to legacy WsMessage format
                    else if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                        if let Err(e) = manager_clone.handle_message(conn_id, ws_msg).await {
                            error!("Error handling legacy message from {}: {}", conn_id, e);
                        }
                    } else {
                        error!("Invalid message format from {}: {}", conn_id, text);
                    }
                }
                Ok(axum::extract::ws::Message::Binary(_)) => {
                    warn!("Binary messages not supported from {}", conn_id);
                }
                Ok(axum::extract::ws::Message::Close(_)) => {
                    info!("Connection {} closed by client", conn_id);
                    break;
                }
                Ok(axum::extract::ws::Message::Ping(_)) => {
                    // Handled automatically by axum
                }
                Ok(axum::extract::ws::Message::Pong(_)) => {
                    manager_clone.update_activity(conn_id).await;
                }
                Err(e) => {
                    error!("WebSocket error from {}: {}", conn_id, e);
                    break;
                }
            }
        }
    });

    // Wait for receive task to complete
    let _ = receive_task.await;

    // Cleanup
    info!("WebSocket connection {} disconnected", conn_id);
    manager.remove_connection(conn_id).await;
}

/// Handle protocol message envelope
async fn handle_protocol_message(
    manager: &WebSocketManager,
    conn_id: uuid::Uuid,
    envelope: MessageEnvelope,
) -> Result<(), WsError> {
    debug!(
        "Handling protocol message from {}: {:?}",
        conn_id, envelope.payload
    );

    match envelope.payload {
        MessagePayload::Handshake(data) => {
            info!(
                "Handshake from {}: {} v{} (capabilities: {:?})",
                conn_id, data.client_name, data.client_version, data.capabilities
            );

            // Store handshake information in connection metadata
            manager
                .store_handshake_info(
                    conn_id,
                    data.client_name,
                    data.client_version,
                    data.capabilities,
                )
                .await;

            debug!("Stored handshake information for connection {}", conn_id);
            Ok(())
        }
        MessagePayload::Auth(data) => {
            let ws_msg = WsMessage::Auth { token: data.token };
            manager.handle_message(conn_id, ws_msg).await
        }
        MessagePayload::Subscribe(data) => {
            let channels = data.channels.into_iter().map(|sub| sub.channel).collect();
            let ws_msg = WsMessage::Subscribe { channels };
            manager.handle_message(conn_id, ws_msg).await
        }
        MessagePayload::Request(data) => {
            // Handle RPC-style requests
            handle_rpc_request(manager, conn_id, &envelope.message_id, data).await
        }
        MessagePayload::Ping(data) => {
            let pong =
                MessageEnvelope::pong(data.timestamp).with_correlation_id(envelope.message_id);
            send_protocol_message(manager, conn_id, pong).await
        }
        _ => {
            warn!(
                "Unexpected message type from {}: {:?}",
                conn_id, envelope.payload
            );
            Ok(())
        }
    }
}

/// Handle RPC-style request
async fn handle_rpc_request(
    manager: &WebSocketManager,
    conn_id: uuid::Uuid,
    message_id: &str,
    request: super::protocol::RequestData,
) -> Result<(), WsError> {
    debug!("Handling RPC request from {}: {}", conn_id, request.method);

    let response = match request.method.as_str() {
        "ping" => MessageEnvelope::success_response(serde_json::json!({
            "timestamp": chrono::Utc::now().timestamp_millis()
        })),
        "subscribe" => {
            if let Ok(channels) = serde_json::from_value::<Vec<String>>(request.params) {
                let ws_msg = WsMessage::Subscribe { channels };
                manager.handle_message(conn_id, ws_msg).await?;
                MessageEnvelope::success_response(serde_json::json!({
                    "message": "Subscribed successfully"
                }))
            } else {
                MessageEnvelope::error_response("INVALID_PARAMS", "Invalid channels parameter")
            }
        }
        "unsubscribe" => {
            if let Ok(channels) = serde_json::from_value::<Vec<String>>(request.params) {
                let ws_msg = WsMessage::Unsubscribe { channels };
                manager.handle_message(conn_id, ws_msg).await?;
                MessageEnvelope::success_response(serde_json::json!({
                    "message": "Unsubscribed successfully"
                }))
            } else {
                MessageEnvelope::error_response("INVALID_PARAMS", "Invalid channels parameter")
            }
        }
        "get_stats" => {
            let stats = manager.get_stats().await;
            MessageEnvelope::success_response(serde_json::json!({
                "totalConnections": stats.total_connections,
                "authenticatedConnections": stats.authenticated_connections,
                "totalChannels": stats.total_channels
            }))
        }
        _ => MessageEnvelope::error_response(
            "METHOD_NOT_FOUND",
            &format!("Method '{}' not found", request.method),
        ),
    };

    let response = response.with_correlation_id(message_id.to_string());
    send_protocol_message(manager, conn_id, response).await
}

/// Send protocol message to connection
async fn send_protocol_message(
    manager: &WebSocketManager,
    conn_id: uuid::Uuid,
    envelope: MessageEnvelope,
) -> Result<(), WsError> {
    // Convert to legacy WsMessage format for now
    // TODO: Update manager to handle MessageEnvelope directly
    let legacy_msg = match envelope.payload {
        MessagePayload::Event(data) => WsMessage::Event {
            channel: data.channel,
            data: data.data,
            timestamp: envelope.timestamp,
        },
        MessagePayload::Error(data) => WsMessage::Error {
            code: data.error.code,
            message: data.error.message,
        },
        MessagePayload::Response(data) => match data {
            ResponseData::Success { result } => WsMessage::Success {
                message: serde_json::to_string(&result).unwrap_or_default(),
            },
            ResponseData::Error { error } => WsMessage::Error {
                code: error.code,
                message: error.message,
            },
            ResponseData::Pending => WsMessage::Success {
                message: "Pending".to_string(),
            },
            ResponseData::Cancelled => WsMessage::Success {
                message: "Cancelled".to_string(),
            },
        },
        MessagePayload::Pong(_) => WsMessage::Pong {
            timestamp: envelope.timestamp,
        },
        _ => return Ok(()), // Skip other message types for now
    };

    let result = manager.send_to_connection(conn_id, legacy_msg).await?;
    if result.is_failure() {
        tracing::warn!(
            connection_id = %conn_id,
            send_result = %result,
            "Failed to send legacy message conversion"
        );
    }
    Ok(())
}

/// Get WebSocket status and information
pub async fn websocket_status_handler(
    State(manager): State<Arc<WebSocketManager>>,
) -> Json<WsStatusResponse> {
    let stats = manager.get_stats().await;

    let response = WsStatusResponse {
        websocket_url: "/ws".to_string(),
        protocol_version: super::protocol::PROTOCOL_VERSION.to_string(),
        available_channels: vec![
            channels::SYSTEM.to_string(),
            channels::AGENTS.to_string(),
            channels::TASKS.to_string(),
            channels::NOTIFICATIONS.to_string(),
            channels::METRICS.to_string(),
            channels::DEBUG.to_string(),
        ],
        stats: WsStats {
            active_connections: stats.total_connections,
            authenticated_connections: stats.authenticated_connections,
            total_channels: stats.total_channels,
            uptime_seconds: 0, // TODO: Track server uptime
        },
    };

    Json(response)
}

/// Broadcast event to channel (admin endpoint)
#[derive(Debug, Deserialize)]
pub struct BroadcastRequest {
    /// Target channel
    pub channel: String,
    /// Event type
    pub event_type: String,
    /// Event data
    pub data: serde_json::Value,
    /// Optional user ID for user-specific events
    pub user_id: Option<String>,
}

pub async fn broadcast_handler(
    State(manager): State<Arc<WebSocketManager>>,
    Json(request): Json<BroadcastRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!(
        "Broadcasting event to channel {}: {}",
        request.channel, request.event_type
    );

    if let Some(user_id) = request.user_id {
        manager
            .send_to_user(&user_id, &request.channel, request.data)
            .await;
    } else {
        manager
            .broadcast_to_channel(&request.channel, request.data)
            .await;
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Event broadcasted successfully"
    })))
}

/// Validate client name
fn is_valid_client_name(name: &str) -> bool {
    // Allow alphanumeric characters, hyphens, and underscores
    name.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        && !name.is_empty()
        && name.len() <= 50
}

/// WebSocket middleware for request logging and metrics
pub async fn websocket_middleware(
    headers: HeaderMap,
    addr: ConnectInfo<SocketAddr>,
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    let start = std::time::Instant::now();
    let addr = addr.0;

    // Log connection attempt
    info!("WebSocket connection attempt from {}", addr);

    // Check for required headers
    if !headers.contains_key("upgrade") || !headers.contains_key("connection") {
        warn!("Invalid WebSocket upgrade headers from {}", addr);
        return Err(StatusCode::BAD_REQUEST);
    }

    let response = next.run(request).await;
    let duration = start.elapsed();

    // Log connection result
    info!("WebSocket upgrade processed for {} in {:?}", addr, duration);

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_client_name() {
        assert!(is_valid_client_name("valid-client"));
        assert!(is_valid_client_name("client_v1"));
        assert!(is_valid_client_name("ClientApp123"));

        assert!(!is_valid_client_name(""));
        assert!(!is_valid_client_name("invalid client")); // space
        assert!(!is_valid_client_name("invalid@client")); // special char
        assert!(!is_valid_client_name(&"x".repeat(51))); // too long
    }

    #[test]
    fn test_ws_upgrade_query_parsing() {
        // This would be tested with actual query parsing in integration tests
        let query = WsUpgradeQuery {
            token: Some("test-token".to_string()),
            client: Some("test-client".to_string()),
            version: Some("1.0.0".to_string()),
        };

        assert_eq!(query.token, Some("test-token".to_string()));
        assert_eq!(query.client, Some("test-client".to_string()));
        assert_eq!(query.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_broadcast_request() {
        let request = BroadcastRequest {
            channel: "test".to_string(),
            event_type: "test-event".to_string(),
            data: serde_json::json!({"key": "value"}),
            user_id: None,
        };

        assert_eq!(request.channel, "test");
        assert_eq!(request.event_type, "test-event");
        assert_eq!(request.data["key"], "value");
        assert!(request.user_id.is_none());
    }
}
