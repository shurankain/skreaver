//! # Agents Module
//!
//! This module contains reference implementations of various agent types built
//! on the Skreaver framework. These agents demonstrate different patterns and
//! capabilities for building AI systems.
//!
//! ## Available Agents
//!
//! - **[echo]**: Simple echo agent for testing basic agent functionality
//! - **[multi_tool]**: Agent showcasing multiple tool usage patterns
//! - **[reasoning]**: Advanced reasoning agent with chain-of-thought capabilities
//! - **[standard_tools]**: Interactive agent demonstrating standard tool library
//!
//! ## Usage
//!
//! Each agent module provides a `run_*` function for CLI execution:
//!
//! ```rust
//! use skreaver_cli::agents::{run_echo_agent, run_reasoning_agent};
//!
//! // Run different agent types
//! run_echo_agent();
//! run_reasoning_agent();
//! ```

pub mod echo;
pub mod multi_tool;
pub mod reasoning;
pub mod standard_tools;

pub use echo::run_echo_agent;
pub use multi_tool::run_multi_agent;
pub use reasoning::run_reasoning_agent;
pub use standard_tools::run_standard_tools_agent;
