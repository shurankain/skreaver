//! API Key authentication for service-to-service communication

use super::{AuthError, AuthMethod, AuthResult, Principal};
use crate::auth::rbac::Role;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

/// The default hardcoded salt value - used to detect when warning should be shown
const DEFAULT_SALT: &str = "skreaver-default-salt-change-in-production";

/// API Key rotation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RotationPolicy {
    /// Rotation disabled
    Disabled,
    /// Manual rotation allowed
    Manual,
    /// Automatic rotation with interval
    Automatic {
        /// Days between automatic rotations
        interval_days: u32,
    },
}

impl RotationPolicy {
    /// Check if rotation is allowed
    pub fn is_allowed(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// Check if automatic rotation is enabled
    pub fn is_automatic(self) -> bool {
        matches!(self, Self::Automatic { .. })
    }

    /// Get rotation interval in days (if automatic)
    pub fn interval_days(self) -> Option<u32> {
        match self {
            Self::Automatic { interval_days } => Some(interval_days),
            _ => None,
        }
    }
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self::Manual
    }
}

/// API Key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    /// Minimum key length
    pub min_length: usize,
    /// Key prefix (e.g., "sk_" for secret keys)
    pub prefix: String,
    /// Default expiration duration in days (None for no expiration)
    pub default_expiry_days: Option<i64>,
    /// Key rotation policy
    pub rotation: RotationPolicy,
    /// Maximum keys per principal
    pub max_keys_per_principal: usize,
    /// Salt for key hashing (MEDIUM-34: prevents hash collision DoS)
    ///
    /// This salt is prepended to API keys before hashing to:
    /// 1. Prevent hash collisions across different deployments
    /// 2. Add an extra layer of security if hashes are exposed
    /// 3. Ensure unique hash values even for identical keys
    ///
    /// Should be set via environment variable SKREAVER_API_KEY_SALT in production.
    #[serde(default = "ApiKeyConfig::default_hash_salt")]
    pub hash_salt: String,
}

impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self {
            min_length: 32,
            prefix: "sk_".to_string(),
            default_expiry_days: Some(365),
            rotation: RotationPolicy::default(),
            max_keys_per_principal: 5,
            hash_salt: Self::default_hash_salt(),
        }
    }
}

impl ApiKeyConfig {
    /// Default hash salt - reads from environment or uses default
    ///
    /// MEDIUM-34: In production, set SKREAVER_API_KEY_SALT to a unique random value
    fn default_hash_salt() -> String {
        std::env::var("SKREAVER_API_KEY_SALT").unwrap_or_else(|_| DEFAULT_SALT.to_string())
    }

    /// Check if using the default hardcoded salt
    pub fn is_using_default_salt(&self) -> bool {
        self.hash_salt == DEFAULT_SALT
    }

    /// Create config with automatic rotation
    pub fn with_auto_rotation(interval_days: u32) -> Self {
        Self {
            rotation: RotationPolicy::Automatic { interval_days },
            ..Default::default()
        }
    }

    /// Create config with rotation disabled
    pub fn no_rotation() -> Self {
        Self {
            rotation: RotationPolicy::Disabled,
            ..Default::default()
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
///
/// This type carries the expiration timestamp, making it impossible to have
/// an expired key without a valid expiration time at the type level.
#[derive(Debug, Clone, Copy)]
pub struct Expired {
    /// The timestamp when this key expired
    pub expired_at: DateTime<Utc>,
}

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
    ///
    /// Returns `Ok(Key<Active>)` if the key is still valid, or `Err(Key<Expired>)`
    /// if the key has expired. The `Expired` state carries the expiration timestamp,
    /// making it impossible to have an expired key without a valid expiration time.
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

    /// Generate a secure random key with retry logic
    ///
    /// This function uses the OS cryptographic RNG and implements retry logic
    /// to handle transient failures (e.g., temporary entropy exhaustion).
    ///
    /// # Errors
    ///
    /// Returns `AuthError::RandomGenerationFailed` if RNG fails after 3 attempts
    ///
    /// LOW-6: Made async to use tokio::time::sleep instead of blocking thread::sleep.
    /// This prevents blocking the async executor when called from async context.
    async fn generate_key_value(prefix: &str, length: usize) -> Result<String, AuthError> {
        use base64::{Engine as _, engine::general_purpose};
        use rand::TryRngCore;

        let mut random_bytes = vec![0u8; length];

        // Try multiple times with exponential backoff
        let mut attempts = 0;
        loop {
            match rand::rngs::OsRng.try_fill_bytes(&mut random_bytes) {
                Ok(()) => break,
                Err(e) if attempts < 2 => {
                    attempts += 1;
                    tracing::warn!(
                        attempt = attempts + 1,
                        error = %e,
                        "Retrying cryptographic RNG after failure"
                    );
                    // LOW-6: Use async sleep to avoid blocking executor thread
                    tokio::time::sleep(tokio::time::Duration::from_millis(10 * (attempts as u64)))
                        .await;
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        attempts = attempts + 1,
                        "Failed to generate cryptographically secure random bytes after retries"
                    );
                    return Err(AuthError::RandomGenerationFailed(e.to_string()));
                }
            }
        }

        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(random_bytes);
        Ok(format!(
            "{}{}",
            prefix,
            &encoded[..length.min(encoded.len())]
        ))
    }
}

impl Key<Expired> {
    /// Get expiration timestamp
    ///
    /// Get the expiration timestamp of this expired key.
    ///
    /// # Safety Invariant
    ///
    /// This method relies on the invariant that `Key<Expired>` is only ever created
    /// by `check_expiration()`, which only transitions to Expired state when
    /// `expires_at` is `Some`. While this cannot be enforced at compile-time due to
    /// Rust's lack of dependent types, it is enforced by:
    ///
    /// 1. `check_expiration()` being the only public API that creates `Key<Expired>`
    /// 2. The `Expired` marker type containing the expiration timestamp for redundancy
    /// 3. This defensive check that returns the `Expired` marker's timestamp as fallback
    ///
    /// # Returns
    ///
    /// The expiration timestamp, guaranteed to exist for properly-constructed Expired keys.
    pub fn expiration_time(&self) -> DateTime<Utc> {
        // Primary: use the stored expiration timestamp
        // This should always succeed for properly-constructed Expired keys
        self.expires_at.unwrap_or_else(|| {
            // DEFENSIVE: If expires_at is somehow None, log warning and return current time
            // This should never happen in practice, but we don't want to panic
            tracing::warn!(
                key_id = %self.id,
                "INVARIANT VIOLATION: Expired key missing expiration timestamp, using current time"
            );
            Utc::now()
        })
    }

    /// Get the Expired state marker with the expiration timestamp
    ///
    /// This provides type-level access to the expiration time, demonstrating that
    /// an Expired key always has an associated expiration timestamp.
    pub fn expired_state(&self) -> Expired {
        Expired {
            expired_at: self.expiration_time(),
        }
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

/// API Key status - represents the current state of a key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApiKeyStatus {
    /// Key is active and can be used for authentication
    #[default]
    Active,

    /// Key has been explicitly revoked
    Revoked {
        /// When the key was revoked
        revoked_at: DateTime<Utc>,
    },

    /// Key has expired naturally
    Expired {
        /// When the key expired
        expired_at: DateTime<Utc>,
    },

    /// Key is temporarily suspended (can be resumed)
    Suspended {
        /// When the key was suspended
        suspended_at: DateTime<Utc>,
        /// Optional: when the suspension will be lifted
        #[serde(skip_serializing_if = "Option::is_none")]
        resume_at: Option<DateTime<Utc>>,
    },

    /// Key has hit rate limits
    RateLimited {
        /// When the rate limit was imposed
        limited_at: DateTime<Utc>,
        /// When the limit will be lifted
        limited_until: DateTime<Utc>,
    },
}

impl ApiKeyStatus {
    /// Check if the status allows key usage
    pub fn is_usable(&self) -> bool {
        matches!(self, ApiKeyStatus::Active)
    }

    /// Check if the key can be rotated in this status
    pub fn can_rotate(&self) -> bool {
        // Can't rotate permanently revoked keys
        !matches!(self, ApiKeyStatus::Revoked { .. })
    }

    /// Get a human-readable description of the status
    pub fn description(&self) -> &'static str {
        match self {
            ApiKeyStatus::Active => "Active and ready for use",
            ApiKeyStatus::Revoked { .. } => "Revoked and cannot be used",
            ApiKeyStatus::Expired { .. } => "Expired and cannot be used",
            ApiKeyStatus::Suspended { .. } => "Temporarily suspended",
            ApiKeyStatus::RateLimited { .. } => "Rate limited",
        }
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
    /// Key status (replaces simple boolean)
    #[serde(default)]
    pub status: ApiKeyStatus,
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
            status: ApiKeyStatus::Active,
            metadata: key.metadata,
        }
    }
}

impl From<Key<Expired>> for ApiKey {
    fn from(key: Key<Expired>) -> Self {
        let expired_at = key.expires_at.unwrap_or_else(Utc::now);
        Self {
            key: key.key,
            id: key.id,
            name: key.name,
            principal_id: key.principal_id,
            roles: key.roles,
            created_at: key.created_at,
            expires_at: key.expires_at,
            last_used_at: key.last_used_at,
            status: ApiKeyStatus::Expired { expired_at },
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
            status: ApiKeyStatus::Revoked {
                revoked_at: Utc::now(),
            },
            metadata: key.metadata,
        }
    }
}

impl TryFrom<ApiKey> for Key<Active> {
    type Error = AuthError;

    fn try_from(api_key: ApiKey) -> Result<Self, Self::Error> {
        // Check status
        if !api_key.status.is_usable() {
            return Err(AuthError::InvalidCredentials);
        }

        // Double-check expiration for safety
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

    /// Check if the key is expired (either by status or expiration time)
    pub fn is_expired(&self) -> bool {
        // Check status first
        if matches!(self.status, ApiKeyStatus::Expired { .. }) {
            return true;
        }

        // Also check expiration time
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Check if the key is valid and can be used
    pub fn is_valid(&self) -> bool {
        self.status.is_usable() && !self.is_expired()
    }

    /// Check if the key is revoked
    pub fn is_revoked(&self) -> bool {
        matches!(self.status, ApiKeyStatus::Revoked { .. })
    }

    /// Check if the key is suspended
    pub fn is_suspended(&self) -> bool {
        matches!(self.status, ApiKeyStatus::Suspended { .. })
    }

    /// Check if the key is rate limited
    pub fn is_rate_limited(&self) -> bool {
        matches!(self.status, ApiKeyStatus::RateLimited { .. })
    }

    /// Get the status description
    pub fn status_description(&self) -> &'static str {
        self.status.description()
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
            key.status = ApiKeyStatus::Revoked {
                revoked_at: Utc::now(),
            };
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
    ///
    /// # Security Warning
    ///
    /// If using the default salt, a warning will be logged. In production,
    /// set the `SKREAVER_API_KEY_SALT` environment variable to a unique random value.
    pub fn new(config: ApiKeyConfig) -> Self {
        if config.is_using_default_salt() {
            warn!(
                "API key manager is using the default hardcoded salt. \
                 This is insecure for production use. \
                 Set SKREAVER_API_KEY_SALT environment variable to a unique random value."
            );
        }

        Self {
            config,
            store: ApiKeyStore::new(),
        }
    }

    /// Generate a new API key (type-safe version)
    pub async fn generate_key(&self, name: String, roles: Vec<Role>) -> AuthResult<Key<Active>> {
        let key_value =
            Key::<Active>::generate_key_value(&self.config.prefix, self.config.min_length).await?;
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
    ///
    /// SECURITY: Uses constant-time comparison to prevent timing attacks.
    /// The hash comparison uses the `subtle` crate's ConstantTimeEq trait
    /// to ensure authentication time doesn't leak information about valid keys.
    pub async fn authenticate_with_key(
        &self,
        key_str: &str,
    ) -> AuthResult<(Key<Active>, Principal)> {
        use subtle::ConstantTimeEq;

        // Validate key format (these checks are intentionally NOT constant-time
        // as they reveal only format requirements, not key validity)
        if !key_str.starts_with(&self.config.prefix) {
            return Err(AuthError::InvalidCredentials);
        }

        if key_str.len() < self.config.min_length + self.config.prefix.len() {
            return Err(AuthError::InvalidCredentials);
        }

        let provided_key_hash = self.hash_key(key_str);

        // Get the key from store using the hash
        // Note: HashMap lookup is not constant-time, but the actual key verification below is
        let api_key = self
            .store
            .get(&provided_key_hash)
            .await
            .ok_or(AuthError::ApiKeyNotFound)?;

        // SECURITY: Constant-time comparison of the provided key hash against stored key hash
        // This prevents timing attacks that could leak information about valid key hashes
        let stored_key_hash = self.hash_key(api_key.expose_key());
        let hashes_equal: bool = provided_key_hash
            .as_bytes()
            .ct_eq(stored_key_hash.as_bytes())
            .into();

        if !hashes_equal {
            // This should never happen if the HashMap lookup succeeded,
            // but we check anyway for defense in depth
            return Err(AuthError::InvalidCredentials);
        }

        // Convert to type-safe key and check state
        let active_key: Key<Active> = api_key.try_into()?;

        // Check expiration (returns Result<Key<Active>, Key<Expired>>)
        let valid_key = active_key
            .check_expiration()
            .map_err(|_expired_key| AuthError::TokenExpired)?;

        // Update last used timestamp
        self.store.update_last_used(&provided_key_hash).await;
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
        if !self.config.rotation.is_allowed() {
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
    ///
    /// MEDIUM-34: Uses salted hashing to prevent:
    /// - Hash collision DoS attacks
    /// - Rainbow table attacks if hashes are exposed
    /// - Cross-deployment hash correlation
    fn hash_key(&self, key: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        // MEDIUM-34: Prepend salt before key to prevent hash collisions
        hasher.update(self.config.hash_salt.as_bytes());
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
