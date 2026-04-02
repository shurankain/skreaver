//! # Skreaver Python Bindings
//!
//! PyO3-based Python bindings for Skreaver's core protocol types.
//!
//! This crate exposes Skreaver's high-performance MCP and A2A protocol
//! infrastructure to Python developers via native bindings.
//!
//! ## Modules
//!
//! - `a2a` - Agent-to-Agent protocol types (Task, Message, AgentCard)
//! - `gateway` - Protocol translation (MCP <-> A2A)
//! - `mcp` - Model Context Protocol server
//! - `memory` - Persistent memory backends

// Allow clippy warnings that are spurious for PyO3 bindings
#![allow(clippy::useless_conversion)]

use pyo3::prelude::*;

pub mod a2a;
pub mod errors;
pub mod gateway;
pub mod mcp;
pub mod memory;

/// Skreaver Python module
///
/// High-performance MCP and A2A protocol infrastructure for AI agents.
#[pymodule]
fn _skreaver(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register submodules
    register_a2a_module(py, m)?;
    register_gateway_module(py, m)?;
    register_mcp_module(py, m)?;
    register_memory_module(py, m)?;
    register_exceptions(py, m)?;

    // Add version info
    m.add("__version__", "0.7.0")?;

    Ok(())
}

/// Register A2A submodule
fn register_a2a_module(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let a2a_module = PyModule::new(py, "a2a")?;

    // Types
    a2a_module.add_class::<a2a::PyTaskStatus>()?;
    a2a_module.add_class::<a2a::PyTask>()?;
    a2a_module.add_class::<a2a::PyMessage>()?;
    a2a_module.add_class::<a2a::PyPart>()?;
    a2a_module.add_class::<a2a::PyArtifact>()?;
    a2a_module.add_class::<a2a::PyAgentCard>()?;
    a2a_module.add_class::<a2a::PyAgentSkill>()?;

    parent.add_submodule(&a2a_module)?;

    // Also expose core types at top level for convenience
    parent.add_class::<a2a::PyTaskStatus>()?;
    parent.add_class::<a2a::PyTask>()?;
    parent.add_class::<a2a::PyMessage>()?;
    parent.add_class::<a2a::PyAgentCard>()?;

    Ok(())
}

/// Register Gateway submodule
fn register_gateway_module(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let gateway_module = PyModule::new(py, "gateway")?;

    gateway_module.add_class::<gateway::PyProtocol>()?;
    gateway_module.add_class::<gateway::PyProtocolDetector>()?;
    gateway_module.add_class::<gateway::PyProtocolGateway>()?;

    parent.add_submodule(&gateway_module)?;

    // Also expose at top level
    parent.add_class::<gateway::PyProtocol>()?;
    parent.add_class::<gateway::PyProtocolGateway>()?;

    Ok(())
}

/// Register MCP submodule
fn register_mcp_module(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let mcp_module = PyModule::new(py, "mcp")?;

    // TODO: Add MCP server wrapper in step 12

    parent.add_submodule(&mcp_module)?;
    Ok(())
}

/// Register Memory submodule
fn register_memory_module(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let memory_module = PyModule::new(py, "memory")?;

    // TODO: Add memory backends in steps 10-11

    parent.add_submodule(&memory_module)?;
    Ok(())
}

/// Register custom exceptions
fn register_exceptions(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let exceptions_module = PyModule::new(py, "exceptions")?;

    exceptions_module.add("SkreavorError", py.get_type::<errors::SkreavorError>())?;
    exceptions_module.add("A2aError", py.get_type::<errors::A2aError>())?;
    exceptions_module.add(
        "TaskNotFoundError",
        py.get_type::<errors::TaskNotFoundError>(),
    )?;
    exceptions_module.add("GatewayError", py.get_type::<errors::GatewayError>())?;

    parent.add_submodule(&exceptions_module)?;
    Ok(())
}
