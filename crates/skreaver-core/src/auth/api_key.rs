//! API Key authentication for service-to-service communication

use super::{AuthError, AuthMethod, AuthResult, Principal};
use crate::auth::rbac::Role;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;
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

// ============================================================================
// Typestate Pattern Markers for API Keys
// ============================================================================

/// Marker for active API key state
#[derive(Debug, Clone, Copy)]
pub struct Active;

/// Marker for expired API key state
#[derive(Debug, Clone, Copy)]
pub struct Expired;

/// Marker for revoked API key state
#[derive(Debug, Clone, Copy)]
pub struct Revoked;

/// Type-safe API Key with state encoded in type system
///
/// # Security
///
/// The key value is stored in a [`SecretString`] which prevents it from being
/// accidentally logged or serialized. Use [`expose_key()`](Key::expose_key) to
/// access the actual key value.
#[derive(Debug, Clone)]
pub struct Key<S> {
    /// The actual key value - PROTECTED from logging/serialization
    key: crate::security::SecretString,
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
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// State marker
    _state: PhantomData<S>,
}

impl<S> Key<S> {
    /// Expose the secret key value for authentication
    ///
    /// # Security
    ///
    /// This is the ONLY way to access the secret key value. The method name is
    /// intentionally verbose to make secret access obvious during code review.
    ///
    /// The exposed value should:
    /// - NEVER be logged
    /// - NEVER be included in error messages
    /// - NEVER be stored in non-secret data structures
    /// - Be used immediately for authentication and not stored
    pub fn expose_key(&self) -> &str {
        self.key.expose_as_str()
    }

    /// Get the key value (deprecated - use expose_key)
    ///
    /// # Deprecated
    ///
    /// Use [`expose_key()`](Key::expose_key) instead for explicit secret access.
    #[deprecated(since = "0.6.0", note = "Use expose_key() for explicit secret access")]
    pub fn key(&self) -> &str {
        self.expose_key()
    }

    /// Get the key ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the key name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the principal ID
    pub fn principal_id(&self) -> &str {
        &self.principal_id
    }

    /// Get assigned roles
    pub fn roles(&self) -> &[Role] {
        &self.roles
    }

    /// Get creation timestamp
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Get expiration timestamp
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }

    /// Get last used timestamp
    pub fn last_used_at(&self) -> Option<DateTime<Utc>> {
        self.last_used_at
    }

    /// Get metadata
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}

impl Key<Active> {
    /// Create a new active API key
    pub fn new(
        key: String,
        id: String,
        name: String,
        principal_id: String,
        roles: Vec<Role>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            key: crate::security::SecretString::from_string(key),
            id,
            name,
            principal_id,
            roles,
            created_at: Utc::now(),
            expires_at,
            last_used_at: None,
            metadata: HashMap::new(),
            _state: PhantomData,
        }
    }

    /// Check if the key is expired and transition to expired state if so
    pub fn check_expiration(self) -> Result<Key<Active>, Box<Key<Expired>>> {
        if let Some(expires_at) = self.expires_at
            && Utc::now() > expires_at
        {
            return Err(Box::new(Key {
                key: self.key,
                id: self.id,
                name: self.name,
                principal_id: self.principal_id,
                roles: self.roles,
                created_at: self.created_at,
                expires_at: self.expires_at,
                last_used_at: self.last_used_at,
                metadata: self.metadata,
                _state: PhantomData,
            }));
        }
        Ok(self)
    }

    /// Check if the key is valid (active state means it's valid)
    pub fn is_valid(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() <= expires_at
        } else {
            true
        }
    }

    /// Update last used timestamp
    pub fn mark_used(mut self) -> Self {
        self.last_used_at = Some(Utc::now());
        self
    }

    /// Revoke the key
    pub fn revoke(self) -> Key<Revoked> {
        Key {
            key: self.key,
            id: self.id,
            name: self.name,
            principal_id: self.principal_id,
            roles: self.roles,
            created_at: self.created_at,
            expires_at: self.expires_at,
            last_used_at: self.last_used_at,
            metadata: self.metadata,
            _state: PhantomData,
        }
    }

    /// Generate a secure random key
    fn generate_key_value(prefix: &str, length: usize) -> String {
        use base64::{Engine as _, engine::general_purpose};
        use rand::TryRngCore;

        let mut random_bytes = vec![0u8; length];
        rand::rngs::OsRng
            .try_fill_bytes(&mut random_bytes)
            .expect("Failed to generate random bytes");

        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(random_bytes);

        format!("{}{}", prefix, &encoded[..length.min(encoded.len())])
    }
}

impl Key<Expired> {
    /// Get expiration timestamp (guaranteed to be Some for expired keys)
    pub fn expiration_time(&self) -> DateTime<Utc> {
        self.expires_at
            .expect("Expired key must have expiration timestamp")
    }

    /// Check how long ago the key expired
    pub fn expired_duration(&self) -> Duration {
        Utc::now() - self.expiration_time()
    }

    /// Cannot use expired key - this method documents the compile-time guarantee
    pub fn is_valid(&self) -> bool {
        false
    }

    /// Can still revoke an expired key
    pub fn revoke(self) -> Key<Revoked> {
        Key {
            key: self.key,
            id: self.id,
            name: self.name,
            principal_id: self.principal_id,
            roles: self.roles,
            created_at: self.created_at,
            expires_at: self.expires_at,
            last_used_at: self.last_used_at,
            metadata: self.metadata,
            _state: PhantomData,
        }
    }
}

impl Key<Revoked> {
    /// Get revocation reason from metadata if present
    pub fn revocation_reason(&self) -> Option<&str> {
        self.metadata.get("revocation_reason").map(|s| s.as_str())
    }

    /// Check if key is valid (always false for revoked keys)
    pub fn is_valid(&self) -> bool {
        false
    }

    /// Revoked keys cannot be used
    pub fn is_revoked(&self) -> bool {
        true
    }
}

/// Backward-compatible API Key metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// The actual key value - PROTECTED from logging/serialization
    key: crate::security::SecretString,
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

// Conversion traits for backward compatibility
impl From<Key<Active>> for ApiKey {
    fn from(key: Key<Active>) -> Self {
        Self {
            key: key.key,
            id: key.id,
            name: key.name,
            principal_id: key.principal_id,
            roles: key.roles,
            created_at: key.created_at,
            expires_at: key.expires_at,
            last_used_at: key.last_used_at,
            active: true,
            metadata: key.metadata,
        }
    }
}

impl From<Key<Expired>> for ApiKey {
    fn from(key: Key<Expired>) -> Self {
        Self {
            key: key.key,
            id: key.id,
            name: key.name,
            principal_id: key.principal_id,
            roles: key.roles,
            created_at: key.created_at,
            expires_at: key.expires_at,
            last_used_at: key.last_used_at,
            active: false,
            metadata: key.metadata,
        }
    }
}

impl From<Key<Revoked>> for ApiKey {
    fn from(key: Key<Revoked>) -> Self {
        Self {
            key: key.key,
            id: key.id,
            name: key.name,
            principal_id: key.principal_id,
            roles: key.roles,
            created_at: key.created_at,
            expires_at: key.expires_at,
            last_used_at: key.last_used_at,
            active: false,
            metadata: key.metadata,
        }
    }
}

impl TryFrom<ApiKey> for Key<Active> {
    type Error = AuthError;

    fn try_from(api_key: ApiKey) -> Result<Self, Self::Error> {
        if !api_key.active {
            return Err(AuthError::InvalidCredentials);
        }

        if let Some(expires_at) = api_key.expires_at
            && Utc::now() > expires_at
        {
            return Err(AuthError::TokenExpired);
        }

        Ok(Key {
            key: api_key.key,
            id: api_key.id,
            name: api_key.name,
            principal_id: api_key.principal_id,
            roles: api_key.roles,
            created_at: api_key.created_at,
            expires_at: api_key.expires_at,
            last_used_at: api_key.last_used_at,
            metadata: api_key.metadata,
            _state: PhantomData,
        })
    }
}

impl ApiKey {
    /// Expose the secret key value for authentication
    ///
    /// # Security
    ///
    /// This is the ONLY way to access the secret key value.
    pub fn expose_key(&self) -> &str {
        self.key.expose_as_str()
    }

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

    /// Generate a new API key (type-safe version)
    pub async fn generate_key(&self, name: String, roles: Vec<Role>) -> AuthResult<Key<Active>> {
        let key_value =
            Key::<Active>::generate_key_value(&self.config.prefix, self.config.min_length);
        let key_hash = self.hash_key(&key_value);

        let expires_at = self
            .config
            .default_expiry_days
            .map(|days| Utc::now() + Duration::days(days));

        let active_key = Key::<Active>::new(
            key_value,
            uuid::Uuid::new_v4().to_string(),
            name,
            uuid::Uuid::new_v4().to_string(), // Generate new principal
            roles,
            expires_at,
        );

        // Store as ApiKey for compatibility
        let api_key: ApiKey = active_key.clone().into();
        self.store.store(key_hash, api_key).await;

        Ok(active_key)
    }

    /// Generate a new API key (backward compatible version)
    pub async fn generate(&self, name: String, roles: Vec<Role>) -> AuthResult<ApiKey> {
        let active_key = self.generate_key(name, roles).await?;
        Ok(active_key.into())
    }

    /// Authenticate with an API key (type-safe version)
    pub async fn authenticate_with_key(
        &self,
        key_str: &str,
    ) -> AuthResult<(Key<Active>, Principal)> {
        // Validate key format
        if !key_str.starts_with(&self.config.prefix) {
            return Err(AuthError::InvalidCredentials);
        }

        if key_str.len() < self.config.min_length + self.config.prefix.len() {
            return Err(AuthError::InvalidCredentials);
        }

        let key_hash = self.hash_key(key_str);

        // Get the key from store
        let api_key = self
            .store
            .get(&key_hash)
            .await
            .ok_or(AuthError::ApiKeyNotFound)?;

        // Convert to type-safe key and check state
        let active_key: Key<Active> = api_key.try_into()?;

        // Check expiration (returns Result<Key<Active>, Key<Expired>>)
        let valid_key = active_key
            .check_expiration()
            .map_err(|_expired_key| AuthError::TokenExpired)?;

        // Update last used timestamp
        self.store.update_last_used(&key_hash).await;
        let used_key = valid_key.mark_used();

        // Create principal
        let mut principal = Principal::new(
            used_key.principal_id.clone(),
            used_key.name.clone(),
            AuthMethod::ApiKey(used_key.id.clone()),
        );

        // Add roles
        for role in &used_key.roles {
            principal = principal.with_role(role.clone());
        }

        // Add metadata
        principal = principal.with_metadata("api_key_id".to_string(), used_key.id.clone());
        principal =
            principal.with_metadata("created_at".to_string(), used_key.created_at.to_rfc3339());

        Ok((used_key, principal))
    }

    /// Authenticate with an API key (backward compatible version)
    pub async fn authenticate(&self, key: &str) -> AuthResult<Principal> {
        let (_key, principal) = self.authenticate_with_key(key).await?;
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

    /// Rotate an API key (type-safe version)
    pub async fn rotate_key(&self, old_key: &str) -> AuthResult<Key<Active>> {
        if !self.config.allow_rotation {
            return Err(AuthError::ValidationError(
                "Key rotation not allowed".to_string(),
            ));
        }

        // Authenticate with old key first (verifies it's valid and active)
        let (active_key, _principal) = self.authenticate_with_key(old_key).await?;

        // Revoke old key (transition to Revoked state)
        let _revoked_key = active_key.revoke();

        // Persist the revocation
        self.revoke(old_key).await?;

        // Generate new key with same roles
        let new_key = self
            .generate_key(
                format!("{} (rotated)", _revoked_key.name),
                _revoked_key.roles,
            )
            .await?;

        Ok(new_key)
    }

    /// Rotate an API key (backward compatible version)
    pub async fn rotate(&self, old_key: &str) -> AuthResult<ApiKey> {
        let new_key = self.rotate_key(old_key).await?;
        Ok(new_key.into())
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

        let key_str = key.expose_key();
        assert!(key_str.starts_with("sk_"));
        assert!(key_str.len() >= 32);
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

        let principal = manager.authenticate(key.expose_key()).await.unwrap();
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
        assert!(manager.authenticate(key.expose_key()).await.is_ok());

        // Revoke the key
        manager.revoke(key.expose_key()).await.unwrap();

        // Key should not work after revocation
        assert!(manager.authenticate(key.expose_key()).await.is_err());
    }
}
