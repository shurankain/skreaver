//! Example: JWT Token Revocation with Blacklist
//!
//! This example demonstrates how to implement JWT token revocation using a blacklist.
//! When a token is compromised or a user logs out, the token can be revoked to prevent
//! further use, even if it hasn't expired yet.
//!
//! # Key Features
//!
//! - Token generation with JTI (JWT ID) for tracking
//! - In-memory blacklist for development/testing
//! - Automatic TTL management (tokens auto-expire from blacklist)
//! - Revocation of both access and refresh tokens
//!
//! # Usage
//!
//! ```bash
//! cargo run --example jwt_token_revocation
//! ```
//!
//! For production deployments, consider using `RedisBlacklist` instead of
//! `InMemoryBlacklist` for distributed token revocation across multiple instances.

use skreaver_core::auth::{
    AuthMethod, InMemoryBlacklist, JwtConfig, JwtManager, Principal, Role, TokenBlacklist,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 JWT Token Revocation Example\n");

    // Step 1: Setup JWT Manager with blacklist
    println!("Step 1: Setting up JWT manager with token revocation...");

    // Using in-memory blacklist (for testing/example purposes)
    // For production, consider using RedisBlacklist for distributed token revocation
    let blacklist = {
        println!("   Using in-memory blacklist (for testing)");
        Arc::new(InMemoryBlacklist::new())
    };

    let config = JwtConfig {
        expiry_minutes: 5, // 5-minute expiry for demo
        ..JwtConfig::default()
    };
    let manager = JwtManager::with_blacklist(config, blacklist.clone());

    println!("✅ JWT manager initialized with revocation support\n");

    // Step 2: Create users and generate tokens
    println!("Step 2: Generating JWT tokens for multiple users...");

    let alice = Principal::new(
        "alice-123".to_string(),
        "Alice Smith".to_string(),
        AuthMethod::ApiKey("alice-key".to_string()),
    )
    .with_role(Role::Admin);

    let bob = Principal::new(
        "bob-456".to_string(),
        "Bob Jones".to_string(),
        AuthMethod::ApiKey("bob-key".to_string()),
    )
    .with_role(Role::Agent);

    let alice_token = manager.generate(&alice).await?;
    let bob_token = manager.generate(&bob).await?;

    println!("   ✓ Generated token for Alice (admin)");
    println!("   ✓ Generated token for Bob (agent)");
    println!();

    // Step 3: Verify tokens work
    println!("Step 3: Verifying tokens are valid...");

    let alice_auth = manager.authenticate(&alice_token.access_token).await?;
    println!("   ✓ Alice's token authenticated: {}", alice_auth.name);

    let bob_auth = manager.authenticate(&bob_token.access_token).await?;
    println!("   ✓ Bob's token authenticated: {}", bob_auth.name);
    println!();

    // Step 4: Security incident - Alice's token is compromised!
    println!("Step 4: Security incident! Alice's token was compromised...");
    println!("   Revoking Alice's access token immediately...");

    manager.revoke(&alice_token.access_token).await?;

    println!("   ✅ Alice's token has been revoked");
    println!();

    // Step 5: Verify revocation
    println!("Step 5: Verifying token revocation...");

    match manager.authenticate(&alice_token.access_token).await {
        Ok(_) => println!("   ❌ ERROR: Alice's token still works (should be revoked!)"),
        Err(e) => {
            println!("   ✓ Alice's token rejected: {}", e);
        }
    }

    match manager.authenticate(&bob_token.access_token).await {
        Ok(auth) => println!("   ✓ Bob's token still works: {}", auth.name),
        Err(_) => println!("   ❌ ERROR: Bob's token was incorrectly revoked!"),
    }
    println!();

    // Step 6: Refresh token revocation
    println!("Step 6: Refresh token revocation scenario...");

    if let Some(alice_refresh) = alice_token.refresh_token {
        println!("   Alice attempts to refresh using her refresh token...");

        // Revoke the refresh token
        manager.revoke(&alice_refresh).await?;
        println!("   ✓ Alice's refresh token has been revoked");

        // Try to use it
        match manager.refresh(&alice_refresh).await {
            Ok(_) => println!("   ❌ ERROR: Refresh succeeded (should fail!)"),
            Err(e) => println!("   ✓ Refresh rejected: {}", e),
        }
    }
    println!();

    // Step 7: Blacklist statistics
    println!("Step 7: Blacklist statistics...");
    let count = blacklist.count().await?;
    println!("   Revoked tokens in blacklist: {}", count);
    println!("   Note: Tokens auto-expire from blacklist after their TTL");
    println!();

    // Step 8: User logout scenario
    println!("Step 8: User logout scenario...");
    println!("   Bob logs out voluntarily...");

    // Revoke both access and refresh tokens
    manager.revoke(&bob_token.access_token).await?;
    if let Some(ref refresh) = bob_token.refresh_token {
        manager.revoke(refresh).await?;
    }

    println!("   ✓ Bob's access token revoked");
    println!("   ✓ Bob's refresh token revoked");

    // Verify both tokens are invalid
    match manager.authenticate(&bob_token.access_token).await {
        Ok(_) => println!("   ❌ ERROR: Access token still works"),
        Err(_) => println!("   ✓ Access token correctly rejected"),
    }

    let final_count = blacklist.count().await?;
    println!("\n   Final blacklist count: {}", final_count);
    println!();

    // Step 9: Production best practices
    println!("📝 Production Best Practices:\n");
    println!("1. Token Lifecycle:");
    println!("   - Generate tokens with short expiry (5-15 minutes for access)");
    println!("   - Use longer expiry for refresh tokens (7-30 days)");
    println!("   - Always revoke both access and refresh on logout\n");

    println!("2. Revocation Strategy:");
    println!("   - Revoke on user logout");
    println!("   - Revoke on password change");
    println!("   - Revoke on security breach detection");
    println!("   - Revoke on permission changes\n");

    println!("3. Performance:");
    println!("   - Use Redis in production (with connection pooling)");
    println!("   - Set TTL to token expiry time (auto-cleanup)");
    println!("   - Monitor blacklist size");
    println!("   - Consider token families for refresh rotation\n");

    println!("4. Monitoring:");
    println!("   - Track revocation events");
    println!("   - Alert on mass revocations (security breach)");
    println!("   - Monitor blacklist hit rate");
    println!("   - Log failed authentication attempts\n");

    println!("✅ Example completed successfully!");

    Ok(())
}
