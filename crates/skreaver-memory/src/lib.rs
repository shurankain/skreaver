//! # Skreaver Memory Backends
//!
//! Memory backend implementations for Skreaver agent infrastructure.
//! Provides persistent and transient storage options for agent state and context.
//!
//! ## Backends
//!
//! - **[FileMemory]**: Persistent file-based storage with JSON serialization  
//! - **[NamespacedMemory]**: Wrapper providing key namespacing for any backend
//! - **[RedisMemory]**: Redis-based distributed memory (requires `redis` feature)
//!
//! Note: `InMemoryMemory` is available in `skreaver-core` as the default implementation.
//!
//! ## Feature Flags
//!
//! - `redis`: Enable Redis backend support
//! - `sqlite`: Enable SQLite backend support (future)
//! - `postgres`: Enable PostgreSQL backend support (future)
//!
//! ## Example
//!
//! ```rust
//! use skreaver_memory::{FileMemory, NamespacedMemory};
//! use skreaver_core::{InMemoryMemory, MemoryReader, MemoryWriter, MemoryUpdate};
//! use skreaver_core::memory::MemoryKey;
//!
//! // In-memory storage for development (from skreaver-core)
//! let mut memory = InMemoryMemory::new();
//!
//! // File-based storage for persistence
//! let mut file_memory = FileMemory::new("agent_state.json");
//!
//! // Namespaced storage to isolate agent data
//! let mut namespaced = NamespacedMemory::new("agent_001", memory);
//! ```

// Re-export core memory traits
pub use skreaver_core::memory::*;

// Always available memory backends
mod file_memory;
pub use file_memory::FileMemory;

mod namespaced_memory;
pub use namespaced_memory::NamespacedMemory;

// Conditional memory backends
#[cfg(feature = "redis")]
mod redis_memory;
#[cfg(feature = "redis")]
pub use redis_memory::RedisMemory;

// Future backends (placeholders - will be implemented in future versions)
