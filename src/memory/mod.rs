//! # Memory Module
//!
//! This module provides persistent storage capabilities for agents. Memory allows agents
//! to maintain state across interactions, learn from past experiences, and build context
//! over time.
//!
//! ## Memory Types
//!
//! - **[InMemoryMemory]**: Fast, transient storage for development and testing
//! - **[FileMemory]**: JSON-based file persistence for simple deployments
//! - **[RedisMemory]**: Distributed storage with Redis backend for production
//! - **[NamespacedMemory]**: Key-value isolation for multi-tenant scenarios
//!
//! ## Core Traits
//!
//! - **[Memory]**: Basic load/store operations for key-value data
//! - **[SnapshotableMemory]**: Extended trait for backup and restore capabilities
//!
//! ## Usage
//!
//! ```rust
//! use skreaver::memory::{Memory, MemoryUpdate, InMemoryMemory};
//!
//! let mut memory = InMemoryMemory::new();
//! memory.store(MemoryUpdate {
//!     key: "context".into(),
//!     value: "conversation state".into(),
//! });
//! ```

mod file_memory;
mod in_memory;
mod namespaced;
mod redis_memory;
/// Core memory trait definitions and data structures.
mod r#trait;

pub use file_memory::FileMemory;
pub use in_memory::InMemoryMemory;
pub use namespaced::NamespacedMemory;
pub use redis_memory::RedisMemory;
pub use r#trait::{Memory, MemoryUpdate, SnapshotableMemory};
