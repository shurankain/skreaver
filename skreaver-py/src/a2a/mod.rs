//! A2A (Agent-to-Agent) protocol Python bindings.
//!
//! This module provides Python bindings for the A2A protocol types:
//! - Task, TaskStatus
//! - Message, Part
//! - Artifact
//! - AgentCard, AgentSkill
//! - A2aClient (async HTTP client)

pub mod client;
mod types;

pub use client::PyA2aClient;
pub use types::{PyAgentCard, PyAgentSkill, PyArtifact, PyMessage, PyPart, PyTask, PyTaskStatus};
