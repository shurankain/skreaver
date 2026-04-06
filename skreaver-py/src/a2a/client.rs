//! A2A Client async bindings for Python.
//!
//! This module provides async Python bindings for the A2A protocol client,
//! enabling Python developers to interact with A2A-compatible agents.

use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use skreaver_a2a::client::{A2aClient, AuthConfig};

use super::types::{PyAgentCard, PyMessage, PyTask};

/// A2A Protocol Client - communicates with A2A-compatible agents
///
/// This client handles:
/// - Agent discovery (fetching agent cards)
/// - Task creation and management
/// - Message sending (sync and streaming)
/// - Authentication (bearer tokens, API keys)
///
/// # Example (Python)
///
/// ```python
/// import asyncio
/// from skreaver import A2aClient
///
/// async def main():
///     client = A2aClient("https://agent.example.com")
///     card = await client.get_agent_card()
///     print(f"Connected to: {card.name}")
///
///     task = await client.send_message("Hello, agent!")
///     print(f"Task: {task.id}, Status: {task.status}")
///
/// asyncio.run(main())
/// ```
#[pyclass(name = "A2aClient")]
pub struct PyA2aClient {
    inner: A2aClient,
}

#[pymethods]
impl PyA2aClient {
    /// Create a new A2A client for the given agent URL
    #[new]
    fn new(url: &str) -> PyResult<Self> {
        let client = A2aClient::new(url)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner: client })
    }

    /// Get the base URL of the agent
    #[getter]
    fn base_url(&self) -> String {
        self.inner.base_url().to_string()
    }

    /// Set bearer token authentication (returns new client)
    ///
    /// # Parameters
    ///
    /// * `token` - The bearer token (JWT, OAuth2, etc.)
    ///
    /// # Returns
    ///
    /// A new client with bearer token auth configured
    fn with_bearer_token(&self, token: &str) -> Self {
        Self {
            inner: self.inner.clone().with_bearer_token(token),
        }
    }

    /// Set API key authentication in header (returns new client)
    ///
    /// # Parameters
    ///
    /// * `header_name` - The header name (e.g., "X-API-Key")
    /// * `api_key` - The API key value
    ///
    /// # Returns
    ///
    /// A new client with API key auth configured
    fn with_api_key(&self, header_name: &str, api_key: &str) -> Self {
        Self {
            inner: self.inner.clone().with_api_key(header_name, api_key),
        }
    }

    /// Set API key authentication in query parameter (returns new client)
    ///
    /// # Parameters
    ///
    /// * `param_name` - The query parameter name
    /// * `api_key` - The API key value
    ///
    /// # Returns
    ///
    /// A new client with API key query auth configured
    fn with_api_key_query(&self, param_name: &str, api_key: &str) -> Self {
        Self {
            inner: self.inner.clone().with_auth(AuthConfig::ApiKeyQuery {
                name: param_name.to_string(),
                value: api_key.to_string(),
            }),
        }
    }

    /// Fetch the agent card (async)
    ///
    /// The agent card describes the agent's capabilities, skills, and
    /// how to interact with it.
    ///
    /// # Returns
    ///
    /// A coroutine that resolves to an AgentCard
    fn get_agent_card<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        future_into_py(py, async move {
            let card = client
                .get_agent_card()
                .await
                .map_err(|e| crate::errors::A2aError::new_err(e.to_string()))?;
            Ok(PyAgentCard { inner: card })
        })
    }

    /// Send a message to the agent (async)
    ///
    /// This creates a new task with the user's message.
    ///
    /// # Parameters
    ///
    /// * `text` - The message text to send
    ///
    /// # Returns
    ///
    /// A coroutine that resolves to a Task
    fn send_message<'py>(&self, py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        let text = text.to_string();
        future_into_py(py, async move {
            let task = client
                .send_message(&text)
                .await
                .map_err(|e| crate::errors::A2aError::new_err(e.to_string()))?;
            Ok(PyTask { inner: task })
        })
    }

    /// Continue an existing task with a new message (async)
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to continue
    /// * `text` - The message text to send
    ///
    /// # Returns
    ///
    /// A coroutine that resolves to the updated Task
    fn continue_task<'py>(
        &self,
        py: Python<'py>,
        task_id: &str,
        text: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        let task_id = task_id.to_string();
        let text = text.to_string();
        future_into_py(py, async move {
            let task = client
                .continue_task(task_id, &text)
                .await
                .map_err(|e| crate::errors::A2aError::new_err(e.to_string()))?;
            Ok(PyTask { inner: task })
        })
    }

    /// Get the current state of a task (async)
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to fetch
    ///
    /// # Returns
    ///
    /// A coroutine that resolves to the Task
    fn get_task<'py>(&self, py: Python<'py>, task_id: &str) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        let task_id = task_id.to_string();
        future_into_py(py, async move {
            let task = client
                .get_task(&task_id)
                .await
                .map_err(|e| crate::errors::A2aError::new_err(e.to_string()))?;
            Ok(PyTask { inner: task })
        })
    }

    /// Cancel a running task (async)
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to cancel
    /// * `reason` - Optional reason for cancellation
    ///
    /// # Returns
    ///
    /// A coroutine that resolves to the cancelled Task
    #[pyo3(signature = (task_id, reason=None))]
    fn cancel_task<'py>(
        &self,
        py: Python<'py>,
        task_id: &str,
        reason: Option<&str>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        let task_id = task_id.to_string();
        let reason = reason.map(|s| s.to_string());
        future_into_py(py, async move {
            let task = client
                .cancel_task(task_id, reason)
                .await
                .map_err(|e| crate::errors::A2aError::new_err(e.to_string()))?;
            Ok(PyTask { inner: task })
        })
    }

    /// Wait for a task to complete (async)
    ///
    /// Polls the task status until it reaches a terminal state.
    ///
    /// # Parameters
    ///
    /// * `task_id` - The ID of the task to wait for
    /// * `poll_interval_ms` - How often to poll for updates (milliseconds)
    /// * `timeout_ms` - Maximum time to wait (milliseconds)
    ///
    /// # Returns
    ///
    /// A coroutine that resolves to the completed Task
    #[pyo3(signature = (task_id, poll_interval_ms=5000, timeout_ms=300000))]
    fn wait_for_task<'py>(
        &self,
        py: Python<'py>,
        task_id: &str,
        poll_interval_ms: u64,
        timeout_ms: u64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        let task_id = task_id.to_string();
        let poll_interval = std::time::Duration::from_millis(poll_interval_ms);
        let timeout = std::time::Duration::from_millis(timeout_ms);
        future_into_py(py, async move {
            let task = client
                .wait_for_task(&task_id, poll_interval, timeout)
                .await
                .map_err(|e| crate::errors::A2aError::new_err(e.to_string()))?;
            Ok(PyTask { inner: task })
        })
    }

    /// Send a message with full control (async)
    ///
    /// # Parameters
    ///
    /// * `message` - A Message object to send
    /// * `task_id` - Optional task ID to continue
    /// * `context_id` - Optional context ID for grouping tasks
    ///
    /// # Returns
    ///
    /// A coroutine that resolves to the Task
    #[pyo3(signature = (message, task_id=None, context_id=None))]
    fn send<'py>(
        &self,
        py: Python<'py>,
        message: &PyMessage,
        task_id: Option<&str>,
        context_id: Option<&str>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.inner.clone();
        let message = message.inner.clone();
        let task_id = task_id.map(|s| s.to_string());
        let context_id = context_id.map(|s| s.to_string());
        future_into_py(py, async move {
            let task = client
                .send(message, task_id, context_id)
                .await
                .map_err(|e| crate::errors::A2aError::new_err(e.to_string()))?;
            Ok(PyTask { inner: task })
        })
    }

    fn __repr__(&self) -> String {
        format!("A2aClient(url={:?})", self.inner.base_url().as_str())
    }
}
