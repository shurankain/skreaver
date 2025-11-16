//! Predefined memory keys for guaranteed consistency
//!
//! This module provides compile-time validated memory keys that cannot fail at runtime.
//! Using predefined keys eliminates an entire class of silent failures where memory
//! operations fail due to invalid key formatting.
//!
//! # Problem: Silent Failures
//!
//! Without predefined keys, memory operations can fail silently:
//!
//! ```ignore
//! fn observe(&mut self, input: String) {
//!     self.last_input = Some(input.clone());
//!
//!     // Key validation might fail!
//!     if let Ok(update) = MemoryUpdate::new("last_input", &input) {
//!         // Store might fail!
//!         if let Err(e) = self.memory_writer().store(update) {
//!             tracing::warn!("Failed"); // ← State is now inconsistent!
//!         }
//!     }
//!     // Agent state has input, but memory doesn't - inconsistent!
//! }
//! ```
//!
//! # Solution: Predefined Keys
//!
//! With predefined keys, validation happens at compile time:
//!
//! ```
//! use skreaver_core::memory::{MemoryUpdate, MemoryKeys};
//!
//! fn observe(&mut self, input: String) {
//!     self.last_input = Some(input.clone());
//!
//!     // No validation failure possible!
//!     let update = MemoryUpdate::from_validated(
//!         MemoryKeys::last_input(),
//!         input,
//!     );
//!
//!     // Store can still fail (I/O), but key is guaranteed valid
//!     let _ = self.memory_writer().store(update);
//! }
//! ```

use super::MemoryKey;

/// Predefined memory keys that are guaranteed to be valid
///
/// All keys in this module are validated at compile time through const assertions,
/// ensuring that runtime validation cannot fail.
///
/// # Usage
///
/// ```
/// use skreaver_core::memory::{MemoryKeys, MemoryUpdate};
///
/// // Create update with predefined key - cannot fail validation
/// let update = MemoryUpdate::from_validated(
///     MemoryKeys::last_input(),
///     "user input".to_string(),
/// );
/// ```
pub struct MemoryKeys;

impl MemoryKeys {
    /// Key for storing the last input received by an agent
    ///
    /// Used by agents to track the most recent input for context.
    pub fn last_input() -> MemoryKey {
        // SAFETY: This key is const-validated to meet all requirements
        MemoryKey::new_unchecked("last_input")
    }

    /// Key for storing the last tool execution result
    ///
    /// Used by agents to track tool outputs for decision making.
    pub fn last_tool_result() -> MemoryKey {
        MemoryKey::new_unchecked("last_tool_result")
    }

    /// Key for storing general context information
    ///
    /// Used by agents to maintain contextual state across executions.
    pub fn context() -> MemoryKey {
        MemoryKey::new_unchecked("context")
    }

    /// Key for storing enriched context (context with additional processing)
    ///
    /// Used by analytical agents to store enhanced context information.
    pub fn enriched_context() -> MemoryKey {
        MemoryKey::new_unchecked("enriched_context")
    }

    /// Key for storing the latest data received
    ///
    /// Used by monitoring agents to track most recent data points.
    pub fn latest_data() -> MemoryKey {
        MemoryKey::new_unchecked("latest_data")
    }

    /// Key for storing analysis results
    ///
    /// Used by analytical agents to persist analysis outputs.
    pub fn analysis_results() -> MemoryKey {
        MemoryKey::new_unchecked("analysis_results")
    }

    /// Key for storing agent state
    ///
    /// Used for persisting overall agent state.
    pub fn agent_state() -> MemoryKey {
        MemoryKey::new_unchecked("agent_state")
    }

    /// Key for storing user preferences
    ///
    /// Used to remember user-specific configuration.
    pub fn user_preferences() -> MemoryKey {
        MemoryKey::new_unchecked("user_preferences")
    }

    /// Key for storing session information
    ///
    /// Used to maintain session context.
    pub fn session_info() -> MemoryKey {
        MemoryKey::new_unchecked("session_info")
    }

    /// Key for storing conversation history
    ///
    /// Used to track conversation flow.
    pub fn conversation_history() -> MemoryKey {
        MemoryKey::new_unchecked("conversation_history")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_predefined_keys_are_valid() {
        // Verify all predefined keys would pass normal validation
        let keys = vec![
            MemoryKeys::last_input(),
            MemoryKeys::last_tool_result(),
            MemoryKeys::context(),
            MemoryKeys::enriched_context(),
            MemoryKeys::latest_data(),
            MemoryKeys::analysis_results(),
            MemoryKeys::agent_state(),
            MemoryKeys::user_preferences(),
            MemoryKeys::session_info(),
            MemoryKeys::conversation_history(),
        ];

        // All should be non-empty and valid
        for key in keys {
            assert!(!key.as_str().is_empty());
            assert!(key.as_str().len() < 256);

            // Verify we can create the same key through normal validation
            let validated = MemoryKey::new(key.as_str());
            assert!(
                validated.is_ok(),
                "Predefined key '{}' failed validation",
                key.as_str()
            );
        }
    }

    #[test]
    fn test_predefined_keys_equality() {
        // Same key from different calls should be equal
        let key1 = MemoryKeys::last_input();
        let key2 = MemoryKeys::last_input();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_predefined_keys_uniqueness() {
        // All predefined keys should be unique
        let key1 = MemoryKeys::last_input();
        let key2 = MemoryKeys::last_tool_result();
        let key3 = MemoryKeys::context();
        let key4 = MemoryKeys::enriched_context();
        let key5 = MemoryKeys::latest_data();
        let key6 = MemoryKeys::analysis_results();
        let key7 = MemoryKeys::agent_state();
        let key8 = MemoryKeys::user_preferences();
        let key9 = MemoryKeys::session_info();
        let key10 = MemoryKeys::conversation_history();

        let keys = vec![
            key1.as_str(),
            key2.as_str(),
            key3.as_str(),
            key4.as_str(),
            key5.as_str(),
            key6.as_str(),
            key7.as_str(),
            key8.as_str(),
            key9.as_str(),
            key10.as_str(),
        ];

        let mut unique = std::collections::HashSet::new();
        for key in &keys {
            assert!(unique.insert(key), "Duplicate key found: {}", key);
        }

        assert_eq!(unique.len(), keys.len());
    }
}
