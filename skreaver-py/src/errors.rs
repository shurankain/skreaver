//! Python exception types for Skreaver errors.
//!
//! Maps Rust error types to Python exceptions.

use pyo3::create_exception;
use pyo3::exceptions::PyException;

// Base exception for all Skreaver errors
create_exception!(skreaver, SkreavorError, PyException);

// A2A protocol errors
create_exception!(skreaver, A2aError, SkreavorError);
create_exception!(skreaver, TaskNotFoundError, A2aError);
create_exception!(skreaver, TaskTerminatedError, A2aError);
create_exception!(skreaver, AuthenticationError, A2aError);
create_exception!(skreaver, RateLimitError, A2aError);

// Gateway errors
create_exception!(skreaver, GatewayError, SkreavorError);
create_exception!(skreaver, ProtocolDetectionError, GatewayError);
create_exception!(skreaver, TranslationError, GatewayError);

// Memory errors
create_exception!(skreaver, MemoryError, SkreavorError);

// MCP errors
create_exception!(skreaver, McpError, SkreavorError);
