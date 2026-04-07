//! MCP (Model Context Protocol) Python bindings.
//!
//! This module provides Python bindings for MCP types:
//! - McpServer - MCP server with Python tool handlers
//! - McpTaskStatus - Task lifecycle status
//! - McpTask - Long-running operation tracking
//! - McpToolAnnotations - Tool behavior hints
//! - McpToolDefinition - Tool metadata

mod server;

pub use server::{PyMcpServer, PyMcpToolBuilder};

use pyo3::prelude::*;
use pyo3::types::PyDict;
use skreaver_mcp::{
    McpTask as RustMcpTask, McpTaskStatus as RustMcpTaskStatus,
    McpToolAnnotations as RustAnnotations, McpToolDefinition as RustToolDefinition,
};

/// MCP Task Status - lifecycle states for long-running operations
#[pyclass(name = "McpTaskStatus", eq, eq_int)]
#[derive(Clone, PartialEq)]
pub enum PyMcpTaskStatus {
    /// Receiver accepted and is working on the request
    Working = 0,
    /// Receiver needs additional input before continuing
    InputRequired = 1,
    /// Operation completed successfully
    Completed = 2,
    /// Operation failed
    Failed = 3,
    /// Task was cancelled
    Cancelled = 4,
}

impl From<RustMcpTaskStatus> for PyMcpTaskStatus {
    fn from(status: RustMcpTaskStatus) -> Self {
        match status {
            RustMcpTaskStatus::Working => PyMcpTaskStatus::Working,
            RustMcpTaskStatus::InputRequired => PyMcpTaskStatus::InputRequired,
            RustMcpTaskStatus::Completed => PyMcpTaskStatus::Completed,
            RustMcpTaskStatus::Failed => PyMcpTaskStatus::Failed,
            RustMcpTaskStatus::Cancelled => PyMcpTaskStatus::Cancelled,
        }
    }
}

impl From<PyMcpTaskStatus> for RustMcpTaskStatus {
    fn from(status: PyMcpTaskStatus) -> Self {
        match status {
            PyMcpTaskStatus::Working => RustMcpTaskStatus::Working,
            PyMcpTaskStatus::InputRequired => RustMcpTaskStatus::InputRequired,
            PyMcpTaskStatus::Completed => RustMcpTaskStatus::Completed,
            PyMcpTaskStatus::Failed => RustMcpTaskStatus::Failed,
            PyMcpTaskStatus::Cancelled => RustMcpTaskStatus::Cancelled,
        }
    }
}

#[pymethods]
impl PyMcpTaskStatus {
    /// Check if this status is terminal (no further transitions)
    fn is_terminal(&self) -> bool {
        matches!(
            self,
            PyMcpTaskStatus::Completed | PyMcpTaskStatus::Failed | PyMcpTaskStatus::Cancelled
        )
    }

    fn __repr__(&self) -> String {
        match self {
            PyMcpTaskStatus::Working => "McpTaskStatus.Working".to_string(),
            PyMcpTaskStatus::InputRequired => "McpTaskStatus.InputRequired".to_string(),
            PyMcpTaskStatus::Completed => "McpTaskStatus.Completed".to_string(),
            PyMcpTaskStatus::Failed => "McpTaskStatus.Failed".to_string(),
            PyMcpTaskStatus::Cancelled => "McpTaskStatus.Cancelled".to_string(),
        }
    }

    fn __str__(&self) -> String {
        match self {
            PyMcpTaskStatus::Working => "working".to_string(),
            PyMcpTaskStatus::InputRequired => "inputRequired".to_string(),
            PyMcpTaskStatus::Completed => "completed".to_string(),
            PyMcpTaskStatus::Failed => "failed".to_string(),
            PyMcpTaskStatus::Cancelled => "cancelled".to_string(),
        }
    }
}

/// MCP Task - tracks long-running operations (2025-11-25 spec)
#[pyclass(name = "McpTask")]
#[derive(Clone)]
pub struct PyMcpTask {
    pub(crate) inner: RustMcpTask,
}

#[pymethods]
impl PyMcpTask {
    /// Create a new MCP task
    #[new]
    #[pyo3(signature = (task_id, ttl=None))]
    fn new(task_id: &str, ttl: Option<u64>) -> Self {
        Self {
            inner: RustMcpTask::new(task_id, ttl),
        }
    }

    /// Task ID
    #[getter]
    fn task_id(&self) -> String {
        self.inner.task_id.clone()
    }

    /// Current status
    #[getter]
    fn status(&self) -> PyMcpTaskStatus {
        self.inner.status.into()
    }

    /// Status message
    #[getter]
    fn status_message(&self) -> Option<String> {
        self.inner.status_message.clone()
    }

    /// Creation timestamp (ISO-8601)
    #[getter]
    fn created_at(&self) -> String {
        self.inner.created_at.to_rfc3339()
    }

    /// Last update timestamp (ISO-8601)
    #[getter]
    fn last_updated_at(&self) -> Option<String> {
        self.inner.last_updated_at.map(|dt| dt.to_rfc3339())
    }

    /// TTL in milliseconds
    #[getter]
    fn ttl(&self) -> Option<u64> {
        self.inner.ttl
    }

    /// Suggested polling interval in milliseconds
    #[getter]
    fn poll_interval(&self) -> Option<u64> {
        self.inner.poll_interval
    }

    /// Task result (if completed)
    #[getter]
    fn result(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        match &self.inner.result {
            Some(value) => {
                let py_obj = pythonize::pythonize(py, value)
                    .map(|bound| bound.unbind())
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                Ok(Some(py_obj))
            }
            None => Ok(None),
        }
    }

    /// Check if task is in terminal state
    fn is_terminal(&self) -> bool {
        self.inner.is_terminal()
    }

    /// Check if task has expired based on TTL
    fn is_expired(&self) -> bool {
        self.inner.is_expired()
    }

    /// Convert to Python dict
    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let json = serde_json::to_value(&self.inner)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        pythonize::pythonize(py, &json)
            .map(|bound| bound.unbind())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!(
            "McpTask(task_id={:?}, status={:?})",
            self.inner.task_id, self.inner.status
        )
    }
}

/// MCP Tool Annotations - behavior hints (2025-11-25 spec)
#[pyclass(name = "McpToolAnnotations")]
#[derive(Clone)]
pub struct PyMcpToolAnnotations {
    pub(crate) inner: RustAnnotations,
}

#[pymethods]
impl PyMcpToolAnnotations {
    /// Create new tool annotations
    #[new]
    fn new() -> Self {
        Self {
            inner: RustAnnotations::default(),
        }
    }

    /// Whether the tool only reads without modifying its environment
    #[getter]
    fn read_only_hint(&self) -> Option<bool> {
        self.inner.read_only_hint
    }

    /// Set read-only hint (returns new instance)
    fn with_read_only(&self, value: bool) -> Self {
        let mut inner = self.inner.clone();
        inner.read_only_hint = Some(value);
        Self { inner }
    }

    /// Whether modifications are destructive vs additive
    #[getter]
    fn destructive_hint(&self) -> Option<bool> {
        self.inner.destructive_hint
    }

    /// Set destructive hint (returns new instance)
    fn with_destructive(&self, value: bool) -> Self {
        let mut inner = self.inner.clone();
        inner.destructive_hint = Some(value);
        Self { inner }
    }

    /// Whether repeated identical calls produce no additional effects
    #[getter]
    fn idempotent_hint(&self) -> Option<bool> {
        self.inner.idempotent_hint
    }

    /// Set idempotent hint (returns new instance)
    fn with_idempotent(&self, value: bool) -> Self {
        let mut inner = self.inner.clone();
        inner.idempotent_hint = Some(value);
        Self { inner }
    }

    /// Whether the tool interacts with external entities
    #[getter]
    fn open_world_hint(&self) -> Option<bool> {
        self.inner.open_world_hint
    }

    /// Set open-world hint (returns new instance)
    fn with_open_world(&self, value: bool) -> Self {
        let mut inner = self.inner.clone();
        inner.open_world_hint = Some(value);
        Self { inner }
    }

    /// Convert to Python dict
    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let json = serde_json::to_value(&self.inner)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        pythonize::pythonize(py, &json)
            .map(|bound| bound.unbind())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!(
            "McpToolAnnotations(read_only={:?}, destructive={:?}, idempotent={:?}, open_world={:?})",
            self.inner.read_only_hint,
            self.inner.destructive_hint,
            self.inner.idempotent_hint,
            self.inner.open_world_hint
        )
    }
}

/// MCP Tool Definition - describes a tool's interface (2025-11-25 spec)
#[pyclass(name = "McpToolDefinition")]
#[derive(Clone)]
pub struct PyMcpToolDefinition {
    pub(crate) inner: RustToolDefinition,
}

#[pymethods]
impl PyMcpToolDefinition {
    /// Create a new tool definition
    #[new]
    #[pyo3(signature = (name, description, input_schema=None))]
    fn new(
        name: &str,
        description: &str,
        input_schema: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let schema = match input_schema {
            Some(dict) => pythonize::depythonize(&dict)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
            None => serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        };

        Ok(Self {
            inner: RustToolDefinition {
                name: name.to_string(),
                title: None,
                description: description.to_string(),
                input_schema: schema,
                output_schema: None,
                annotations: None,
            },
        })
    }

    /// Tool name
    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    /// Tool title
    #[getter]
    fn title(&self) -> Option<String> {
        self.inner.title.clone()
    }

    /// Set title (returns new instance)
    fn with_title(&self, title: &str) -> Self {
        let mut inner = self.inner.clone();
        inner.title = Some(title.to_string());
        Self { inner }
    }

    /// Tool description
    #[getter]
    fn description(&self) -> String {
        self.inner.description.clone()
    }

    /// Input schema (JSON Schema)
    #[getter]
    fn input_schema(&self, py: Python<'_>) -> PyResult<PyObject> {
        pythonize::pythonize(py, &self.inner.input_schema)
            .map(|bound| bound.unbind())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// Output schema (JSON Schema)
    #[getter]
    fn output_schema(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        match &self.inner.output_schema {
            Some(schema) => {
                let py_obj = pythonize::pythonize(py, schema)
                    .map(|bound| bound.unbind())
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                Ok(Some(py_obj))
            }
            None => Ok(None),
        }
    }

    /// Set output schema (returns new instance)
    fn with_output_schema(&self, schema: Bound<'_, PyDict>) -> PyResult<Self> {
        let schema_value: serde_json::Value = pythonize::depythonize(&schema)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let mut inner = self.inner.clone();
        inner.output_schema = Some(schema_value);
        Ok(Self { inner })
    }

    /// Tool annotations
    #[getter]
    fn annotations(&self) -> Option<PyMcpToolAnnotations> {
        self.inner
            .annotations
            .clone()
            .map(|a| PyMcpToolAnnotations { inner: a })
    }

    /// Set annotations (returns new instance)
    fn with_annotations(&self, annotations: &PyMcpToolAnnotations) -> Self {
        let mut inner = self.inner.clone();
        inner.annotations = Some(annotations.inner.clone());
        Self { inner }
    }

    /// Convert to Python dict
    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let json = serde_json::to_value(&self.inner)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        pythonize::pythonize(py, &json)
            .map(|bound| bound.unbind())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// Create from Python dict
    #[staticmethod]
    fn from_dict(dict: Bound<'_, PyDict>) -> PyResult<Self> {
        let json: serde_json::Value = pythonize::depythonize(&dict)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let inner: RustToolDefinition = serde_json::from_value(json)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "McpToolDefinition(name={:?}, description={:?})",
            self.inner.name, self.inner.description
        )
    }
}
