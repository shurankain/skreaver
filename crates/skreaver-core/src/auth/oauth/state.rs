//! CSRF state parameter storage for OAuth flows.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::pkce::PkceChallenge;
use super::types::OAuthError;

/// Stores PKCE challenges keyed by state parameter.
///
/// `take()` removes the entry on read (one-time use) to prevent replay.
pub trait StateStore: Send + Sync {
    fn store(&self, state: &str, pkce: PkceChallenge, ttl: Duration) -> Result<(), OAuthError>;
    fn take(&self, state: &str) -> Result<PkceChallenge, OAuthError>;
}

struct Entry {
    pkce: PkceChallenge,
    expires_at: Instant,
}

/// In-memory state store with TTL-based expiration.
pub struct InMemoryStateStore {
    entries: Mutex<HashMap<String, Entry>>,
}

impl InMemoryStateStore {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Remove expired entries.
    fn cleanup(&self, map: &mut HashMap<String, Entry>) {
        let now = Instant::now();
        map.retain(|_, entry| entry.expires_at > now);
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl StateStore for InMemoryStateStore {
    fn store(&self, state: &str, pkce: PkceChallenge, ttl: Duration) -> Result<(), OAuthError> {
        let mut map = self
            .entries
            .lock()
            .map_err(|_| OAuthError::PkceError("State store lock poisoned".into()))?;
        self.cleanup(&mut map);
        map.insert(
            state.to_string(),
            Entry {
                pkce,
                expires_at: Instant::now() + ttl,
            },
        );
        Ok(())
    }

    fn take(&self, state: &str) -> Result<PkceChallenge, OAuthError> {
        let mut map = self
            .entries
            .lock()
            .map_err(|_| OAuthError::PkceError("State store lock poisoned".into()))?;
        self.cleanup(&mut map);
        let entry = map.remove(state).ok_or(OAuthError::InvalidState)?;
        if entry.expires_at < Instant::now() {
            return Err(OAuthError::StateExpired);
        }
        Ok(entry.pkce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_take() {
        let store = InMemoryStateStore::new();
        let pkce = PkceChallenge::generate();
        let challenge = pkce.code_challenge.clone();

        store
            .store("state-123", pkce, Duration::from_secs(60))
            .unwrap();
        let retrieved = store.take("state-123").unwrap();
        assert_eq!(retrieved.code_challenge, challenge);
    }

    #[test]
    fn test_take_removes_entry() {
        let store = InMemoryStateStore::new();
        store
            .store(
                "state-1",
                PkceChallenge::generate(),
                Duration::from_secs(60),
            )
            .unwrap();
        store.take("state-1").unwrap();

        // Second take should fail
        assert!(store.take("state-1").is_err());
    }

    #[test]
    fn test_unknown_state_fails() {
        let store = InMemoryStateStore::new();
        assert!(store.take("nonexistent").is_err());
    }

    #[test]
    fn test_expired_state_fails() {
        let store = InMemoryStateStore::new();
        store
            .store("old", PkceChallenge::generate(), Duration::from_millis(1))
            .unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(store.take("old").is_err());
    }
}
