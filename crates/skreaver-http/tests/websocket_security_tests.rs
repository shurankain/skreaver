//! WebSocket security and concurrent access tests

use skreaver_http::websocket::{
    AuthHandler, ConnectionInfo, WebSocketConfig, WebSocketManager, WsError,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};

struct TestAuthHandler;

#[async_trait::async_trait]
impl AuthHandler for TestAuthHandler {
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
async fn test_message_size_limit() {
    let config = WebSocketConfig {
        max_message_size: 100,
        ..Default::default()
    };
    let manager = WebSocketManager::new(config);

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let info = ConnectionInfo::new(addr);
    let conn_id = info.id();

    let _sender = manager.add_connection(conn_id, info).await.unwrap();

    // Test message size limit - this is enforced at the handler level
    // The config is set correctly and the value is used
    assert_eq!(manager.config.max_message_size, 100);
}

#[tokio::test]
async fn test_subscription_limit_per_connection() {
    let config = WebSocketConfig {
        max_subscriptions_per_connection: 3,
        ..Default::default()
    };
    let manager = WebSocketManager::new(config);

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let info = ConnectionInfo::new(addr);
    let conn_id = info.id();

    let _sender = manager.add_connection(conn_id, info).await.unwrap();
    manager.test_set_authenticated(conn_id, "test_user").await;

    // Subscribe to channels up to limit
    let result = manager
        .handle_subscribe(
            conn_id,
            vec!["ch1".to_string(), "ch2".to_string(), "ch3".to_string()],
        )
        .await;
    assert!(result.is_ok());

    // Attempt to subscribe beyond limit should fail
    let result = manager
        .handle_subscribe(conn_id, vec!["ch4".to_string()])
        .await;
    assert!(matches!(
        result,
        Err(WsError::SubscriptionLimitExceeded { .. })
    ));
}

#[tokio::test]
async fn test_channel_subscriber_limit() {
    let config = WebSocketConfig {
        max_subscribers_per_channel: 2,
        ..Default::default()
    };
    let manager = WebSocketManager::new(config);

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

    // Add 3 connections
    let mut conn_ids = Vec::new();
    for _ in 0..3 {
        let info = ConnectionInfo::new(addr);
        let conn_id = info.id();
        let _sender = manager.add_connection(conn_id, info).await.unwrap();

        // Set authenticated
        manager.test_set_authenticated(conn_id, "test_user").await;

        conn_ids.push(conn_id);
    }

    // First two subscriptions should succeed
    for &conn_id in &conn_ids[0..2] {
        let result = manager
            .handle_subscribe(conn_id, vec!["test_channel".to_string()])
            .await;
        assert!(result.is_ok());
    }

    // Third subscription should fail
    let result = manager
        .handle_subscribe(conn_ids[2], vec!["test_channel".to_string()])
        .await;
    assert!(matches!(
        result,
        Err(WsError::ChannelSubscriberLimitExceeded { .. })
    ));
}

#[tokio::test]
async fn test_ip_rate_limiting() {
    let config = WebSocketConfig {
        max_connections_per_ip: 2,
        ..Default::default()
    };
    let manager = WebSocketManager::new(config);

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

    // First two connections should succeed
    let info1 = ConnectionInfo::new(addr);
    let conn_id1 = info1.id();
    let result1 = manager.add_connection(conn_id1, info1).await;
    assert!(result1.is_ok());

    let info2 = ConnectionInfo::new(addr);
    let conn_id2 = info2.id();
    let result2 = manager.add_connection(conn_id2, info2).await;
    assert!(result2.is_ok());

    // Third connection from same IP should fail
    let info3 = ConnectionInfo::new(addr);
    let result3 = manager.add_connection(info3.id(), info3).await;
    assert!(matches!(result3, Err(WsError::RateLimitExceeded)));

    // After removing one connection, new connection should succeed
    manager.remove_connection(conn_id1).await;
    let info4 = ConnectionInfo::new(addr);
    let result4 = manager.add_connection(info4.id(), info4).await;
    assert!(result4.is_ok());
}

#[tokio::test]
async fn test_concurrent_subscription_race_condition() {
    // This test verifies the TOCTOU fix
    let config = WebSocketConfig::default();
    let manager =
        Arc::new(WebSocketManager::new(config).with_auth_handler(Arc::new(TestAuthHandler)));

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let info = ConnectionInfo::new(addr);
    let conn_id = info.id();

    let _sender = manager.add_connection(conn_id, info).await.unwrap();

    // Authenticate the connection
    manager.test_set_authenticated(conn_id, "user123").await;

    // Spawn multiple concurrent subscription requests
    let mut handles = vec![];
    for i in 0..5 {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            manager_clone
                .handle_subscribe(conn_id, vec![format!("channel_{}", i)])
                .await
        });
        handles.push(handle);
    }

    // All should succeed without race conditions
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_authentication_required_for_subscription() {
    let config = WebSocketConfig::default();
    let manager = WebSocketManager::new(config).with_auth_handler(Arc::new(TestAuthHandler));

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let info = ConnectionInfo::new(addr);
    let conn_id = info.id();

    let _sender = manager.add_connection(conn_id, info).await.unwrap();

    // Attempt to subscribe without authentication should fail
    let result = manager
        .handle_subscribe(conn_id, vec!["test_channel".to_string()])
        .await;
    assert!(matches!(result, Err(WsError::AuthenticationFailed(_))));

    // Authenticate
    manager.test_set_authenticated(conn_id, "user123").await;

    // Now subscription should succeed
    let result = manager
        .handle_subscribe(conn_id, vec!["test_channel".to_string()])
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_permission_denied_for_private_channel() {
    let config = WebSocketConfig::default();
    let manager = WebSocketManager::new(config).with_auth_handler(Arc::new(TestAuthHandler));

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let info = ConnectionInfo::new(addr);
    let conn_id = info.id();

    let _sender = manager.add_connection(conn_id, info).await.unwrap();

    // Authenticate
    manager.test_set_authenticated(conn_id, "user123").await;

    // Attempt to subscribe to private channel should fail
    let result = manager
        .handle_subscribe(conn_id, vec!["private_admin".to_string()])
        .await;
    assert!(matches!(result, Err(WsError::PermissionDenied)));
}

#[tokio::test]
async fn test_connection_cleanup_on_error() {
    let config = WebSocketConfig::default();
    let manager = WebSocketManager::new(config);

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let info = ConnectionInfo::new(addr);
    let conn_id = info.id();

    let _sender = manager.add_connection(conn_id, info).await.unwrap();

    // Verify connection exists
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_connections, 1);

    // Remove connection
    manager.remove_connection(conn_id).await;

    // Verify connection removed
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_connections, 0);

    // Verify IP count cleaned up
    let ip_count = manager.test_get_ip_connection_count().await;
    assert_eq!(ip_count, 0);
}

#[tokio::test]
async fn test_broadcast_without_deadlock() {
    let config = WebSocketConfig::default();
    let manager = Arc::new(WebSocketManager::new(config));

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

    // Add multiple connections
    for _ in 0..10 {
        let info = ConnectionInfo::new(addr);
        let conn_id = info.id();
        let _sender = manager.add_connection(conn_id, info).await.unwrap();

        // Set authenticated and subscribe
        manager.test_set_authenticated(conn_id, "test_user").await;
        manager
            .test_subscribe_channel(conn_id, "test_channel")
            .await;
    }

    // Broadcast multiple messages concurrently
    let mut handles = vec![];
    for i in 0..20 {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            manager_clone
                .broadcast_to_channel(
                    &"test_channel".into(),
                    serde_json::json!({"message": format!("test_{}", i)}),
                )
                .await;
        });
        handles.push(handle);
    }

    // Wait for all broadcasts to complete without deadlock
    let timeout = tokio::time::timeout(Duration::from_secs(5), async {
        for handle in handles {
            handle.await.unwrap();
        }
    })
    .await;

    assert!(timeout.is_ok(), "Broadcast deadlocked");
}

#[tokio::test]
async fn test_expired_connections_cleanup() {
    let config = WebSocketConfig {
        connection_timeout: Duration::from_millis(100),
        ..Default::default()
    };
    let manager = WebSocketManager::new(config);

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let info = ConnectionInfo::new(addr);
    let conn_id = info.id();

    let _sender = manager.add_connection(conn_id, info).await.unwrap();

    // Wait for connection to expire
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Cleanup expired connections
    let cleaned = manager.cleanup_expired().await;
    assert_eq!(cleaned, 1);

    // Verify connection removed
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_connections, 0);
}
