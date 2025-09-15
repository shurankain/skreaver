//! Secure storage for authentication credentials

use super::AuthResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
pub struct InMemoryStorage {
    data: Arc<RwLock<HashMap<String, String>>>,
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryStorage {
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

/// Secure storage wrapper with encryption (placeholder for now)
pub struct SecureStorage {
    backend: Box<dyn CredentialStorage>,
}

impl SecureStorage {
    pub fn new(backend: Box<dyn CredentialStorage>) -> Self {
        Self { backend }
    }

    /// Encrypt and store a value
    pub async fn store_encrypted(&self, key: &str, value: &str) -> AuthResult<()> {
        // TODO: Implement actual encryption
        self.backend.store(key, value).await
    }

    /// Retrieve and decrypt a value
    pub async fn get_decrypted(&self, key: &str) -> AuthResult<Option<String>> {
        // TODO: Implement actual decryption
        self.backend.get(key).await
    }
}
