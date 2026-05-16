//! OAuth 2.1 type definitions.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::security::SecretString;

/// OAuth 2.1 grant types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantType {
    AuthorizationCode,
    RefreshToken,
}

impl std::fmt::Display for GrantType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GrantType::AuthorizationCode => write!(f, "authorization_code"),
            GrantType::RefreshToken => write!(f, "refresh_token"),
        }
    }
}

/// Result of starting the OAuth authorization flow.
pub struct AuthorizationRequest {
    /// Full authorization URL to redirect the user to.
    pub authorization_url: String,
    /// CSRF state token (must be verified on callback).
    pub state: String,
}

/// Token request sent to the token endpoint.
pub struct TokenRequest {
    pub grant_type: GrantType,
    pub code: Option<String>,
    pub redirect_uri: String,
    pub code_verifier: Option<SecretString>,
    pub refresh_token: Option<SecretString>,
    pub client_id: String,
    pub client_secret: Option<SecretString>,
}

impl TokenRequest {
    /// Encode as application/x-www-form-urlencoded parameters.
    pub fn to_params(&self) -> Vec<(&str, String)> {
        let mut params = vec![
            ("grant_type", self.grant_type.to_string()),
            ("client_id", self.client_id.clone()),
            ("redirect_uri", self.redirect_uri.clone()),
        ];
        if let Some(code) = &self.code {
            params.push(("code", code.clone()));
        }
        if let Some(verifier) = &self.code_verifier {
            params.push(("code_verifier", verifier.expose_secret().to_string()));
        }
        if let Some(refresh) = &self.refresh_token {
            params.push(("refresh_token", refresh.expose_secret().to_string()));
        }
        params
    }
}

/// Token response from the authorization server.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Errors from the OAuth flow.
#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("Invalid OAuth configuration: {0}")]
    InvalidConfig(String),

    #[error("PKCE error: {0}")]
    PkceError(String),

    #[error("Invalid state parameter (possible CSRF)")]
    InvalidState,

    #[error("State expired or not found")]
    StateExpired,

    #[error("Token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("Invalid grant: {0}")]
    InvalidGrant(String),

    #[error("Provider error: {error}")]
    ProviderError {
        error: String,
        description: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grant_type_display() {
        assert_eq!(
            GrantType::AuthorizationCode.to_string(),
            "authorization_code"
        );
        assert_eq!(GrantType::RefreshToken.to_string(), "refresh_token");
    }

    #[test]
    fn test_token_request_params() {
        let req = TokenRequest {
            grant_type: GrantType::AuthorizationCode,
            code: Some("auth_code_123".to_string()),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            code_verifier: Some(SecretString::from_string("verifier_abc".to_string())),
            refresh_token: None,
            client_id: "my-client".to_string(),
            client_secret: None,
        };
        let params = req.to_params();
        assert!(
            params
                .iter()
                .any(|(k, v)| *k == "grant_type" && v == "authorization_code")
        );
        assert!(
            params
                .iter()
                .any(|(k, v)| *k == "code" && v == "auth_code_123")
        );
        assert!(
            params
                .iter()
                .any(|(k, v)| *k == "code_verifier" && v == "verifier_abc")
        );
        assert!(
            params
                .iter()
                .any(|(k, v)| *k == "client_id" && v == "my-client")
        );
    }

    #[test]
    fn test_token_response_deserialize() {
        let json = r#"{
            "access_token": "at_123",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "rt_456",
            "scope": "read write"
        }"#;
        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token, "at_123");
        assert_eq!(resp.token_type, "Bearer");
        assert_eq!(resp.expires_in, Some(3600));
        assert_eq!(resp.refresh_token.as_deref(), Some("rt_456"));
        assert_eq!(resp.scope.as_deref(), Some("read write"));
    }

    #[test]
    fn test_token_response_minimal() {
        let json = r#"{"access_token": "at", "token_type": "Bearer"}"#;
        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token, "at");
        assert!(resp.expires_in.is_none());
        assert!(resp.refresh_token.is_none());
    }
}
