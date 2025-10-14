//! Encryption utilities for secure credential storage
//!
//! This module provides AES-256-GCM encryption for protecting sensitive data at rest.
//! Each value is encrypted with a unique nonce for maximum security.

use super::{AuthError, AuthResult};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use zeroize::Zeroize;

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

    /// Get reference to the raw key bytes (internal use only)
    pub(super) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Encrypt a value using AES-256-GCM with a random nonce
///
/// # Arguments
///
/// * `cipher` - The AES-256-GCM cipher instance
/// * `plaintext` - The value to encrypt
///
/// # Returns
///
/// Base64-encoded string in format: `nonce||ciphertext`
///
/// # Errors
///
/// Returns `AuthError::EncryptionFailed` if encryption fails (very rare).
pub(super) fn encrypt(cipher: &Aes256Gcm, plaintext: &str) -> AuthResult<String> {
    // Generate a unique nonce for this encryption
    use aes_gcm::aead::rand_core::RngCore;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt the value
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| AuthError::EncryptionFailed)?;

    // Combine nonce and ciphertext, then base64 encode
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&combined))
}

/// Decrypt a value that was encrypted with `encrypt()`
///
/// # Arguments
///
/// * `cipher` - The AES-256-GCM cipher instance
/// * `encrypted` - Base64-encoded `nonce||ciphertext` string
///
/// # Returns
///
/// The decrypted plaintext string
///
/// # Errors
///
/// Returns `AuthError::DecryptionFailed` if:
/// - The encrypted value is not valid base64
/// - The value is too short (< 12 bytes for nonce)
/// - Authentication tag verification fails
/// - Decryption fails
pub(super) fn decrypt(cipher: &Aes256Gcm, encrypted: &str) -> AuthResult<String> {
    // Decode from base64
    let combined = BASE64
        .decode(encrypted)
        .map_err(|_| AuthError::DecryptionFailed)?;

    // Split nonce and ciphertext
    if combined.len() < 12 {
        return Err(AuthError::DecryptionFailed);
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AuthError::DecryptionFailed)?;

    String::from_utf8(plaintext).map_err(|_| AuthError::DecryptionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_key_generation() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();

        // Keys should be different
        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_encryption_key_from_bytes() {
        let bytes = [42u8; 32];
        let key = EncryptionKey::from_bytes(bytes);
        assert_eq!(key.as_bytes(), &bytes);
    }

    #[test]
    fn test_encryption_roundtrip() {
        let key = EncryptionKey::generate();
        let cipher = Aes256Gcm::new(key.as_bytes().into());

        let plaintext = "Hello, World!";
        let encrypted = encrypt(&cipher, plaintext).unwrap();
        let decrypted = decrypt(&cipher, &encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encryption_different_ciphertexts() {
        let key = EncryptionKey::generate();
        let cipher = Aes256Gcm::new(key.as_bytes().into());

        let plaintext = "Hello, World!";
        let encrypted1 = encrypt(&cipher, plaintext).unwrap();
        let encrypted2 = encrypt(&cipher, plaintext).unwrap();

        // Same plaintext should produce different ciphertexts (due to unique nonces)
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same plaintext
        assert_eq!(decrypt(&cipher, &encrypted1).unwrap(), plaintext);
        assert_eq!(decrypt(&cipher, &encrypted2).unwrap(), plaintext);
    }

    #[test]
    fn test_decryption_wrong_key() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();

        let cipher1 = Aes256Gcm::new(key1.as_bytes().into());
        let cipher2 = Aes256Gcm::new(key2.as_bytes().into());

        let plaintext = "secret";
        let encrypted = encrypt(&cipher1, plaintext).unwrap();

        // Decryption with wrong key should fail
        assert!(decrypt(&cipher2, &encrypted).is_err());
    }

    #[test]
    fn test_decryption_tampered_data() {
        let key = EncryptionKey::generate();
        let cipher = Aes256Gcm::new(key.as_bytes().into());

        let plaintext = "secret";
        let mut encrypted = encrypt(&cipher, plaintext).unwrap();

        // Tamper with the encrypted data
        encrypted.push('X');

        // Decryption should fail due to authentication
        assert!(decrypt(&cipher, &encrypted).is_err());
    }
}
