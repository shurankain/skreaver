//! HTTP middleware for authentication

use super::{AuthContext, AuthError, AuthManager, AuthMethod};
use std::sync::Arc;

/// Authenticated request with auth context
pub struct AuthenticatedRequest<T> {
    pub inner: T,
    pub auth_context: AuthContext,
}

/// Authentication middleware for HTTP endpoints
pub struct AuthMiddleware {
    auth_manager: Arc<AuthManager>,
    /// Allow anonymous access
    allow_anonymous: bool,
    /// Paths that don't require authentication
    public_paths: Vec<String>,
}

impl AuthMiddleware {
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self {
            auth_manager,
            allow_anonymous: false,
            public_paths: vec!["/health".to_string(), "/metrics".to_string()],
        }
    }

    pub fn with_anonymous(mut self, allow: bool) -> Self {
        self.allow_anonymous = allow;
        self
    }

    pub fn with_public_path(mut self, path: String) -> Self {
        self.public_paths.push(path);
        self
    }

    /// Extract auth method from headers
    pub fn extract_auth_method(&self, authorization: Option<&str>) -> Option<AuthMethod> {
        if let Some(auth_header) = authorization {
            if let Some(bearer) = auth_header.strip_prefix("Bearer ") {
                return Some(AuthMethod::Bearer(bearer.to_string()));
            }

            if let Some(api_key) = auth_header.strip_prefix("ApiKey ") {
                return Some(AuthMethod::ApiKey(api_key.to_string()));
            }

            if let Some(basic) = auth_header.strip_prefix("Basic ") {
                // Decode base64 and parse username:password
                if let Ok(decoded) =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, basic)
                    && let Ok(creds) = String::from_utf8(decoded)
                {
                    let parts: Vec<_> = creds.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        return Some(AuthMethod::Basic {
                            username: parts[0].to_string(),
                            password: parts[1].to_string(),
                        });
                    }
                }
            }
        }
        None
    }

    /// Check if path requires authentication
    pub fn requires_auth(&self, path: &str) -> bool {
        !self.public_paths.iter().any(|p| path.starts_with(p))
    }

    /// Authenticate a request
    pub async fn authenticate(
        &self,
        path: &str,
        authorization: Option<&str>,
    ) -> Result<Option<AuthContext>, AuthError> {
        // Check if path requires authentication
        if !self.requires_auth(path) {
            return Ok(None);
        }

        // Extract auth method
        let auth_method = self.extract_auth_method(authorization);

        if let Some(method) = auth_method {
            let context = self.auth_manager.authenticate(&method).await?;
            Ok(Some(context))
        } else if self.allow_anonymous {
            Ok(None)
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_middleware() -> AuthMiddleware {
        use crate::auth::api_key::ApiKeyConfig;
        use crate::auth::jwt::JwtConfig;
        use crate::auth::{ApiKeyManager, JwtManager, RoleManager, storage::InMemoryStorage};

        let auth_manager = AuthManager::new(
            ApiKeyManager::new(ApiKeyConfig::default()),
            JwtManager::new(JwtConfig::default()),
            RoleManager::new(),
            Box::new(InMemoryStorage::new()),
        );

        AuthMiddleware::new(Arc::new(auth_manager))
    }

    #[test]
    fn test_extract_bearer_token() {
        let middleware = create_test_middleware();

        let auth = middleware.extract_auth_method(Some("Bearer eyJhbGciOiJIUzI1NiJ9"));
        assert!(matches!(auth, Some(AuthMethod::Bearer(_))));
    }

    #[test]
    fn test_extract_api_key() {
        let middleware = create_test_middleware();

        let auth = middleware.extract_auth_method(Some("ApiKey sk_test_key"));
        assert!(matches!(auth, Some(AuthMethod::ApiKey(_))));
    }

    #[test]
    fn test_public_paths() {
        let middleware = create_test_middleware();

        assert!(!middleware.requires_auth("/health"));
        assert!(!middleware.requires_auth("/metrics"));
        assert!(middleware.requires_auth("/api/agents"));
    }
}
