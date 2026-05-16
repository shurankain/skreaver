//! OAuth 2.1 configuration.

use serde::{Deserialize, Serialize};

use crate::security::SecretString;

use super::types::OAuthError;

/// OAuth 2.1 provider configuration.
#[derive(Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// OAuth client ID.
    pub client_id: String,
    /// Client secret (None for public clients using PKCE only).
    #[serde(default, skip_serializing)]
    pub client_secret: Option<SecretString>,
    /// Authorization endpoint URL.
    pub authorization_endpoint: String,
    /// Token endpoint URL.
    pub token_endpoint: String,
    /// Exact redirect URI (must match registration).
    pub redirect_uri: String,
    /// Requested scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Whether PKCE is required (default true per OAuth 2.1).
    #[serde(default = "default_true")]
    pub pkce_required: bool,
    /// State parameter TTL in seconds (default 600 = 10 minutes).
    #[serde(default = "default_state_ttl")]
    pub state_ttl_seconds: u64,
}

fn default_true() -> bool {
    true
}

fn default_state_ttl() -> u64 {
    600
}

impl OAuthConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), OAuthError> {
        if self.client_id.is_empty() {
            return Err(OAuthError::InvalidConfig("client_id is required".into()));
        }
        if self.authorization_endpoint.is_empty() {
            return Err(OAuthError::InvalidConfig(
                "authorization_endpoint is required".into(),
            ));
        }
        if self.token_endpoint.is_empty() {
            return Err(OAuthError::InvalidConfig(
                "token_endpoint is required".into(),
            ));
        }
        if self.redirect_uri.is_empty() {
            return Err(OAuthError::InvalidConfig("redirect_uri is required".into()));
        }
        Ok(())
    }
}

impl std::fmt::Debug for OAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("authorization_endpoint", &self.authorization_endpoint)
            .field("token_endpoint", &self.token_endpoint)
            .field("redirect_uri", &self.redirect_uri)
            .field("scopes", &self.scopes)
            .field("pkce_required", &self.pkce_required)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> OAuthConfig {
        OAuthConfig {
            client_id: "my-client".to_string(),
            client_secret: None,
            authorization_endpoint: "https://auth.example.com/authorize".to_string(),
            token_endpoint: "https://auth.example.com/token".to_string(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            scopes: vec!["openid".to_string(), "profile".to_string()],
            pkce_required: true,
            state_ttl_seconds: 600,
        }
    }

    #[test]
    fn test_valid_config() {
        assert!(valid_config().validate().is_ok());
    }

    #[test]
    fn test_missing_client_id() {
        let mut config = valid_config();
        config.client_id = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_missing_endpoints() {
        let mut config = valid_config();
        config.authorization_endpoint = String::new();
        assert!(config.validate().is_err());

        let mut config = valid_config();
        config.token_endpoint = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_debug_redacts_secret() {
        let mut config = valid_config();
        config.client_secret = Some(SecretString::from_string("super-secret".to_string()));
        let debug = format!("{:?}", config);
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("super-secret"));
    }

    #[test]
    fn test_serde_round_trip() {
        let config = valid_config();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: OAuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.client_id, "my-client");
        assert!(deserialized.pkce_required);
        assert_eq!(deserialized.scopes.len(), 2);
    }
}
