//! A2A type bindings for Python.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use skreaver_a2a::{
    AgentCard as RustAgentCard, AgentSkill as RustAgentSkill, Artifact as RustArtifact,
    Message as RustMessage, Part as RustPart, Task as RustTask, TaskStatus as RustTaskStatus,
};

/// Task status enum
#[pyclass(name = "TaskStatus", eq, eq_int)]
#[derive(Clone, PartialEq)]
pub enum PyTaskStatus {
    Working = 0,
    Completed = 1,
    Failed = 2,
    Cancelled = 3,
    Rejected = 4,
    InputRequired = 5,
}

impl From<RustTaskStatus> for PyTaskStatus {
    fn from(status: RustTaskStatus) -> Self {
        match status {
            RustTaskStatus::Working => PyTaskStatus::Working,
            RustTaskStatus::Completed => PyTaskStatus::Completed,
            RustTaskStatus::Failed => PyTaskStatus::Failed,
            RustTaskStatus::Cancelled => PyTaskStatus::Cancelled,
            RustTaskStatus::Rejected => PyTaskStatus::Rejected,
            RustTaskStatus::InputRequired => PyTaskStatus::InputRequired,
        }
    }
}

impl From<PyTaskStatus> for RustTaskStatus {
    fn from(status: PyTaskStatus) -> Self {
        match status {
            PyTaskStatus::Working => RustTaskStatus::Working,
            PyTaskStatus::Completed => RustTaskStatus::Completed,
            PyTaskStatus::Failed => RustTaskStatus::Failed,
            PyTaskStatus::Cancelled => RustTaskStatus::Cancelled,
            PyTaskStatus::Rejected => RustTaskStatus::Rejected,
            PyTaskStatus::InputRequired => RustTaskStatus::InputRequired,
        }
    }
}

#[pymethods]
impl PyTaskStatus {
    /// Check if this status represents a terminal state
    fn is_terminal(&self) -> bool {
        matches!(
            self,
            PyTaskStatus::Completed
                | PyTaskStatus::Failed
                | PyTaskStatus::Cancelled
                | PyTaskStatus::Rejected
        )
    }

    fn __repr__(&self) -> String {
        match self {
            PyTaskStatus::Working => "TaskStatus.Working".to_string(),
            PyTaskStatus::Completed => "TaskStatus.Completed".to_string(),
            PyTaskStatus::Failed => "TaskStatus.Failed".to_string(),
            PyTaskStatus::Cancelled => "TaskStatus.Cancelled".to_string(),
            PyTaskStatus::Rejected => "TaskStatus.Rejected".to_string(),
            PyTaskStatus::InputRequired => "TaskStatus.InputRequired".to_string(),
        }
    }
}

/// A2A Task - core unit of work
#[pyclass(name = "Task")]
#[derive(Clone)]
pub struct PyTask {
    pub(crate) inner: RustTask,
}

#[pymethods]
impl PyTask {
    /// Create a new task with optional ID
    #[new]
    #[pyo3(signature = (id=None))]
    fn new(id: Option<String>) -> Self {
        let task = match id {
            Some(id) => RustTask::new(&id),
            None => RustTask::new_with_uuid(),
        };
        Self { inner: task }
    }

    /// Task ID
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Task status
    #[getter]
    fn status(&self) -> PyTaskStatus {
        self.inner.status.into()
    }

    /// Set task status
    #[setter]
    fn set_status(&mut self, status: PyTaskStatus) {
        self.inner.set_status(status.into());
    }

    /// Context ID (session)
    #[getter]
    fn context_id(&self) -> Option<String> {
        self.inner.context_id.clone()
    }

    /// Add a message to the task
    fn add_message(&mut self, message: &PyMessage) {
        self.inner.add_message(message.inner.clone());
    }

    /// Add an artifact to the task
    fn add_artifact(&mut self, artifact: &PyArtifact) {
        self.inner.add_artifact(artifact.inner.clone());
    }

    /// Check if task is in terminal state
    fn is_terminal(&self) -> bool {
        self.inner.is_terminal()
    }

    /// Check if task requires user input
    fn requires_input(&self) -> bool {
        self.inner.requires_input()
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
        let task: RustTask = serde_json::from_value(json)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner: task })
    }

    fn __repr__(&self) -> String {
        format!(
            "Task(id={:?}, status={:?})",
            self.inner.id, self.inner.status
        )
    }
}

/// A2A Message - communication between agents
#[pyclass(name = "Message")]
#[derive(Clone)]
pub struct PyMessage {
    pub(crate) inner: RustMessage,
}

#[pymethods]
impl PyMessage {
    /// Create a user message
    #[staticmethod]
    fn user(text: &str) -> Self {
        Self {
            inner: RustMessage::user(text),
        }
    }

    /// Create an agent message
    #[staticmethod]
    fn agent(text: &str) -> Self {
        Self {
            inner: RustMessage::agent(text),
        }
    }

    /// Message ID
    #[getter]
    fn id(&self) -> Option<String> {
        self.inner.id.clone()
    }

    /// Message role (user or agent)
    #[getter]
    fn role(&self) -> String {
        format!("{:?}", self.inner.role).to_lowercase()
    }

    /// Get text content (first text part)
    #[getter]
    fn text(&self) -> Option<String> {
        self.inner.parts.iter().find_map(|part| {
            if let RustPart::Text(text_part) = part {
                Some(text_part.text.clone())
            } else {
                None
            }
        })
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
            "Message(role={:?}, id={:?})",
            self.inner.role, self.inner.id
        )
    }
}

/// A2A Part - content part of a message
#[pyclass(name = "Part")]
#[derive(Clone)]
pub struct PyPart {
    pub(crate) inner: RustPart,
}

#[pymethods]
impl PyPart {
    /// Create a text part
    #[staticmethod]
    fn text(content: &str) -> Self {
        Self {
            inner: RustPart::text(content),
        }
    }

    /// Create a file part
    #[staticmethod]
    fn file(url: &str, mime_type: &str) -> Self {
        Self {
            inner: RustPart::file(url, mime_type),
        }
    }

    /// Part type (text, file, or data)
    #[getter]
    fn part_type(&self) -> String {
        match &self.inner {
            RustPart::Text(_) => "text".to_string(),
            RustPart::File(_) => "file".to_string(),
            RustPart::Data(_) => "data".to_string(),
        }
    }

    fn __repr__(&self) -> String {
        format!("Part(type={:?})", self.part_type())
    }
}

/// A2A Artifact - output produced by a task
#[pyclass(name = "Artifact")]
#[derive(Clone)]
pub struct PyArtifact {
    pub(crate) inner: RustArtifact,
}

#[pymethods]
impl PyArtifact {
    /// Create a text artifact with auto-generated ID
    #[staticmethod]
    fn text(content: &str) -> Self {
        Self {
            inner: RustArtifact::text(uuid::Uuid::new_v4().to_string(), content),
        }
    }

    /// Artifact ID
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Artifact label
    #[getter]
    fn label(&self) -> Option<String> {
        self.inner.label.clone()
    }

    /// Set artifact label (returns new instance)
    fn with_label(&self, label: &str) -> Self {
        Self {
            inner: self.inner.clone().with_label(label),
        }
    }

    /// Artifact description
    #[getter]
    fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }

    /// Set artifact description (returns new instance)
    fn with_description(&self, desc: &str) -> Self {
        Self {
            inner: self.inner.clone().with_description(desc),
        }
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
        format!("Artifact(id={:?})", self.inner.id)
    }
}

/// A2A AgentCard - describes agent capabilities
#[pyclass(name = "AgentCard")]
#[derive(Clone)]
pub struct PyAgentCard {
    pub(crate) inner: RustAgentCard,
}

#[pymethods]
impl PyAgentCard {
    /// Create a new agent card
    #[new]
    fn new(agent_id: &str, name: &str, base_url: &str) -> Self {
        Self {
            inner: RustAgentCard::new(agent_id, name, base_url),
        }
    }

    /// Agent ID
    #[getter]
    fn agent_id(&self) -> String {
        self.inner.agent_id.clone()
    }

    /// Agent name
    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    /// Agent description
    #[getter]
    fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }

    /// Set description (returns new instance)
    fn with_description(&self, desc: &str) -> Self {
        Self {
            inner: self.inner.clone().with_description(desc),
        }
    }

    /// Enable streaming capability (returns new instance)
    fn with_streaming(&self) -> Self {
        Self {
            inner: self.inner.clone().with_streaming(),
        }
    }

    /// Enable push notifications capability (returns new instance)
    fn with_push_notifications(&self) -> Self {
        Self {
            inner: self.inner.clone().with_push_notifications(),
        }
    }

    /// Add a skill to the agent (returns new instance)
    fn with_skill(&self, skill: &PyAgentSkill) -> Self {
        Self {
            inner: self.inner.clone().with_skill(skill.inner.clone()),
        }
    }

    /// Check if streaming is supported
    fn supports_streaming(&self) -> bool {
        self.inner.capabilities.streaming
    }

    /// Check if push notifications are supported
    fn supports_push_notifications(&self) -> bool {
        self.inner.capabilities.push_notifications
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
        let card: RustAgentCard = serde_json::from_value(json)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner: card })
    }

    fn __repr__(&self) -> String {
        format!(
            "AgentCard(id={:?}, name={:?})",
            self.inner.agent_id, self.inner.name
        )
    }
}

/// A2A AgentSkill - describes a skill the agent can perform
#[pyclass(name = "AgentSkill")]
#[derive(Clone)]
pub struct PyAgentSkill {
    pub(crate) inner: RustAgentSkill,
}

#[pymethods]
impl PyAgentSkill {
    /// Create a new skill
    #[new]
    fn new(id: &str, name: &str) -> Self {
        Self {
            inner: RustAgentSkill::new(id, name),
        }
    }

    /// Skill ID
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Skill name
    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    /// Skill description
    #[getter]
    fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }

    /// Set description (returns new instance)
    fn with_description(&self, desc: &str) -> Self {
        Self {
            inner: self.inner.clone().with_description(desc),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "AgentSkill(id={:?}, name={:?})",
            self.inner.id, self.inner.name
        )
    }
}
