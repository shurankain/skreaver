//! Protocol Gateway Python bindings.
//!
//! This module provides Python bindings for protocol translation:
//! - Protocol enum (MCP, A2A)
//! - ProtocolDetector
//! - ProtocolGateway

use pyo3::prelude::*;
use pyo3::types::PyDict;
use skreaver_gateway::{
    Protocol as RustProtocol, ProtocolDetector as RustDetector, ProtocolGateway as RustGateway,
};

/// Protocol type enum
#[pyclass(name = "Protocol", eq, eq_int)]
#[derive(Clone, PartialEq)]
pub enum PyProtocol {
    Mcp = 0,
    A2a = 1,
}

impl From<RustProtocol> for PyProtocol {
    fn from(protocol: RustProtocol) -> Self {
        match protocol {
            RustProtocol::Mcp => PyProtocol::Mcp,
            RustProtocol::A2a => PyProtocol::A2a,
        }
    }
}

impl From<PyProtocol> for RustProtocol {
    fn from(protocol: PyProtocol) -> Self {
        match protocol {
            PyProtocol::Mcp => RustProtocol::Mcp,
            PyProtocol::A2a => RustProtocol::A2a,
        }
    }
}

#[pymethods]
impl PyProtocol {
    fn __repr__(&self) -> String {
        match self {
            PyProtocol::Mcp => "Protocol.Mcp".to_string(),
            PyProtocol::A2a => "Protocol.A2a".to_string(),
        }
    }
}

/// Protocol detector - identifies message format
#[pyclass(name = "ProtocolDetector")]
#[derive(Clone)]
pub struct PyProtocolDetector {
    inner: RustDetector,
}

#[pymethods]
impl PyProtocolDetector {
    /// Create a new protocol detector
    #[new]
    fn new() -> Self {
        Self {
            inner: RustDetector::new(),
        }
    }

    /// Create a strict protocol detector
    #[staticmethod]
    fn strict() -> Self {
        Self {
            inner: RustDetector::strict(),
        }
    }

    /// Detect protocol from a Python dict (JSON object)
    fn detect(&self, message: Bound<'_, PyDict>) -> PyResult<PyProtocol> {
        let json: serde_json::Value = pythonize::depythonize(&message)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        self.inner
            .detect(&json)
            .map(|p| p.into())
            .map_err(|e| crate::errors::ProtocolDetectionError::new_err(e.to_string()))
    }

    /// Detect protocol from a JSON string
    fn detect_str(&self, json_str: &str) -> PyResult<PyProtocol> {
        self.inner
            .detect_str(json_str)
            .map(|p| p.into())
            .map_err(|e| crate::errors::ProtocolDetectionError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        "ProtocolDetector()".to_string()
    }
}

/// Protocol gateway - translates between MCP and A2A
#[pyclass(name = "ProtocolGateway")]
#[derive(Clone)]
pub struct PyProtocolGateway {
    inner: RustGateway,
}

#[pymethods]
impl PyProtocolGateway {
    /// Create a new protocol gateway
    #[new]
    fn new() -> Self {
        Self {
            inner: RustGateway::new(),
        }
    }

    /// Detect protocol and translate to target protocol
    fn translate_to(
        &self,
        py: Python<'_>,
        message: Bound<'_, PyDict>,
        target: PyProtocol,
    ) -> PyResult<PyObject> {
        let json: serde_json::Value = pythonize::depythonize(&message)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let result = self
            .inner
            .translate_to(json, target.into())
            .map_err(|e| crate::errors::TranslationError::new_err(e.to_string()))?;

        pythonize::pythonize(py, &result)
            .map(|bound| bound.unbind())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// Detect protocol and translate to the opposite protocol
    fn translate_opposite(&self, py: Python<'_>, message: Bound<'_, PyDict>) -> PyResult<PyObject> {
        let json: serde_json::Value = pythonize::depythonize(&message)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let result = self
            .inner
            .translate_opposite(json)
            .map_err(|e| crate::errors::TranslationError::new_err(e.to_string()))?;

        pythonize::pythonize(py, &result)
            .map(|bound| bound.unbind())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// Detect the protocol of a message
    fn detect(&self, message: Bound<'_, PyDict>) -> PyResult<PyProtocol> {
        let json: serde_json::Value = pythonize::depythonize(&message)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        self.inner
            .detector
            .detect(&json)
            .map(|p| p.into())
            .map_err(|e| crate::errors::ProtocolDetectionError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        "ProtocolGateway()".to_string()
    }
}
