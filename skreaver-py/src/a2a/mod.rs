//! A2A (Agent-to-Agent) protocol Python bindings.
//!
//! This module provides Python bindings for the A2A protocol types:
//! - Task, TaskStatus
//! - Message, Part
//! - Artifact
//! - AgentCard, AgentSkill

mod types;

pub use types::{PyAgentCard, PyAgentSkill, PyArtifact, PyMessage, PyPart, PyTask, PyTaskStatus};

// Client module will be added in step 8
// pub mod client;
