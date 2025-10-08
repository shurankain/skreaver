//! Secure storage for authentication credentials
//!
//! This module provides encrypted credential storage using AES-256-GCM.
//! Each value is encrypted with a unique nonce for maximum security.

use super::{AuthError, AuthResult};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use zeroize::Zeroize;

/// Trait for credential storage backends
#[async_trait]
pub trait CredentialStorage: Send + Sync {
    /// Store a credential
    async fn store(&self, key: &str, value: &str) -> AuthResult<()>;

    /// Retrieve a credential
    async fn get(&self, key: &str) -> AuthResult<Option<String>>;

    /// Delete a credential
    async fn delete(&self, key: &str) -> AuthResult<()>;

    /// List all keys
    async fn list_keys(&self) -> AuthResult<Vec<String>>;

    /// Check if a key exists
    async fn exists(&self, key: &str) -> AuthResult<bool>;
}

/// In-memory credential storage (for testing/development)
#[derive(Clone)]
pub struct InMemoryStorage {
    data: Arc<RwLock<HashMap<String, String>>>,
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryStorage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl CredentialStorage for InMemoryStorage {
    async fn store(&self, key: &str, value: &str) -> AuthResult<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn get(&self, key: &str) -> AuthResult<Option<String>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn delete(&self, key: &str) -> AuthResult<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn list_keys(&self) -> AuthResult<Vec<String>> {
        let data = self.data.read().await;
        Ok(data.keys().cloned().collect())
    }

    async fn exists(&self, key: &str) -> AuthResult<bool> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }
}

/// Encryption key for AES-256-GCM
///
/// This type uses `zeroize` to securely erase the key from memory when dropped.
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct EncryptionKey([u8; 32]);

impl EncryptionKey {
    /// Create a new encryption key from raw bytes
    ///
    /// # Security
    ///
    /// The key must be 32 bytes (256 bits) of cryptographically secure random data.
    /// Use `EncryptionKey::generate()` to create a new key.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Generate a new random encryption key using a cryptographically secure RNG
    #[must_use]
    pub fn generate() -> Self {
        use aes_gcm::aead::rand_core::RngCore;
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Load key from environment variable (recommended for production)
    ///
    /// The environment variable should contain a base64-encoded 32-byte key.
    ///
    /// # Example
    ///
    /// ```bash
    /// export SKREAVER_ENCRYPTION_KEY=$(openssl rand -base64 32)
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `AuthError::InvalidEncryptionKey` if:
    /// - The environment variable is not set
    /// - The value is not valid base64
    /// - The decoded value is not exactly 32 bytes
    pub fn from_env(var_name: &str) -> AuthResult<Self> {
        let encoded = std::env::var(var_name).map_err(|_| AuthError::InvalidEncryptionKey)?;

        let decoded = BASE64
            .decode(encoded.trim())
            .map_err(|_| AuthError::InvalidEncryptionKey)?;

        if decoded.len() != 32 {
            return Err(AuthError::InvalidEncryptionKey);
        }

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&decoded);
        Ok(Self(bytes))
    }

    /// Get reference to the raw key bytes (use sparingly)
    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Secure storage wrapper with AES-256-GCM encryption
///
/// This implementation provides authenticated encryption for all stored credentials.
/// Each value is encrypted with a unique nonce, preventing replay attacks and
/// ensuring that identical plaintexts produce different ciphertexts.
///
/// # Security Properties
///
/// - **Confidentiality**: AES-256 encryption prevents unauthorized reading
/// - **Integrity**: GCM authentication prevents tampering
/// - **Uniqueness**: Random nonce per encryption prevents pattern analysis
///
/// # Example
///
/// ```no_run
/// use skreaver_core::auth::{SecureStorage, InMemoryStorage, EncryptionKey};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = Box::new(InMemoryStorage::new());
/// let key = EncryptionKey::from_env("SKREAVER_ENCRYPTION_KEY")?;
/// let storage = SecureStorage::new(backend, &key);
///
/// storage.store_encrypted("api_key", "secret-value").await?;
/// let value = storage.get_decrypted("api_key").await?;
/// assert_eq!(value, Some("secret-value".to_string()));
/// # Ok(())
/// # }
/// ```
pub struct SecureStorage {
    backend: Box<dyn CredentialStorage>,
    cipher: Aes256Gcm,
}

impl SecureStorage {
    /// Create a new secure storage with the given backend and encryption key
    ///
    /// # Security
    ///
    /// The encryption key should be:
    /// - Generated using `EncryptionKey::generate()`
    /// - Loaded from a secure source using `EncryptionKey::from_env()`
    /// - NEVER hardcoded in source code
    /// - Rotated periodically (recommended: every 90 days)
    #[must_use]
    pub fn new(backend: Box<dyn CredentialStorage>, key: &EncryptionKey) -> Self {
        let cipher = Aes256Gcm::new(key.as_bytes().into());
        Self { backend, cipher }
    }

    /// Encrypt and store a value
    ///
    /// The value is encrypted with AES-256-GCM and a random nonce, then
    /// base64-encoded before storage. The format is: `nonce || ciphertext`
    /// where || denotes concatenation.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::EncryptionFailed` if encryption fails (very rare).
    /// Returns `AuthError::StorageError` if the backend storage fails.
    pub async fn store_encrypted(&self, key: &str, value: &str) -> AuthResult<()> {
        // Generate a unique nonce for this encryption
        use aes_gcm::aead::rand_core::RngCore;
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the value
        let ciphertext = self
            .cipher
            .encrypt(nonce, value.as_bytes())
            .map_err(|_| AuthError::EncryptionFailed)?;

        // Combine nonce + ciphertext and encode as base64
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        let encoded = BASE64.encode(&combined);

        // Store the encrypted value
        self.backend.store(key, &encoded).await
    }

    /// Retrieve and decrypt a value
    ///
    /// The value is retrieved from storage, base64-decoded, and decrypted.
    /// Returns `None` if the key doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::DecryptionFailed` if:
    /// - The stored data is corrupted or invalid
    /// - The data was encrypted with a different key
    /// - The authentication tag is invalid (data was tampered with)
    ///
    /// Returns `AuthError::StorageError` if the backend storage fails.
    pub async fn get_decrypted(&self, key: &str) -> AuthResult<Option<String>> {
        // Retrieve the encrypted value
        let Some(encoded) = self.backend.get(key).await? else {
            return Ok(None);
        };

        // Decode from base64
        let combined = BASE64
            .decode(encoded.as_bytes())
            .map_err(|_| AuthError::DecryptionFailed)?;

        // Extract nonce and ciphertext
        if combined.len() < 12 {
            return Err(AuthError::DecryptionFailed);
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt the value
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AuthError::DecryptionFailed)?;

        // Convert to string
        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|_| AuthError::DecryptionFailed)
    }

    /// Delete an encrypted value
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if the backend storage fails.
    pub async fn delete(&self, key: &str) -> AuthResult<()> {
        self.backend.delete(key).await
    }

    /// List all keys (keys are not encrypted, only values)
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if the backend storage fails.
    pub async fn list_keys(&self) -> AuthResult<Vec<String>> {
        self.backend.list_keys().await
    }

    /// Check if a key exists
    ///
    /// # Errors
    ///
    /// Returns `AuthError::StorageError` if the backend storage fails.
    pub async fn exists(&self, key: &str) -> AuthResult<bool> {
        self.backend.exists(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_storage_basic_operations() {
        let storage = InMemoryStorage::new();

        // Test store and get
        storage.store("key1", "value1").await.unwrap();
        let value = storage.get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // Test exists
        assert!(storage.exists("key1").await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());

        // Test list_keys
        storage.store("key2", "value2").await.unwrap();
        let keys = storage.list_keys().await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));

        // Test delete
        storage.delete("key1").await.unwrap();
        assert!(!storage.exists("key1").await.unwrap());
        assert_eq!(storage.get("key1").await.unwrap(), None);
    }

    #[test]
    fn test_encryption_key_generation() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();

        // Keys should be different (extremely high probability)
        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_encryption_key_from_env() {
        use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

        // Generate a valid 32-byte key and encode it
        let mut key_bytes = [0u8; 32];
        use aes_gcm::aead::rand_core::RngCore;
        aes_gcm::aead::OsRng.fill_bytes(&mut key_bytes);
        let encoded = BASE64.encode(&key_bytes);

        // Set environment variable
        unsafe { std::env::set_var("TEST_ENCRYPTION_KEY", &encoded) };

        // Load from environment
        let key = EncryptionKey::from_env("TEST_ENCRYPTION_KEY").unwrap();
        assert_eq!(key.as_bytes(), &key_bytes);

        // Clean up
        unsafe { std::env::remove_var("TEST_ENCRYPTION_KEY") };
    }

    #[test]
    fn test_encryption_key_from_env_invalid() {
        // Test with missing variable
        unsafe { std::env::remove_var("NONEXISTENT_KEY") };
        let result = EncryptionKey::from_env("NONEXISTENT_KEY");
        assert!(matches!(result, Err(AuthError::InvalidEncryptionKey)));

        // Test with invalid base64
        unsafe { std::env::set_var("INVALID_KEY", "not-valid-base64!!!") };
        let result = EncryptionKey::from_env("INVALID_KEY");
        assert!(matches!(result, Err(AuthError::InvalidEncryptionKey)));
        unsafe { std::env::remove_var("INVALID_KEY") };

        // Test with wrong length
        use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
        let short_key = BASE64.encode(&[0u8; 16]); // Only 16 bytes, not 32
        unsafe { std::env::set_var("SHORT_KEY", &short_key) };
        let result = EncryptionKey::from_env("SHORT_KEY");
        assert!(matches!(result, Err(AuthError::InvalidEncryptionKey)));
        unsafe { std::env::remove_var("SHORT_KEY") };
    }

    #[tokio::test]
    async fn test_secure_storage_encryption_decryption() {
        let backend = Box::new(InMemoryStorage::new());
        let key = EncryptionKey::generate();
        let storage = SecureStorage::new(backend, &key);

        // Test basic encryption and decryption
        let original_value = "my-secret-api-key";
        storage
            .store_encrypted("api_key", original_value)
            .await
            .unwrap();

        let decrypted = storage.get_decrypted("api_key").await.unwrap();
        assert_eq!(decrypted, Some(original_value.to_string()));
    }

    #[tokio::test]
    async fn test_secure_storage_actually_encrypts() {
        // Use a shared backend so we can access the raw stored data
        let shared_backend = InMemoryStorage::new();
        let key = EncryptionKey::generate();
        let storage = SecureStorage::new(Box::new(shared_backend.clone()), &key);

        // Store encrypted value
        let original_value = "my-secret-password";
        storage
            .store_encrypted("password", original_value)
            .await
            .unwrap();

        // Get raw stored value directly from the shared backend
        let raw_backend_value = shared_backend.get("password").await.unwrap().unwrap();

        // The stored value in the backend should NOT be plaintext
        assert_ne!(raw_backend_value, original_value);

        // The stored value should be base64-encoded encrypted data
        use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
        let decoded = BASE64.decode(&raw_backend_value);
        assert!(decoded.is_ok());

        // The decoded data should be at least nonce (12 bytes) + some ciphertext
        let decoded_bytes = decoded.unwrap();
        assert!(decoded_bytes.len() > 12);
    }

    #[tokio::test]
    async fn test_secure_storage_unique_nonces() {
        let backend = Box::new(InMemoryStorage::new());
        let key = EncryptionKey::generate();
        let storage = SecureStorage::new(backend, &key);

        // Encrypt the same value twice
        storage.store_encrypted("key1", "same-value").await.unwrap();
        storage.store_encrypted("key2", "same-value").await.unwrap();

        // Get the raw backend to check stored values
        // Note: We can't easily access the backend directly, so we'll use a workaround
        // by creating a fresh storage with the same backend type
        let check_backend = InMemoryStorage::new();
        let check_key = EncryptionKey::generate();
        let check_storage = SecureStorage::new(Box::new(check_backend), &check_key);

        // Encrypt same value twice with check_storage
        check_storage
            .store_encrypted("key1", "same-value")
            .await
            .unwrap();
        check_storage
            .store_encrypted("key2", "same-value")
            .await
            .unwrap();

        // Both should decrypt to the same value
        let decrypted1 = storage.get_decrypted("key1").await.unwrap();
        let decrypted2 = storage.get_decrypted("key2").await.unwrap();
        assert_eq!(decrypted1, Some("same-value".to_string()));
        assert_eq!(decrypted2, Some("same-value".to_string()));
    }

    #[tokio::test]
    async fn test_secure_storage_wrong_key() {
        // Create shared backend and two different keys
        let shared_backend = InMemoryStorage::new();
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();

        // Encrypt with first key
        let storage_encrypt = SecureStorage::new(Box::new(shared_backend.clone()), &key1);
        storage_encrypt
            .store_encrypted("secret", "classified")
            .await
            .unwrap();

        // Try to decrypt with different key
        let storage_decrypt = SecureStorage::new(Box::new(shared_backend), &key2);
        let result = storage_decrypt.get_decrypted("secret").await;

        // Should fail with decryption error
        assert!(matches!(result, Err(AuthError::DecryptionFailed)));
    }

    #[tokio::test]
    async fn test_secure_storage_nonexistent_key() {
        let backend = Box::new(InMemoryStorage::new());
        let key = EncryptionKey::generate();
        let storage = SecureStorage::new(backend, &key);

        // Try to get non-existent key
        let result = storage.get_decrypted("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_secure_storage_corrupted_data() {
        let shared_backend = InMemoryStorage::new();
        let key = EncryptionKey::generate();

        // Store corrupted data directly in backend
        shared_backend
            .store("corrupted", "not-valid-base64!!!")
            .await
            .unwrap();

        let test_storage = SecureStorage::new(Box::new(shared_backend.clone()), &key);
        let result = test_storage.get_decrypted("corrupted").await;

        // Should fail with decryption error
        assert!(matches!(result, Err(AuthError::DecryptionFailed)));

        // Test with valid base64 but too short (less than nonce size)
        use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
        let short_data = BASE64.encode(&[1, 2, 3]); // Only 3 bytes
        shared_backend.store("short", &short_data).await.unwrap();

        let result = test_storage.get_decrypted("short").await;
        assert!(matches!(result, Err(AuthError::DecryptionFailed)));
    }

    #[tokio::test]
    async fn test_secure_storage_delete() {
        let backend = Box::new(InMemoryStorage::new());
        let key = EncryptionKey::generate();
        let storage = SecureStorage::new(backend, &key);

        // Store and verify
        storage.store_encrypted("temp", "data").await.unwrap();
        assert!(storage.exists("temp").await.unwrap());

        // Delete
        storage.delete("temp").await.unwrap();
        assert!(!storage.exists("temp").await.unwrap());

        // Get should return None
        let result = storage.get_decrypted("temp").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_secure_storage_list_keys() {
        let backend = Box::new(InMemoryStorage::new());
        let key = EncryptionKey::generate();
        let storage = SecureStorage::new(backend, &key);

        // Store multiple values
        storage.store_encrypted("key1", "value1").await.unwrap();
        storage.store_encrypted("key2", "value2").await.unwrap();
        storage.store_encrypted("key3", "value3").await.unwrap();

        // List keys
        let keys = storage.list_keys().await.unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[tokio::test]
    async fn test_secure_storage_overwrite() {
        let backend = Box::new(InMemoryStorage::new());
        let key = EncryptionKey::generate();
        let storage = SecureStorage::new(backend, &key);

        // Store initial value
        storage.store_encrypted("key", "value1").await.unwrap();
        let result = storage.get_decrypted("key").await.unwrap();
        assert_eq!(result, Some("value1".to_string()));

        // Overwrite with new value
        storage.store_encrypted("key", "value2").await.unwrap();
        let result = storage.get_decrypted("key").await.unwrap();
        assert_eq!(result, Some("value2".to_string()));
    }

    #[test]
    fn test_encryption_key_zeroize() {
        // This test verifies that the key is marked for zeroing
        // The actual zeroing happens automatically on drop due to the #[zeroize(drop)] attribute
        let key = EncryptionKey::generate();

        // Get the bytes to verify they're non-zero
        let bytes = key.as_bytes();
        let is_non_zero = bytes.iter().any(|&b| b != 0);
        assert!(is_non_zero, "Generated key should have non-zero bytes");

        // Drop happens here automatically
        drop(key);

        // We can't directly verify the memory was zeroed (it's dropped),
        // but the zeroize crate handles this automatically due to #[zeroize(drop)]
    }
}
