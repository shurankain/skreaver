//! Non-empty collections for compile-time guarantees.
//!
//! This module provides collection types that prevent empty states at compile time,
//! enabling safer APIs by making invalid states unrepresentable.

pub mod non_empty_queue;
pub mod non_empty_vec;

pub use non_empty_queue::NonEmptyQueue;
pub use non_empty_vec::NonEmptyVec;
