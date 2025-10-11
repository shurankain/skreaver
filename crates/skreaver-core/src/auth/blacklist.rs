//! Token blacklist for JWT revocation
//!
//! This module provides token revocation functionality through blacklisting.
//! Revoked tokens are stored with TTL (time-to-live) equal to their remaining validity period.

#[cfg(feature = "redis")]
use super::AuthError;
use super::AuthResult;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait for token blacklist implementations
#[async_trait]
pub trait TokenBlacklist: Send + Sync {
    /// Add a token to the blacklist with TTL in seconds
    ///
    /// # Arguments
    ///
    /// * `jti` - JWT ID (unique token identifier)
    /// * `ttl_seconds` - Time to live in seconds (typically token expiration time - current time)
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if the blacklist operation fails.
    async fn revoke(&self, jti: &str, ttl_seconds: i64) -> AuthResult<()>;

    /// Check if a token is blacklisted
    ///
    /// # Arguments
    ///
    /// * `jti` - JWT ID to check
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if the blacklist check fails.
    async fn is_revoked(&self, jti: &str) -> AuthResult<bool>;

    /// Remove a token from the blacklist (for testing/cleanup)
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if the removal fails.
    async fn remove(&self, jti: &str) -> AuthResult<()>;

    /// Clear all blacklisted tokens (for testing only)
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if the clear operation fails.
    async fn clear(&self) -> AuthResult<()>;

    /// Count blacklisted tokens (for monitoring)
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if the count operation fails.
    async fn count(&self) -> AuthResult<usize>;
}

/// In-memory token blacklist (for testing and development)
///
/// **Warning**: This implementation does not persist across restarts and should
/// only be used for development and testing. Use `RedisBlacklist` in production.
#[derive(Clone)]
pub struct InMemoryBlacklist {
    // Use HashSet for O(1) lookups
    // Note: TTL is not enforced in memory (would require background task)
    tokens: Arc<RwLock<HashSet<String>>>,
}

impl InMemoryBlacklist {
    /// Create a new in-memory blacklist
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashSet::new())),
        }
    }
}

impl Default for InMemoryBlacklist {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenBlacklist for InMemoryBlacklist {
    async fn revoke(&self, jti: &str, _ttl_seconds: i64) -> AuthResult<()> {
        let mut tokens = self.tokens.write().await;
        tokens.insert(jti.to_string());
        Ok(())
    }

    async fn is_revoked(&self, jti: &str) -> AuthResult<bool> {
        let tokens = self.tokens.read().await;
        Ok(tokens.contains(jti))
    }

    async fn remove(&self, jti: &str) -> AuthResult<()> {
        let mut tokens = self.tokens.write().await;
        tokens.remove(jti);
        Ok(())
    }

    async fn clear(&self) -> AuthResult<()> {
        let mut tokens = self.tokens.write().await;
        tokens.clear();
        Ok(())
    }

    async fn count(&self) -> AuthResult<usize> {
        let tokens = self.tokens.read().await;
        Ok(tokens.len())
    }
}

/// Redis-based token blacklist (for production)
///
/// This implementation uses Redis with automatic TTL expiration.
/// Revoked tokens are automatically removed from Redis when they expire.
///
/// # Example
///
/// ```ignore
/// use skreaver_core::auth::RedisBlacklist;
///
/// let blacklist = RedisBlacklist::new("redis://localhost:6379").await?;
/// blacklist.revoke("token-jti", 3600).await?;
/// ```
#[cfg(feature = "redis")]
pub struct RedisBlacklist {
    client: redis::Client,
    key_prefix: String,
}

#[cfg(feature = "redis")]
impl RedisBlacklist {
    /// Create a new Redis-based blacklist
    ///
    /// # Arguments
    ///
    /// * `redis_url` - Redis connection URL (e.g., "redis://localhost:6379")
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if connection to Redis fails.
    pub fn new(redis_url: &str) -> AuthResult<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| AuthError::StorageError(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self {
            client,
            key_prefix: "skreaver:blacklist:".to_string(),
        })
    }

    /// Create with custom key prefix
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if connection to Redis fails.
    pub fn with_prefix(redis_url: &str, prefix: &str) -> AuthResult<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| AuthError::StorageError(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self {
            client,
            key_prefix: prefix.to_string(),
        })
    }

    /// Get full Redis key for a JTI
    fn get_key(&self, jti: &str) -> String {
        format!("{}{}", self.key_prefix, jti)
    }

    /// Get connection
    async fn get_connection(&self) -> AuthResult<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AuthError::StorageError(format!("Failed to get Redis connection: {}", e)))
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl TokenBlacklist for RedisBlacklist {
    async fn revoke(&self, jti: &str, ttl_seconds: i64) -> AuthResult<()> {
        use redis::AsyncCommands;

        let mut conn = self.get_connection().await?;
        let key = self.get_key(jti);

        // Store with TTL (value doesn't matter, we just check existence)
        let _: () = conn.set_ex(&key, "revoked", ttl_seconds as u64)
            .await
            .map_err(|e| AuthError::StorageError(format!("Failed to revoke token: {}", e)))?;

        Ok(())
    }

    async fn is_revoked(&self, jti: &str) -> AuthResult<bool> {
        use redis::AsyncCommands;

        let mut conn = self.get_connection().await?;
        let key = self.get_key(jti);

        conn.exists(&key)
            .await
            .map_err(|e| AuthError::StorageError(format!("Failed to check token: {}", e)))
    }

    async fn remove(&self, jti: &str) -> AuthResult<()> {
        use redis::AsyncCommands;

        let mut conn = self.get_connection().await?;
        let key = self.get_key(jti);

        let _: () = conn.del(key.as_str())
            .await
            .map_err(|e| AuthError::StorageError(format!("Failed to remove token: {}", e)))?;

        Ok(())
    }

    async fn clear(&self) -> AuthResult<()> {
        use redis::AsyncCommands;

        let mut conn = self.get_connection().await?;
        let pattern = format!("{}*", self.key_prefix);

        // Get all keys matching pattern
        let keys: Vec<String> = conn
            .keys(&pattern)
            .await
            .map_err(|e| AuthError::StorageError(format!("Failed to get keys: {}", e)))?;

        if !keys.is_empty() {
            let _: () = conn.del(keys)
                .await
                .map_err(|e| AuthError::StorageError(format!("Failed to clear tokens: {}", e)))?;
        }

        Ok(())
    }

    async fn count(&self) -> AuthResult<usize> {
        use redis::AsyncCommands;

        let mut conn = self.get_connection().await?;
        let pattern = format!("{}*", self.key_prefix);

        let keys: Vec<String> = conn
            .keys(&pattern)
            .await
            .map_err(|e| AuthError::StorageError(format!("Failed to count tokens: {}", e)))?;

        Ok(keys.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_blacklist_revoke_and_check() {
        let blacklist = InMemoryBlacklist::new();

        // Token should not be revoked initially
        assert!(!blacklist.is_revoked("token-1").await.unwrap());

        // Revoke token
        blacklist.revoke("token-1", 3600).await.unwrap();

        // Token should now be revoked
        assert!(blacklist.is_revoked("token-1").await.unwrap());

        // Other tokens should not be affected
        assert!(!blacklist.is_revoked("token-2").await.unwrap());
    }

    #[tokio::test]
    async fn test_in_memory_blacklist_remove() {
        let blacklist = InMemoryBlacklist::new();

        // Revoke token
        blacklist.revoke("token-1", 3600).await.unwrap();
        assert!(blacklist.is_revoked("token-1").await.unwrap());

        // Remove token
        blacklist.remove("token-1").await.unwrap();
        assert!(!blacklist.is_revoked("token-1").await.unwrap());
    }

    #[tokio::test]
    async fn test_in_memory_blacklist_clear() {
        let blacklist = InMemoryBlacklist::new();

        // Revoke multiple tokens
        blacklist.revoke("token-1", 3600).await.unwrap();
        blacklist.revoke("token-2", 3600).await.unwrap();
        blacklist.revoke("token-3", 3600).await.unwrap();

        assert_eq!(blacklist.count().await.unwrap(), 3);

        // Clear all
        blacklist.clear().await.unwrap();
        assert_eq!(blacklist.count().await.unwrap(), 0);

        // All tokens should be un-revoked
        assert!(!blacklist.is_revoked("token-1").await.unwrap());
        assert!(!blacklist.is_revoked("token-2").await.unwrap());
        assert!(!blacklist.is_revoked("token-3").await.unwrap());
    }

    #[tokio::test]
    async fn test_in_memory_blacklist_count() {
        let blacklist = InMemoryBlacklist::new();

        assert_eq!(blacklist.count().await.unwrap(), 0);

        blacklist.revoke("token-1", 3600).await.unwrap();
        assert_eq!(blacklist.count().await.unwrap(), 1);

        blacklist.revoke("token-2", 3600).await.unwrap();
        assert_eq!(blacklist.count().await.unwrap(), 2);

        // Revoking same token shouldn't increase count
        blacklist.revoke("token-1", 3600).await.unwrap();
        assert_eq!(blacklist.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_blacklist_clone() {
        let blacklist1 = InMemoryBlacklist::new();
        blacklist1.revoke("token-1", 3600).await.unwrap();

        // Clone shares the same underlying storage
        let blacklist2 = blacklist1.clone();
        assert!(blacklist2.is_revoked("token-1").await.unwrap());

        // Changes in one affect the other
        blacklist2.revoke("token-2", 3600).await.unwrap();
        assert!(blacklist1.is_revoked("token-2").await.unwrap());
    }
}
