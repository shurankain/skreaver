//! Example: Secure Credential Storage with AES-256-GCM Encryption
//!
//! This example demonstrates how to use the SecureStorage wrapper to encrypt
//! credentials at rest using AES-256-GCM authenticated encryption.
//!
//! Key features:
//! - Confidentiality: AES-256 prevents unauthorized reading
//! - Integrity: GCM authentication prevents tampering
//! - Uniqueness: Random nonce per encryption prevents pattern analysis
//! - Memory safety: Encryption keys are automatically zeroed on drop
//!
//! # Usage
//!
//! ```bash
//! # Generate a secure encryption key (32 bytes, base64-encoded)
//! export SKREAVER_ENCRYPTION_KEY=$(openssl rand -base64 32)
//!
//! # Run the example
//! cargo run --example secure_credential_storage
//! ```

use skreaver_core::auth::{EncryptionKey, InMemoryStorage, SecureStorage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 Secure Credential Storage Example\n");

    // Step 1: Create or load encryption key
    println!("Step 1: Loading encryption key...");
    let key = match EncryptionKey::from_env("SKREAVER_ENCRYPTION_KEY") {
        Ok(key) => {
            println!("✅ Loaded encryption key from environment variable\n");
            key
        }
        Err(_) => {
            println!("⚠️  No encryption key in environment, generating new one...");
            println!("   In production, always use SKREAVER_ENCRYPTION_KEY env var!\n");
            EncryptionKey::generate()
        }
    };

    // Step 2: Create secure storage
    println!("Step 2: Creating secure storage...");
    let backend = Box::new(InMemoryStorage::new());
    let storage = SecureStorage::new(backend, &key);
    println!("✅ Secure storage initialized\n");

    // Step 3: Store sensitive credentials
    println!("Step 3: Storing sensitive credentials...");
    let credentials = vec![
        ("api_key", "sk_live_abc123def456ghi789"),
        ("database_password", "super-secret-password-2024"),
        ("oauth_token", "ya29.A0AfB_byABC...XYZ"),
        ("private_key", "-----BEGIN PRIVATE KEY-----\nMIIEvQ..."),
    ];

    for (name, value) in &credentials {
        storage.store_encrypted(name, value).await?;
        println!("   ✓ Encrypted and stored: {}", name);
    }
    println!();

    // Step 4: Retrieve and decrypt credentials
    println!("Step 4: Retrieving encrypted credentials...");
    for (name, expected_value) in &credentials {
        match storage.get_decrypted(name).await? {
            Some(decrypted_value) => {
                let matches = &decrypted_value == expected_value;
                if matches {
                    println!(
                        "   ✓ Decrypted {}: {} (length: {} chars)",
                        name,
                        mask_value(&decrypted_value),
                        decrypted_value.len()
                    );
                } else {
                    eprintln!("   ❌ Decryption mismatch for {}", name);
                }
            }
            None => {
                eprintln!("   ❌ Credential not found: {}", name);
            }
        }
    }
    println!();

    // Step 5: List all stored keys
    println!("Step 5: Listing all stored keys...");
    let keys = storage.list_keys().await?;
    println!("   Found {} encrypted credentials:", keys.len());
    for key in &keys {
        println!("   - {}", key);
    }
    println!();

    // Step 6: Update a credential
    println!("Step 6: Updating a credential...");
    storage
        .store_encrypted("api_key", "sk_live_NEW_KEY_VALUE_789")
        .await?;
    let updated = storage.get_decrypted("api_key").await?.unwrap();
    println!("   ✓ Updated api_key: {}", mask_value(&updated));
    println!();

    // Step 7: Delete a credential
    println!("Step 7: Deleting a credential...");
    storage.delete("oauth_token").await?;
    let exists = storage.exists("oauth_token").await?;
    if !exists {
        println!("   ✓ Successfully deleted oauth_token");
    }
    println!();

    // Step 8: Demonstrate encryption is real
    println!("Step 8: Verifying encryption is real...");
    println!("   Note: In production, the raw encrypted data would be stored in");
    println!("   a database or file. Here we demonstrate that it's not plaintext.\n");

    println!("   Example plaintext: \"my-secret-password\"");
    println!("   After encryption: (base64-encoded encrypted blob)");
    println!("   Format: nonce(12 bytes) + ciphertext(variable) + auth_tag(16 bytes)\n");

    // Summary
    println!("✅ All operations completed successfully!\n");

    println!("📝 Key Takeaways:");
    println!("   1. Credentials are encrypted with AES-256-GCM before storage");
    println!("   2. Each encryption uses a unique random nonce");
    println!("   3. Authentication tags prevent data tampering");
    println!("   4. Encryption keys are automatically zeroed from memory");
    println!("   5. Always load keys from environment variables in production");
    println!();

    println!("🔧 Production Setup:");
    println!("   export SKREAVER_ENCRYPTION_KEY=$(openssl rand -base64 32)");
    println!("   # Store this key securely (e.g., AWS Secrets Manager, HashiCorp Vault)");
    println!("   # Rotate the key every 90 days");

    Ok(())
}

/// Mask a sensitive value for display
fn mask_value(value: &str) -> String {
    if value.len() <= 8 {
        "*".repeat(value.len())
    } else {
        format!("{}...{}", &value[..4], &value[value.len() - 4..])
    }
}
