//! PKCE (Proof Key for Code Exchange) utilities.
//!
//! Implements RFC 7636 with S256 method (mandatory in OAuth 2.1).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use sha2::{Digest, Sha256};

use crate::security::SecretString;

/// PKCE code challenge method. OAuth 2.1 mandates S256.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkceMethod {
    S256,
}

impl std::fmt::Display for PkceMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PkceMethod::S256 => write!(f, "S256"),
        }
    }
}

/// A PKCE code verifier and its corresponding challenge.
#[derive(Clone)]
pub struct PkceChallenge {
    /// The code verifier (43-128 URL-safe characters). Keep secret.
    pub code_verifier: SecretString,
    /// BASE64URL(SHA256(code_verifier)). Sent to the authorization server.
    pub code_challenge: String,
    /// Always S256 per OAuth 2.1.
    pub method: PkceMethod,
}

impl PkceChallenge {
    /// Generate a new PKCE challenge with a cryptographically random verifier.
    pub fn generate() -> Self {
        let verifier = generate_verifier();
        let challenge = compute_challenge(verifier.expose_secret());
        Self {
            code_verifier: verifier,
            code_challenge: challenge,
            method: PkceMethod::S256,
        }
    }

    /// Verify that a code_verifier matches a code_challenge.
    pub fn verify(verifier: &str, challenge: &str) -> bool {
        let computed = compute_challenge(verifier);
        computed == challenge
    }
}

/// Generate a random 64-byte URL-safe code verifier (86 chars base64url).
fn generate_verifier() -> SecretString {
    let mut bytes = [0u8; 64];
    rand::rng().fill(&mut bytes);
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    // Truncate to 128 chars max per RFC 7636
    let verifier = if encoded.len() > 128 {
        encoded[..128].to_string()
    } else {
        encoded
    };
    SecretString::from_string(verifier)
}

/// Compute BASE64URL(SHA256(verifier)) per RFC 7636 Section 4.2.
fn compute_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_verifier_length() {
        let challenge = PkceChallenge::generate();
        let verifier = challenge.code_verifier.expose_secret();
        assert!(
            verifier.len() >= 43,
            "Verifier too short: {}",
            verifier.len()
        );
        assert!(
            verifier.len() <= 128,
            "Verifier too long: {}",
            verifier.len()
        );
    }

    #[test]
    fn test_challenge_is_base64url() {
        let challenge = PkceChallenge::generate();
        // SHA-256 digest is 32 bytes → 43 chars in base64url (no padding)
        assert_eq!(challenge.code_challenge.len(), 43);
        assert!(
            challenge
                .code_challenge
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "Challenge contains invalid chars: {}",
            challenge.code_challenge
        );
    }

    #[test]
    fn test_verify_round_trip() {
        let challenge = PkceChallenge::generate();
        assert!(PkceChallenge::verify(
            challenge.code_verifier.expose_secret(),
            &challenge.code_challenge,
        ));
    }

    #[test]
    fn test_verify_wrong_verifier_fails() {
        let challenge = PkceChallenge::generate();
        assert!(!PkceChallenge::verify(
            "wrong-verifier-value",
            &challenge.code_challenge,
        ));
    }

    #[test]
    fn test_method_is_s256() {
        let challenge = PkceChallenge::generate();
        assert_eq!(challenge.method, PkceMethod::S256);
        assert_eq!(challenge.method.to_string(), "S256");
    }

    #[test]
    fn test_each_generate_is_unique() {
        let a = PkceChallenge::generate();
        let b = PkceChallenge::generate();
        assert_ne!(
            a.code_verifier.expose_secret(),
            b.code_verifier.expose_secret()
        );
        assert_ne!(a.code_challenge, b.code_challenge);
    }
}
