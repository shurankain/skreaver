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
//! - **[Memory]**: Basic load/store operations for key-value data (legacy, use MemoryReader/Writer)
//! - **[MemoryReader]**: Read-only memory operations with concurrent access support
//! - **[MemoryWriter]**: Write-only memory operations with batch support
//! - **[TransactionalMemory]**: Atomic transaction support for consistent updates
//! - **[SnapshotableMemory]**: Extended trait for backup and restore capabilities
//!
//! ## Usage
//!
//! ```rust
//! use skreaver::memory::{MemoryReader, MemoryWriter, MemoryUpdate, InMemoryMemory, MemoryKey};
//!
//! let mut memory = InMemoryMemory::new();
//!
//! // Write operations require mutable access
//! memory.store(MemoryUpdate {
//!     key: MemoryKey::new("context").unwrap(),
//!     value: "conversation state".into(),
//! }).unwrap();
//!
//! // Read operations can use immutable access
//! let key = MemoryKey::new("context").unwrap();
//! let value = memory.load(&key).unwrap();
//! ```

/// Core memory trait definitions and data structures.
mod core;
mod file_memory;
mod in_memory;
mod namespaced;
mod redis_memory;

pub use core::{
    InvalidMemoryKey, MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory,
    TransactionalMemory,
};
pub use file_memory::FileMemory;
pub use in_memory::InMemoryMemory;
pub use namespaced::NamespacedMemory;
pub use redis_memory::RedisMemory;
