//! Authentication token types for compile-time safety

use std::str::FromStr;

/// Strongly typed authentication token
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthToken {
    /// JWT Bearer token
    Jwt(String),
    /// API Key token (with sk- prefix)
    ApiKey(String),
}

/// Error when parsing authentication tokens
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthTokenError {
    /// No authentication token provided
    Missing,
    /// Invalid format - not Bearer or API key
    InvalidFormat,
    /// Empty token value
    EmptyToken,
    /// Invalid API key format (missing sk- prefix)
    InvalidApiKeyFormat,
}

impl AuthToken {
    /// Parse authorization header value into typed token
    pub fn from_header(header_value: &str) -> Result<Self, AuthTokenError> {
        let header_value = header_value.trim();

        if header_value.is_empty() {
            return Err(AuthTokenError::EmptyToken);
        }

        // Handle Bearer token
        if let Some(token) = header_value.strip_prefix("Bearer ") {
            if token.is_empty() {
                return Err(AuthTokenError::EmptyToken);
            }

            // Check if it's an API key (starts with sk-)
            if token.starts_with("sk-") {
                return Ok(AuthToken::ApiKey(token.to_string()));
            } else {
                return Ok(AuthToken::Jwt(token.to_string()));
            }
        }

        Err(AuthTokenError::InvalidFormat)
    }

    /// Parse API key from X-API-Key header
    pub fn from_api_key_header(header_value: &str) -> Result<Self, AuthTokenError> {
        let header_value = header_value.trim();

        if header_value.is_empty() {
            return Err(AuthTokenError::EmptyToken);
        }

        if !header_value.starts_with("sk-") {
            return Err(AuthTokenError::InvalidApiKeyFormat);
        }

        Ok(AuthToken::ApiKey(header_value.to_string()))
    }

    /// Get the underlying token value
    pub fn value(&self) -> &str {
        match self {
            Self::Jwt(token) => token,
            Self::ApiKey(key) => key,
        }
    }
}

impl FromStr for AuthToken {
    type Err = AuthTokenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_header(&format!("Bearer {}", s))
    }
}

impl std::fmt::Display for AuthTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "No authentication token provided"),
            Self::InvalidFormat => write!(
                f,
                "Invalid authentication format - expected Bearer token or API key"
            ),
            Self::EmptyToken => write!(f, "Empty authentication token"),
            Self::InvalidApiKeyFormat => {
                write!(f, "Invalid API key format - must start with 'sk-'")
            }
        }
    }
}

impl std::error::Error for AuthTokenError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_token_parsing() {
        let token =
            AuthToken::from_header("Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...").unwrap();
        match token {
            AuthToken::Jwt(value) => assert!(value.starts_with("eyJ")),
            _ => panic!("Expected JWT token"),
        }
    }

    #[test]
    fn test_api_key_parsing() {
        let token = AuthToken::from_header("Bearer sk-test-key-123").unwrap();
        match token {
            AuthToken::ApiKey(value) => assert_eq!(value, "sk-test-key-123"),
            _ => panic!("Expected API key"),
        }
    }

    #[test]
    fn test_api_key_header_parsing() {
        let token = AuthToken::from_api_key_header("sk-test-key-456").unwrap();
        match token {
            AuthToken::ApiKey(value) => assert_eq!(value, "sk-test-key-456"),
            _ => panic!("Expected API key"),
        }
    }

    #[test]
    fn test_invalid_formats() {
        assert!(matches!(
            AuthToken::from_header("InvalidToken"),
            Err(AuthTokenError::InvalidFormat)
        ));

        assert!(matches!(
            AuthToken::from_header("Bearer "),
            Err(AuthTokenError::EmptyToken)
        ));

        assert!(matches!(
            AuthToken::from_api_key_header("invalid-key"),
            Err(AuthTokenError::InvalidApiKeyFormat)
        ));
    }
}
