//! API Key authentication for service-to-service communication

use super::{AuthError, AuthMethod, AuthResult, Principal};
use crate::auth::rbac::Role;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// API Key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    /// Minimum key length
    pub min_length: usize,
    /// Key prefix (e.g., "sk_" for secret keys)
    pub prefix: String,
    /// Default expiration duration in days (None for no expiration)
    pub default_expiry_days: Option<i64>,
    /// Allow key rotation
    pub allow_rotation: bool,
    /// Maximum keys per principal
    pub max_keys_per_principal: usize,
}

impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self {
            min_length: 32,
            prefix: "sk_".to_string(),
            default_expiry_days: Some(365),
            allow_rotation: true,
            max_keys_per_principal: 5,
        }
    }
}

/// API Key metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// The actual key value (shown only once)
    pub key: String,
    /// Key identifier (for management)
    pub id: String,
    /// Key name/description
    pub name: String,
    /// Associated principal ID
    pub principal_id: String,
    /// Assigned roles
    pub roles: Vec<Role>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,
    /// Last used timestamp
    pub last_used_at: Option<DateTime<Utc>>,
    /// Is the key active?
    pub active: bool,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ApiKey {
    /// Check if the key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Check if the key is valid
    pub fn is_valid(&self) -> bool {
        self.active && !self.is_expired()
    }

    /// Generate a secure random key
    fn generate_key(prefix: &str, length: usize) -> String {
        use base64::{Engine as _, engine::general_purpose};
        use rand::TryRngCore;

        let mut random_bytes = vec![0u8; length];
        // Use try_fill_bytes which is available in rand 0.9
        rand::rngs::OsRng
            .try_fill_bytes(&mut random_bytes)
            .expect("Failed to generate random bytes");

        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(random_bytes);

        format!("{}{}", prefix, &encoded[..length.min(encoded.len())])
    }
}

/// API Key store for managing keys
#[derive(Clone)]
struct ApiKeyStore {
    /// Map of key hash to API key metadata
    keys: Arc<RwLock<HashMap<String, ApiKey>>>,
    /// Map of principal ID to their key IDs
    principal_keys: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl ApiKeyStore {
    fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            principal_keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store a new API key
    async fn store(&self, key_hash: String, api_key: ApiKey) {
        let mut keys = self.keys.write().await;
        let mut principal_keys = self.principal_keys.write().await;

        keys.insert(key_hash.clone(), api_key.clone());

        principal_keys
            .entry(api_key.principal_id.clone())
            .or_insert_with(Vec::new)
            .push(api_key.id.clone());
    }

    /// Get an API key by its hash
    async fn get(&self, key_hash: &str) -> Option<ApiKey> {
        let keys = self.keys.read().await;
        keys.get(key_hash).cloned()
    }

    /// Update last used timestamp
    async fn update_last_used(&self, key_hash: &str) {
        let mut keys = self.keys.write().await;
        if let Some(key) = keys.get_mut(key_hash) {
            key.last_used_at = Some(Utc::now());
        }
    }

    /// Revoke a key
    async fn revoke(&self, key_hash: &str) -> bool {
        let mut keys = self.keys.write().await;
        if let Some(key) = keys.get_mut(key_hash) {
            key.active = false;
            true
        } else {
            false
        }
    }

    /// Get all keys for a principal
    async fn get_principal_keys(&self, principal_id: &str) -> Vec<ApiKey> {
        let keys = self.keys.read().await;
        let principal_keys = self.principal_keys.read().await;

        if let Some(key_ids) = principal_keys.get(principal_id) {
            key_ids
                .iter()
                .filter_map(|id| keys.values().find(|k| &k.id == id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }
}

/// API Key manager
pub struct ApiKeyManager {
    config: ApiKeyConfig,
    store: ApiKeyStore,
}

impl ApiKeyManager {
    /// Create a new API key manager
    pub fn new(config: ApiKeyConfig) -> Self {
        Self {
            config,
            store: ApiKeyStore::new(),
        }
    }

    /// Generate a new API key
    pub async fn generate(&self, name: String, roles: Vec<Role>) -> AuthResult<ApiKey> {
        let key = ApiKey::generate_key(&self.config.prefix, self.config.min_length);
        let key_hash = self.hash_key(&key);

        let expires_at = self
            .config
            .default_expiry_days
            .map(|days| Utc::now() + Duration::days(days));

        let api_key = ApiKey {
            key: key.clone(),
            id: uuid::Uuid::new_v4().to_string(),
            name,
            principal_id: uuid::Uuid::new_v4().to_string(), // Generate new principal
            roles,
            created_at: Utc::now(),
            expires_at,
            last_used_at: None,
            active: true,
            metadata: HashMap::new(),
        };

        self.store.store(key_hash, api_key.clone()).await;

        Ok(api_key)
    }

    /// Authenticate with an API key
    pub async fn authenticate(&self, key: &str) -> AuthResult<Principal> {
        // Validate key format
        if !key.starts_with(&self.config.prefix) {
            return Err(AuthError::InvalidCredentials);
        }

        if key.len() < self.config.min_length + self.config.prefix.len() {
            return Err(AuthError::InvalidCredentials);
        }

        let key_hash = self.hash_key(key);

        // Get the key from store
        let api_key = self
            .store
            .get(&key_hash)
            .await
            .ok_or(AuthError::ApiKeyNotFound)?;

        // Validate the key
        if !api_key.is_valid() {
            if api_key.is_expired() {
                return Err(AuthError::TokenExpired);
            }
            return Err(AuthError::InvalidCredentials);
        }

        // Update last used timestamp
        self.store.update_last_used(&key_hash).await;

        // Create principal
        let mut principal = Principal::new(
            api_key.principal_id.clone(),
            api_key.name.clone(),
            AuthMethod::ApiKey(api_key.id.clone()),
        );

        // Add roles
        for role in api_key.roles {
            principal = principal.with_role(role);
        }

        // Add metadata
        principal = principal.with_metadata("api_key_id".to_string(), api_key.id);
        principal =
            principal.with_metadata("created_at".to_string(), api_key.created_at.to_rfc3339());

        Ok(principal)
    }

    /// Revoke an API key
    pub async fn revoke(&self, key: &str) -> AuthResult<()> {
        let key_hash = self.hash_key(key);

        if self.store.revoke(&key_hash).await {
            Ok(())
        } else {
            Err(AuthError::ApiKeyNotFound)
        }
    }

    /// Rotate an API key
    pub async fn rotate(&self, old_key: &str) -> AuthResult<ApiKey> {
        if !self.config.allow_rotation {
            return Err(AuthError::ValidationError(
                "Key rotation not allowed".to_string(),
            ));
        }

        // Authenticate with old key first
        let _principal = self.authenticate(old_key).await?;

        // Get the old key metadata
        let key_hash = self.hash_key(old_key);
        let old_api_key = self
            .store
            .get(&key_hash)
            .await
            .ok_or(AuthError::ApiKeyNotFound)?;

        // Revoke old key
        self.revoke(old_key).await?;

        // Generate new key with same roles
        let new_api_key = self
            .generate(format!("{} (rotated)", old_api_key.name), old_api_key.roles)
            .await?;

        Ok(new_api_key)
    }

    /// List all keys for a principal
    pub async fn list_keys(&self, principal_id: &str) -> Vec<ApiKey> {
        self.store.get_principal_keys(principal_id).await
    }

    /// Hash a key for storage
    fn hash_key(&self, key: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_generation() {
        let manager = ApiKeyManager::new(ApiKeyConfig::default());
        let key = manager
            .generate("Test Key".to_string(), vec![Role::Agent])
            .await
            .unwrap();

        assert!(key.key.starts_with("sk_"));
        assert!(key.key.len() >= 32);
        assert_eq!(key.name, "Test Key");
        assert!(key.is_valid());
    }

    #[tokio::test]
    async fn test_api_key_authentication() {
        let manager = ApiKeyManager::new(ApiKeyConfig::default());
        let key = manager
            .generate("Test Key".to_string(), vec![Role::Agent])
            .await
            .unwrap();

        let principal = manager.authenticate(&key.key).await.unwrap();
        assert_eq!(principal.name, "Test Key");
        assert!(principal.has_role(&Role::Agent));
    }

    #[tokio::test]
    async fn test_api_key_revocation() {
        let manager = ApiKeyManager::new(ApiKeyConfig::default());
        let key = manager
            .generate("Test Key".to_string(), vec![Role::Agent])
            .await
            .unwrap();

        // Key should work before revocation
        assert!(manager.authenticate(&key.key).await.is_ok());

        // Revoke the key
        manager.revoke(&key.key).await.unwrap();

        // Key should not work after revocation
        assert!(manager.authenticate(&key.key).await.is_err());
    }
}
