//! Integration tests for graceful shutdown functionality
//!
//! These tests verify that the graceful shutdown signal handlers compile
//! and integrate correctly with the HTTP runtime.

use skreaver_http::runtime::{HttpAgentRuntime, shutdown_signal, shutdown_signal_with_timeout, shutdown_with_cleanup};
use skreaver_tools::InMemoryToolRegistry;
use tokio::net::TcpListener;
use std::time::Duration;

/// Test that shutdown_signal compiles and can be used with axum::serve
#[tokio::test]
async fn test_shutdown_signal_compiles() {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);
    let app = runtime.router();

    // Bind to port 0 to get random available port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    // Create the server with graceful shutdown
    // We don't actually run it, just verify it compiles
    let _server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal());

    // Test passes if this compiles
}

/// Test that shutdown_signal_with_timeout compiles
#[tokio::test]
async fn test_shutdown_signal_with_timeout_compiles() {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);
    let app = runtime.router();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let _server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal_with_timeout(Duration::from_secs(30)));

    // Test passes if this compiles
}

/// Test that shutdown_with_cleanup compiles and accepts cleanup function
#[tokio::test]
async fn test_shutdown_with_cleanup_compiles() {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);
    let app = runtime.router();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let cleanup = || async {
        // Simulate cleanup tasks
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    let _server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_with_cleanup(cleanup));

    // Test passes if this compiles
}

/// Test that the HTTP runtime can be created and served with graceful shutdown
#[tokio::test]
async fn test_runtime_with_graceful_shutdown() {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);
    let app = runtime.router();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    // Spawn server in background task
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                // Immediately shutdown for test
                tokio::time::sleep(Duration::from_millis(10)).await;
            })
            .await
    });

    // Wait for server to shutdown
    let result = server_handle.await;
    assert!(result.is_ok());
}

/// Test graceful shutdown documentation example
#[test]
fn test_graceful_shutdown_documentation() {
    // This is a compile-time test to ensure the example in docs compiles
    let _example = || async {
        use skreaver_http::runtime::{HttpAgentRuntime, shutdown_signal};
        use skreaver_tools::InMemoryToolRegistry;
        use tokio::net::TcpListener;

        let registry = InMemoryToolRegistry::new();
        let runtime = HttpAgentRuntime::new(registry);
        let app = runtime.router();

        let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .unwrap();
    };
}
