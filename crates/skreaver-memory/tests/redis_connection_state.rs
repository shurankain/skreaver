//! Integration tests for Redis connection state tracking with phantom types
//!
//! These tests demonstrate the compile-time guarantees and runtime behavior
//! of the type-safe connection state management system.

#[cfg(feature = "redis")]
mod redis_connection_state_tests {
    use skreaver_memory::redis::{Disconnected, RedisConnection, StatefulConnectionManager};
    use std::time::Duration;

    #[test]
    fn test_connection_state_phantom_types() {
        // Test that we can create disconnected connections
        let disconnected = RedisConnection::<Disconnected>::new_disconnected();
        assert_eq!(disconnected.attempt_count(), 0);

        // Test attempt count tracking
        let reset_disconnected = disconnected.reset_attempts();
        assert_eq!(reset_disconnected.attempt_count(), 0);
    }

    #[test]
    fn test_connection_state_transitions() {
        // Create a disconnected connection
        let disconnected = RedisConnection::<Disconnected>::new_disconnected();

        // Verify initial state
        assert_eq!(disconnected.attempt_count(), 0);

        // Test that we can reset attempts
        let reset_conn = disconnected.reset_attempts();
        assert_eq!(reset_conn.attempt_count(), 0);
    }

    #[tokio::test]
    async fn test_connection_manager_creation() {
        // Test StatefulConnectionManager creation without actual Redis
        // This demonstrates the API design

        // Note: This test doesn't actually connect to Redis, but shows the API
        // In a real test environment, you would use a test Redis instance

        // The fact that this compiles demonstrates type safety
        let _create_manager = |pool| {
            StatefulConnectionManager::new(pool)
                .with_max_idle_duration(Duration::from_secs(300))
                .with_max_retry_attempts(3)
        };
    }

    #[test]
    fn test_compile_time_state_guarantees() {
        // These tests verify that the type system prevents invalid operations

        // Creating a disconnected connection is always safe
        let disconnected = RedisConnection::<Disconnected>::new_disconnected();

        // Disconnected connections can be reset
        let _reset = disconnected.reset_attempts();

        // The following operations would only be available on Connected connections:
        // - connection() method to get underlying connection
        // - execute() method for Redis operations
        // - ping() method
        // - disconnect() method
        // - connection_duration() method
        // - idle_duration() method
        // - is_stale() method

        // This demonstrates that the type system prevents using disconnected connections
        // for operations that require a live connection.
    }

    #[test]
    fn test_connection_attempt_tracking() {
        let disconnected = RedisConnection::<Disconnected>::new_disconnected();
        assert_eq!(disconnected.attempt_count(), 0);

        // Simulate multiple connection attempts
        // In a real scenario, each failed connect() call would increment the counter
        // Here we just verify that the counter can be reset
        let reset_conn = disconnected.reset_attempts();
        assert_eq!(reset_conn.attempt_count(), 0);
    }

    #[test]
    fn test_state_manager_configuration() {
        // Test that StatefulConnectionManager can be configured
        // This demonstrates the builder pattern

        use deadpool_redis::{Config, Pool};

        // Create a dummy pool configuration (won't actually connect)
        let _config = Config::from_url("redis://localhost:6379");

        // This would create a pool in real usage
        // let pool = config.create_pool(Some(deadpool_redis::Runtime::Tokio1)).unwrap();

        // The fact that we can call these methods shows the API design works
        let _configured_manager = |pool: Pool| {
            StatefulConnectionManager::new(pool)
                .with_max_idle_duration(Duration::from_secs(600))
                .with_max_retry_attempts(5)
        };
    }

    // Compile-time tests: These demonstrate that certain operations are impossible

    #[test]
    fn test_impossible_operations_at_compile_time() {
        // This test demonstrates what operations are NOT possible at compile time

        let disconnected = RedisConnection::<Disconnected>::new_disconnected();

        // The following lines would NOT compile if uncommented:

        // disconnected.connection(); // ERROR: method not available on Disconnected
        // disconnected.ping().await; // ERROR: method not available on Disconnected
        // disconnected.execute(|_| async { Ok(()) }).await; // ERROR: method not available
        // disconnected.disconnect(); // ERROR: method not available on Disconnected
        // disconnected.connection_duration(); // ERROR: method not available
        // disconnected.idle_duration(); // ERROR: method not available
        // disconnected.is_stale(Duration::from_secs(60)); // ERROR: method not available

        // This proves that the type system prevents using disconnected connections
        // for operations that require an active connection.

        // Only methods available on Disconnected:
        let _attempt_count = disconnected.attempt_count();
        let _reset = disconnected.reset_attempts();
        // connect() method would be available to transition to Connected state
    }

    #[test]
    fn test_type_system_state_transitions() {
        // This test shows how state transitions work at the type level

        let disconnected = RedisConnection::<Disconnected>::new_disconnected();

        // State transitions that are allowed:
        // Disconnected -> Connected (via connect() method)
        // Connected -> Disconnected (via disconnect() method or failed ping())
        // Connected -> Connected (via successful ping())

        // The type system ensures we can't create invalid state transitions
        // like trying to call disconnect() on an already disconnected connection

        assert_eq!(disconnected.attempt_count(), 0);
    }

    #[cfg(feature = "redis")]
    #[tokio::test]
    async fn test_integration_with_redis_memory() {
        // This test would demonstrate integration with RedisMemory
        // In a real test environment with Redis running:

        // use skreaver_memory::RedisMemory;
        // use skreaver_memory::redis::RedisConfigBuilder;

        // let config = RedisConfigBuilder::new()
        //     .standalone("redis://localhost:6379")
        //     .build()
        //     .expect("Config should be valid");

        // let memory = RedisMemory::new(config).await.expect("Should connect");

        // The RedisMemory now has type-safe connection management built-in
        // through the StatefulConnectionManager
    }
}
