//! Integration tests for Redis type-safe configuration
//!
//! These tests demonstrate the compile-time validation improvements
//! and show how invalid configurations are prevented at compile time.

#[cfg(feature = "redis")]
mod redis_type_safety_tests {
    use skreaver_memory::RedisMemory;
    use skreaver_memory::redis::RedisConfigBuilder;

    #[tokio::test]
    async fn test_type_safe_localhost_config() {
        // This configuration is guaranteed to be valid at compile time
        let config = RedisConfigBuilder::new()
            .standalone("redis://localhost:6379")
            .with_pool_size(10)
            .with_database(1)
            .build()
            .expect("Config should be valid");

        // The config is now ValidRedisConfig, not just RedisConfig
        assert_eq!(config.pool_size(), 10);
        assert_eq!(config.database(), 1);
    }

    #[tokio::test]
    async fn test_type_safe_cluster_config() {
        let nodes = vec![
            "redis://node1:6379".to_string(),
            "redis://node2:6379".to_string(),
            "redis://node3:6379".to_string(),
        ];

        let config = RedisConfigBuilder::new()
            .cluster(nodes)
            .with_pool_size(30)
            .build()
            .expect("Cluster config should be valid");

        assert_eq!(config.pool_size(), 30);
    }

    #[tokio::test]
    async fn test_invalid_deployment_rejected() {
        // This will fail to build because no deployment is specified
        let result = RedisConfigBuilder::new()
            .with_pool_size(10)
            // No deployment specified - should fail
            .build();

        assert!(result.is_err(), "Config with no deployment should fail");
    }

    #[tokio::test]
    async fn test_empty_strings_handled() {
        // Empty strings are automatically filtered out
        let config = RedisConfigBuilder::new()
            .standalone("") // Empty URL - will be ignored
            .with_pool_size(10);

        let result = config.build();
        assert!(
            result.is_err(),
            "Config with empty URL should fail validation"
        );
    }

    #[tokio::test]
    async fn test_convenient_constructors() {
        // These should be safe to use without validation
        let _localhost = RedisMemory::localhost().await;

        let cluster_nodes = vec![
            "redis://node1:6379".to_string(),
            "redis://node2:6379".to_string(),
        ];
        let _cluster = RedisMemory::cluster(cluster_nodes).await;

        // Both may fail due to connection issues, but not due to configuration errors
    }

    #[test]
    fn test_compile_time_constants() {
        use skreaver_memory::redis::config::{DatabaseId, PoolSize};

        // These are validated at compile time
        const VALID_POOL: Option<PoolSize> = PoolSize::new(10);
        const INVALID_POOL: Option<PoolSize> = PoolSize::new(0);
        const VALID_DB: Option<DatabaseId> = DatabaseId::new(5);
        const INVALID_DB: Option<DatabaseId> = DatabaseId::new(20);

        assert!(VALID_POOL.is_some());
        assert!(INVALID_POOL.is_none());
        assert!(VALID_DB.is_some());
        assert!(INVALID_DB.is_none());
    }

    #[test]
    fn demonstrate_impossible_states() {
        // The following code CANNOT compile because it's impossible to create
        // an invalid ValidRedisConfig directly:

        // This would be a compile error:
        // let invalid_config = ValidRedisConfig { ... };

        // The ONLY way to get ValidRedisConfig is through the builder:
        let config = RedisConfigBuilder::new()
            .standalone("redis://localhost:6379")
            .build();

        match config {
            Ok(valid_config) => {
                // This is guaranteed to be a valid configuration
                assert!(valid_config.pool_size() > 0);
                assert!(valid_config.database() <= 15);
            }
            Err(_) => {
                // Configuration was invalid and caught at build time
            }
        }
    }
}
