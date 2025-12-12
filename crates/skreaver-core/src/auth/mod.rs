//! Authentication and authorization framework for Skreaver
//!
//! This module provides a comprehensive authentication system with:
//! - API Key authentication for service-to-service communication
//! - JWT (JSON Web Token) support with HMAC validation
//! - Role-Based Access Control (RBAC) with admin/agent/viewer roles
//! - Per-tool access policies for fine-grained control
//! - Secure credential storage and management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub mod api_key;
pub mod jwt;
pub mod jwt_revocation;
pub mod middleware;
pub mod rbac;
pub mod storage;

pub use api_key::{Active, ApiKey, ApiKeyConfig, ApiKeyManager, Expired, Key, Revoked};
pub use jwt::{
    AccessToken, JwtClaims, JwtConfig, JwtManager, JwtToken, RefreshToken, Token, TokenPair,
};
#[cfg(feature = "redis")]
pub use jwt_revocation::RedisBlacklist;
pub use jwt_revocation::{InMemoryBlacklist, TokenBlacklist};
pub use middleware::{AuthMiddleware, AuthenticatedRequest, AuthenticationPolicy};
pub use rbac::{Permission, Role, RoleManager, ToolPolicy};
pub use storage::{CredentialStorage, EncryptionKey, InMemoryStorage, SecureStorage};

/// Authentication errors
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Token expired")]
    TokenExpired,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("API key not found")]
    ApiKeyNotFound,

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Role not found: {0}")]
    RoleNotFound(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed")]
    DecryptionFailed,

    #[error("Invalid encryption key")]
    InvalidEncryptionKey,

    #[error("Failed to generate cryptographically secure random bytes: {0}")]
    RandomGenerationFailed(String),
}

/// Authentication result type
pub type AuthResult<T> = Result<T, AuthError>;

/// Authentication method types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// API Key authentication
    ApiKey(String),
    /// JWT Bearer token
    Bearer(String),
    /// Basic authentication (username:password)
    Basic { username: String, password: String },
}

/// Authenticated principal information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principal {
    /// Unique identifier
    pub id: String,
    /// Principal name
    pub name: String,
    /// Authentication method used
    pub auth_method: AuthMethod,
    /// Assigned roles
    pub roles: Vec<Role>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Principal {
    /// Create a new principal
    pub fn new(id: String, name: String, auth_method: AuthMethod) -> Self {
        Self {
            id,
            name,
            auth_method,
            roles: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a role to the principal
    pub fn with_role(mut self, role: Role) -> Self {
        self.roles.push(role);
        self
    }

    /// Add metadata to the principal
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if principal has a specific role
    pub fn has_role(&self, role: &Role) -> bool {
        self.roles.contains(role)
    }

    /// Check if principal has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.roles
            .iter()
            .any(|role| role.has_permission(permission))
    }
}

/// Authentication context for requests
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Authenticated principal
    pub principal: Principal,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Request ID for audit logging
    pub request_id: String,
    /// Additional context data
    pub context: HashMap<String, String>,
}

impl AuthContext {
    /// Create a new authentication context
    pub fn new(principal: Principal) -> Self {
        Self {
            principal,
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
            context: HashMap::new(),
        }
    }

    /// Check if the context has required permission
    pub fn check_permission(&self, permission: &Permission) -> AuthResult<()> {
        if self.principal.has_permission(permission) {
            Ok(())
        } else {
            Err(AuthError::InsufficientPermissions)
        }
    }

    /// Check if the context has required role
    pub fn check_role(&self, role: &Role) -> AuthResult<()> {
        if self.principal.has_role(role) {
            Ok(())
        } else {
            Err(AuthError::InsufficientPermissions)
        }
    }
}

/// Main authentication manager
pub struct AuthManager {
    /// API Key manager
    api_key_manager: ApiKeyManager,
    /// JWT manager
    jwt_manager: JwtManager,
    /// Role manager
    role_manager: RoleManager,
    /// Credential storage
    storage: Box<dyn CredentialStorage>,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(
        api_key_manager: ApiKeyManager,
        jwt_manager: JwtManager,
        role_manager: RoleManager,
        storage: Box<dyn CredentialStorage>,
    ) -> Self {
        Self {
            api_key_manager,
            jwt_manager,
            role_manager,
            storage,
        }
    }

    /// Authenticate a request
    pub async fn authenticate(&self, method: &AuthMethod) -> AuthResult<AuthContext> {
        let principal = match method {
            AuthMethod::ApiKey(key) => self.api_key_manager.authenticate(key).await?,
            AuthMethod::Bearer(token) => self.jwt_manager.authenticate(token).await?,
            AuthMethod::Basic { username, password } => {
                self.authenticate_basic(username, password).await?
            }
        };

        Ok(AuthContext::new(principal))
    }

    /// Authenticate with username and password
    async fn authenticate_basic(&self, _username: &str, _password: &str) -> AuthResult<Principal> {
        // This would typically check against a user database
        // For now, return an error as this is a placeholder
        Err(AuthError::InvalidCredentials)
    }

    /// Generate a new API key
    pub async fn generate_api_key(&self, name: String, roles: Vec<Role>) -> AuthResult<ApiKey> {
        self.api_key_manager.generate(name, roles).await
    }

    /// Generate a new JWT token
    pub async fn generate_jwt(&self, principal: &Principal) -> AuthResult<JwtToken> {
        self.jwt_manager.generate(principal).await
    }

    /// Revoke an API key
    pub async fn revoke_api_key(&self, key: &str) -> AuthResult<()> {
        self.api_key_manager.revoke(key).await
    }

    /// Check if a tool can be accessed by the principal
    pub fn check_tool_access(&self, tool_name: &str, principal: &Principal) -> bool {
        let roles = &principal.roles;
        let permissions = roles.iter().flat_map(|role| role.permissions()).collect();

        self.role_manager
            .check_tool_access(tool_name, roles, &permissions)
    }

    /// Store a credential securely
    pub async fn store_credential(&self, key: &str, value: &str) -> AuthResult<()> {
        self.storage.store(key, value).await
    }

    /// Retrieve a credential
    pub async fn get_credential(&self, key: &str) -> AuthResult<Option<String>> {
        self.storage.get(key).await
    }

    /// Delete a credential
    pub async fn delete_credential(&self, key: &str) -> AuthResult<()> {
        self.storage.delete(key).await
    }

    /// Add a tool policy to the role manager
    pub fn add_tool_policy(&mut self, policy: ToolPolicy) {
        self.role_manager.add_tool_policy(policy);
    }

    /// Define a custom role with specific permissions
    pub fn define_custom_role(
        &mut self,
        name: String,
        permissions: std::collections::HashSet<Permission>,
    ) {
        self.role_manager.define_custom_role(name, permissions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_principal_creation() {
        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test-key".to_string()),
        );

        assert_eq!(principal.id, "user-123");
        assert_eq!(principal.name, "Test User");
        assert!(principal.roles.is_empty());
    }

    #[test]
    fn test_auth_context() {
        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::Bearer("token".to_string()),
        );

        let context = AuthContext::new(principal);
        assert!(!context.request_id.is_empty());
    }

    #[tokio::test]
    async fn test_auth_manager_tool_access() {
        let auth_manager = create_test_auth_manager();

        let principal = Principal::new(
            "user-123".to_string(),
            "Test User".to_string(),
            AuthMethod::ApiKey("test".to_string()),
        )
        .with_role(Role::Agent);

        // Agent should access regular tools
        assert!(auth_manager.check_tool_access("http_get", &principal));

        // Agent should not access admin tools
        assert!(!auth_manager.check_tool_access("shell_exec", &principal));
    }

    #[tokio::test]
    async fn test_auth_manager_credential_storage() {
        let auth_manager = create_test_auth_manager();

        // Test storing and retrieving credentials
        auth_manager
            .store_credential("test_key", "test_value")
            .await
            .unwrap();

        let retrieved = auth_manager.get_credential("test_key").await.unwrap();
        assert_eq!(retrieved, Some("test_value".to_string()));

        // Test deletion
        auth_manager.delete_credential("test_key").await.unwrap();
        let deleted = auth_manager.get_credential("test_key").await.unwrap();
        assert_eq!(deleted, None);
    }

    fn create_test_auth_manager() -> AuthManager {
        AuthManager::new(
            ApiKeyManager::new(api_key::ApiKeyConfig::default()),
            jwt::JwtManager::new(jwt::JwtConfig::default()),
            rbac::RoleManager::with_defaults(),
            Box::new(storage::InMemoryStorage::new()),
        )
    }
}
