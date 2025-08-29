//! Memory core types and traits.
//!
//! This module re-exports all memory-related types from skreaver-core
//! to maintain API compatibility while consolidating implementation.

// Re-export all core memory types and traits
pub use skreaver_core::memory::{
    InvalidMemoryKey, MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory,
    TransactionalMemory,
};
