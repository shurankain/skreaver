//! MCP Server Python bindings.
//!
//! Provides a Python wrapper for running MCP servers with Python tool handlers.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3_async_runtimes::tokio::future_into_py;
use skreaver_core::tool::{ExecutionResult, Tool};
use skreaver_mcp::McpServer as RustMcpServer;
use skreaver_tools::InMemoryToolRegistry;
use std::sync::Arc;

/// A Python callable wrapped as a Skreaver Tool.
///
/// This allows Python functions to be registered as MCP tools.
struct PyToolWrapper {
    name: String,
    description: String,
    input_schema: Option<serde_json::Value>,
    handler: PyObject,
}

impl Tool for PyToolWrapper {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        self.input_schema.clone()
    }

    fn call(&self, input: String) -> ExecutionResult {
        // Parse input as JSON for the Python handler
        let json_input: serde_json::Value = serde_json::from_str(&input).unwrap_or_else(|_| {
            // If not valid JSON, wrap as string
            serde_json::json!({ "input": input })
        });

        // Call the Python handler
        Python::with_gil(|py| {
            // Convert JSON to Python dict
            let py_input = match pythonize::pythonize(py, &json_input) {
                Ok(obj) => obj,
                Err(e) => {
                    return ExecutionResult::failure(format!("Failed to convert input: {}", e));
                }
            };

            // Call the Python function
            match self.handler.call1(py, (py_input,)) {
                Ok(result) => {
                    // Try to convert result back to JSON string
                    match result.extract::<String>(py) {
                        Ok(s) => ExecutionResult::success(s),
                        Err(_) => {
                            // Bind the result for conversion
                            let bound_result = result.bind(py);

                            // Try to convert Python object to JSON
                            match pythonize::depythonize::<serde_json::Value>(bound_result) {
                                Ok(json_result) => ExecutionResult::success(
                                    serde_json::to_string(&json_result)
                                        .unwrap_or_else(|_| "{}".to_string()),
                                ),
                                Err(e) => {
                                    // Fall back to string representation
                                    match bound_result.str() {
                                        Ok(s) => ExecutionResult::success(s.to_string()),
                                        Err(_) => ExecutionResult::failure(format!(
                                            "Failed to convert result: {}",
                                            e
                                        )),
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => ExecutionResult::failure(format!("Tool execution failed: {}", e)),
            }
        })
    }
}

/// MCP Server - exposes tools via Model Context Protocol
///
/// This server can be used with Claude Desktop and other MCP clients.
///
/// Example:
///     >>> from skreaver.mcp import McpServer
///     >>>
///     >>> def my_tool(params):
///     ...     return {"result": params.get("message", "Hello!")}
///     >>>
///     >>> server = McpServer("my-server", "1.0.0")
///     >>> server.add_tool("greet", "Greets the user", my_tool)
///     >>> await server.serve_stdio()
#[pyclass(name = "McpServer")]
pub struct PyMcpServer {
    name: String,
    version: String,
    tools: Vec<Arc<PyToolWrapper>>,
}

#[pymethods]
impl PyMcpServer {
    /// Create a new MCP server
    ///
    /// Args:
    ///     name: Server name (shown to MCP clients)
    ///     version: Server version
    #[new]
    #[pyo3(signature = (name, version="0.1.0"))]
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            tools: Vec::new(),
        }
    }

    /// Server name
    #[getter]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Server version
    #[getter]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Number of registered tools
    #[getter]
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Add a tool to the server
    ///
    /// Args:
    ///     name: Tool name (alphanumeric, _, -)
    ///     description: Human-readable description
    ///     handler: Python callable that takes a dict and returns a result
    ///     input_schema: Optional JSON Schema for input validation
    ///
    /// Example:
    ///     >>> def calculator(params):
    ///     ...     a = params.get("a", 0)
    ///     ...     b = params.get("b", 0)
    ///     ...     return {"sum": a + b}
    ///     >>>
    ///     >>> server.add_tool(
    ///     ...     "calculator",
    ///     ...     "Adds two numbers",
    ///     ...     calculator,
    ///     ...     {"type": "object", "properties": {"a": {"type": "number"}, "b": {"type": "number"}}}
    ///     ... )
    #[pyo3(signature = (name, description, handler, input_schema=None))]
    pub fn add_tool(
        &mut self,
        name: &str,
        description: &str,
        handler: PyObject,
        input_schema: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        // Validate tool name
        if name.is_empty() {
            return Err(crate::errors::McpError::new_err(
                "Tool name cannot be empty",
            ));
        }
        if name.len() > 128 {
            return Err(crate::errors::McpError::new_err(
                "Tool name too long (max 128 characters)",
            ));
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(crate::errors::McpError::new_err(
                "Tool name can only contain alphanumeric characters, underscores, and hyphens",
            ));
        }

        // Check for duplicate names
        if self.tools.iter().any(|t| t.name == name) {
            return Err(crate::errors::McpError::new_err(format!(
                "Tool '{}' already registered",
                name
            )));
        }

        // Convert input schema
        let schema =
            match input_schema {
                Some(dict) => Some(pythonize::depythonize(&dict).map_err(|e| {
                    crate::errors::McpError::new_err(format!("Invalid schema: {}", e))
                })?),
                None => None,
            };

        let tool = PyToolWrapper {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: schema,
            handler,
        };

        self.tools.push(Arc::new(tool));
        Ok(())
    }

    /// List registered tool names
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name.clone()).collect()
    }

    /// Start serving via stdio transport
    ///
    /// This is the standard MCP transport for Claude Desktop.
    /// The server will read JSON-RPC requests from stdin and
    /// write responses to stdout.
    ///
    /// Note: This method blocks until the client disconnects.
    pub fn serve_stdio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let name = self.name.clone();
        let version = self.version.clone();
        let tools: Vec<Arc<PyToolWrapper>> = self.tools.clone();

        future_into_py(py, async move {
            // Build the tool registry
            let registry = InMemoryToolRegistry::new();

            // Create the MCP server
            let mut server = RustMcpServer::with_info(&registry, &name, &version);

            // Add all Python tools
            for tool in tools {
                server.add_tool(tool as Arc<dyn Tool>);
            }

            // Serve via stdio
            server
                .serve_stdio()
                .await
                .map_err(|e| crate::errors::McpError::new_err(e.to_string()))?;

            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "McpServer(name={:?}, version={:?}, tools={})",
            self.name,
            self.version,
            self.tools.len()
        )
    }
}

/// Builder for creating MCP tool definitions
///
/// This provides a fluent API for building tool definitions
/// before adding them to the server.
///
/// Example:
///     >>> from skreaver.mcp import McpToolBuilder
///     >>>
///     >>> tool = (McpToolBuilder("greet")
///     ...     .description("Greets the user")
///     ...     .read_only()
///     ...     .build())
#[pyclass(name = "McpToolBuilder")]
#[derive(Clone)]
pub struct PyMcpToolBuilder {
    name: String,
    description: String,
    input_schema: Option<serde_json::Value>,
    read_only: bool,
    destructive: bool,
    idempotent: bool,
    open_world: bool,
}

#[pymethods]
impl PyMcpToolBuilder {
    /// Create a new tool builder
    #[new]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            input_schema: None,
            read_only: false,
            destructive: false,
            idempotent: false,
            open_world: true,
        }
    }

    /// Set the tool description
    pub fn description(&self, description: &str) -> Self {
        let mut builder = self.clone();
        builder.description = description.to_string();
        builder
    }

    /// Set the input schema
    pub fn input_schema(&self, schema: Bound<'_, PyDict>) -> PyResult<Self> {
        let mut builder = self.clone();
        builder.input_schema = Some(
            pythonize::depythonize(&schema)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        );
        Ok(builder)
    }

    /// Mark as read-only (doesn't modify state)
    pub fn read_only(&self) -> Self {
        let mut builder = self.clone();
        builder.read_only = true;
        builder
    }

    /// Mark as destructive (modifications are destructive)
    pub fn destructive(&self) -> Self {
        let mut builder = self.clone();
        builder.destructive = true;
        builder
    }

    /// Mark as idempotent (repeated calls have same effect)
    pub fn idempotent(&self) -> Self {
        let mut builder = self.clone();
        builder.idempotent = true;
        builder
    }

    /// Mark as closed world (no external interactions)
    pub fn closed_world(&self) -> Self {
        let mut builder = self.clone();
        builder.open_world = false;
        builder
    }

    /// Get the tool name
    #[getter]
    pub fn name(&self) -> &str {
        &self.name
    }

    fn __repr__(&self) -> String {
        format!("McpToolBuilder(name={:?})", self.name)
    }
}
