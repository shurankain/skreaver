//! Structured Tracing and Session Correlation
//!
//! Provides distributed tracing capabilities with session correlation and
//! tool execution tracking as specified in DEVELOPMENT_PLAN.md.

use crate::tags::{AgentId, CardinalTags, SessionId, ToolName};
use crate::{ObservabilityConfig, ObservabilityError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Global session tracker instance
static SESSION_TRACKER: OnceLock<Arc<SessionTracker>> = OnceLock::new();

/// Session tracking and trace correlation
#[derive(Debug)]
pub struct SessionTracker {
    active_sessions: Mutex<HashMap<SessionId, SessionContext>>,
}

impl Default for SessionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionTracker {
    /// Create new session tracker
    pub fn new() -> Self {
        Self {
            active_sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Start a new agent session with trace context
    pub fn start_session(&self, agent_id: AgentId) -> Result<SessionId, TracingError> {
        let session_id = SessionId::generate();
        let context = SessionContext::new(agent_id.clone(), session_id.clone());

        let mut sessions = self
            .active_sessions
            .lock()
            .map_err(|_| TracingError::LockError("Failed to acquire session lock".to_string()))?;

        sessions.insert(session_id.clone(), context);

        // Create root span for session
        #[cfg(feature = "tracing")]
        {
            let span = tracing::info_span!(
                "agent_session",
                agent.id = %agent_id,
                session.id = %session_id,
                otel.name = "agent_session"
            );
            let _enter = span.enter();
            tracing::info!("Starting agent session");
        }

        Ok(session_id)
    }

    /// End an agent session
    pub fn end_session(&self, session_id: &SessionId) -> Result<(), TracingError> {
        let mut sessions = self
            .active_sessions
            .lock()
            .map_err(|_| TracingError::LockError("Failed to acquire session lock".to_string()))?;

        if let Some(context) = sessions.remove(session_id) {
            #[cfg(feature = "tracing")]
            {
                let span = tracing::info_span!(
                    "agent_session_end",
                    agent.id = %context.agent_id,
                    session.id = %session_id,
                    otel.name = "agent_session_end"
                );
                let _enter = span.enter();
                tracing::info!("Ending agent session");
            }
        }

        Ok(())
    }

    /// Start tool execution span within session
    pub fn start_tool_execution(
        &self,
        session_id: &SessionId,
        tool_name: &ToolName,
    ) -> Result<ToolSpan, TracingError> {
        let sessions = self
            .active_sessions
            .lock()
            .map_err(|_| TracingError::LockError("Failed to acquire session lock".to_string()))?;

        let context = sessions
            .get(session_id)
            .ok_or_else(|| TracingError::SessionNotFound(session_id.clone()))?;

        #[cfg(feature = "tracing")]
        let span = tracing::info_span!(
            "tool_execution",
            agent.id = %context.agent_id,
            session.id = %session_id,
            tool.name = %tool_name,
            otel.name = "tool_execution"
        );

        Ok(ToolSpan::new(
            session_id.clone(),
            tool_name.clone(),
            #[cfg(feature = "tracing")]
            span,
        ))
    }

    /// Get current session context
    pub fn get_session_context(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionContext>, TracingError> {
        let sessions = self
            .active_sessions
            .lock()
            .map_err(|_| TracingError::LockError("Failed to acquire session lock".to_string()))?;

        Ok(sessions.get(session_id).cloned())
    }

    /// Get active session count
    pub fn active_session_count(&self) -> Result<usize, TracingError> {
        let sessions = self
            .active_sessions
            .lock()
            .map_err(|_| TracingError::LockError("Failed to acquire session lock".to_string()))?;

        Ok(sessions.len())
    }
}

/// Session context for trace correlation
#[derive(Debug, Clone)]
pub struct SessionContext {
    pub agent_id: AgentId,
    pub session_id: SessionId,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub tags: CardinalTags,
}

impl SessionContext {
    /// Create new session context
    pub fn new(agent_id: AgentId, session_id: SessionId) -> Self {
        let tags = CardinalTags::for_agent_session(agent_id.clone(), session_id.clone());

        Self {
            agent_id,
            session_id,
            start_time: chrono::Utc::now(),
            tags,
        }
    }
}

/// Tool execution span for structured tracing
pub struct ToolSpan {
    session_id: SessionId,
    tool_name: ToolName,
    #[cfg(feature = "tracing")]
    span: tracing::Span,
    start_time: std::time::Instant,
}

impl ToolSpan {
    /// Create new tool span
    fn new(
        session_id: SessionId,
        tool_name: ToolName,
        #[cfg(feature = "tracing")] span: tracing::Span,
    ) -> Self {
        Self {
            session_id,
            tool_name,
            #[cfg(feature = "tracing")]
            span,
            start_time: std::time::Instant::now(),
        }
    }

    /// Get session ID
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Get tool name
    pub fn tool_name(&self) -> &ToolName {
        &self.tool_name
    }

    /// Record tool execution success
    pub fn success(self) {
        let duration = self.start_time.elapsed();

        #[cfg(feature = "tracing")]
        {
            let _enter = self.span.enter();
            tracing::info!(
                duration_ms = duration.as_millis() as u64,
                status = "success",
                "Tool execution completed successfully"
            );
        }
    }

    /// Record tool execution error
    pub fn error(self, error_msg: &str) {
        let duration = self.start_time.elapsed();

        #[cfg(feature = "tracing")]
        {
            let _enter = self.span.enter();
            tracing::error!(
                duration_ms = duration.as_millis() as u64,
                status = "error",
                error = error_msg,
                "Tool execution failed"
            );
        }
    }

    /// Get tool execution duration so far
    pub fn duration(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

/// Trace context for correlation across services
#[derive(Debug, Clone)]
pub struct TraceContext {
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}

impl TraceContext {
    /// Create trace context from session
    pub fn from_session(session_id: SessionId, agent_id: AgentId) -> Self {
        Self {
            session_id,
            agent_id,
            trace_id: None,
            span_id: None,
        }
    }

    /// Create trace context with OpenTelemetry IDs
    pub fn with_otel_ids(
        session_id: SessionId,
        agent_id: AgentId,
        trace_id: String,
        span_id: String,
    ) -> Self {
        Self {
            session_id,
            agent_id,
            trace_id: Some(trace_id),
            span_id: Some(span_id),
        }
    }

    /// Convert to cardinal tags
    pub fn to_tags(&self) -> CardinalTags {
        CardinalTags::for_agent_session(self.agent_id.clone(), self.session_id.clone())
    }
}

/// Initialize tracing subsystem
pub fn init_tracing(config: &ObservabilityConfig) -> Result<(), ObservabilityError> {
    let session_tracker = Arc::new(SessionTracker::new());
    SESSION_TRACKER
        .set(session_tracker)
        .map_err(|_| ObservabilityError::TracingInit("Already initialized".to_string()))?;

    #[cfg(feature = "tracing")]
    {
        // Set up tracing subscriber with sampling
        use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();

        tracing::info!(
            namespace = config.namespace,
            sampling.error = config.log_sampling.error_sample_rate,
            sampling.warn = config.log_sampling.warn_sample_rate,
            sampling.info = config.log_sampling.info_sample_rate,
            sampling.debug = config.log_sampling.debug_sample_rate,
            "Initialized structured tracing"
        );
    }

    Ok(())
}

/// Get global session tracker
pub fn get_session_tracker() -> Option<Arc<SessionTracker>> {
    SESSION_TRACKER.get().cloned()
}

/// Tracing system errors
#[derive(thiserror::Error, Debug)]
pub enum TracingError {
    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Span creation failed: {0}")]
    SpanCreation(String),

    #[error("Tracing not initialized")]
    NotInitialized,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::AgentId;

    #[test]
    fn test_session_lifecycle() {
        let tracker = SessionTracker::new();
        let agent_id = AgentId::new("test-agent").unwrap();

        // Start session
        let session_id = tracker.start_session(agent_id.clone()).unwrap();
        assert_eq!(tracker.active_session_count().unwrap(), 1);

        // Get context
        let context = tracker.get_session_context(&session_id).unwrap();
        assert!(context.is_some());
        assert_eq!(context.unwrap().agent_id, agent_id);

        // End session
        tracker.end_session(&session_id).unwrap();
        assert_eq!(tracker.active_session_count().unwrap(), 0);
    }

    #[test]
    fn test_tool_span_creation() {
        let tracker = SessionTracker::new();
        let agent_id = AgentId::new("test-agent").unwrap();
        let tool_name = crate::tags::ToolName::new("test_tool").unwrap();

        let session_id = tracker.start_session(agent_id).unwrap();
        let span = tracker
            .start_tool_execution(&session_id, &tool_name)
            .unwrap();

        // Test span lifecycle
        assert!(span.duration() < std::time::Duration::from_millis(10));
        span.success();
    }

    #[test]
    fn test_trace_context() {
        let agent_id = AgentId::new("test-agent").unwrap();
        let session_id = SessionId::generate();

        let context = TraceContext::from_session(session_id.clone(), agent_id.clone());
        assert_eq!(context.session_id, session_id);
        assert_eq!(context.agent_id, agent_id);
        assert!(context.trace_id.is_none());

        let tags = context.to_tags();
        assert_eq!(tags.agent_id, Some(agent_id));
        assert_eq!(tags.session_id, Some(session_id));
    }
}
