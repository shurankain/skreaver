//! Type definitions for HTTP runtime
//!
//! This module contains all the request and response types, as well as
//! common data structures used by the HTTP runtime endpoints.

pub mod requests;
pub mod responses;

// Re-export all types for convenience
pub use requests::*;
pub use responses::*;
