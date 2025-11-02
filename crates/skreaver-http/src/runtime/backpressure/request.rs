//! Type-safe request lifecycle management using the typestate pattern.
//!
//! This module provides a compile-time safe state machine for request lifecycle
//! management, ensuring that requests can only transition through valid states.
//!
//! # Typestate Pattern
//!
//! The `Request<S>` type uses the typestate pattern to enforce valid state
//! transitions at compile time. Each state (Queued, Processing, Completed, Failed)
//! is represented by a marker type, and only valid transitions are allowed.

use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use uuid::Uuid;

use super::config::RequestPriority;
use super::error::BackpressureError;

// ============================================================================
// State marker types
// ============================================================================

/// Marker type for Queued state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    pub queued_at: Instant,
    pub timeout: Duration,
    pub input: Option<String>,
}

/// Marker type for Processing state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Processing {
    pub started_at: Instant,
    pub queued_duration: Duration,
}

/// Marker type for Completed state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed {
    pub completed_at: Instant,
    pub processing_time: Duration,
    pub result: String,
}

/// Marker type for Failed state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    pub failed_at: Instant,
    pub error: String,
}

// ============================================================================
// Type-safe request with typestate pattern
// ============================================================================

/// Type-safe request using typestate pattern.
/// The type parameter `S` represents the current state and enforces
/// valid state transitions at compile time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request<S> {
    id: Uuid,
    agent_id: String,
    priority: RequestPriority,
    state: S,
}

// ============================================================================
// Constructors and state-independent methods
// ============================================================================

impl Request<Queued> {
    /// Create a new request in Queued state
    pub fn new(agent_id: String, priority: RequestPriority, timeout: Duration) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_id,
            priority,
            state: Queued {
                queued_at: Instant::now(),
                timeout,
                input: None,
            },
        }
    }

    /// Set input data for the request
    pub fn with_input(mut self, input: String) -> Self {
        self.state.input = Some(input);
        self
    }

    /// Get when queued
    pub fn queued_at(&self) -> Instant {
        self.state.queued_at
    }

    /// Get timeout duration
    pub fn timeout_duration(&self) -> Duration {
        self.state.timeout
    }

    /// Get input data if available
    pub fn input(&self) -> Option<&str> {
        self.state.input.as_deref()
    }

    /// Check if request has timed out
    pub fn has_timed_out(&self) -> bool {
        self.state.queued_at.elapsed() > self.state.timeout
    }

    /// Transition to Processing state
    pub fn start_processing(self) -> Request<Processing> {
        let queued_duration = self.state.queued_at.elapsed();
        Request {
            id: self.id,
            agent_id: self.agent_id,
            priority: self.priority,
            state: Processing {
                started_at: Instant::now(),
                queued_duration,
            },
        }
    }

    /// Transition to Failed state (timeout in queue)
    pub fn fail_timeout(self) -> Request<Failed> {
        Request {
            id: self.id,
            agent_id: self.agent_id,
            priority: self.priority,
            state: Failed {
                failed_at: Instant::now(),
                error: format!(
                    "Request timed out in queue after {:?}",
                    self.state.queued_at.elapsed()
                ),
            },
        }
    }
}

impl<S> Request<S> {
    /// Get the request ID (available in all states)
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get the agent ID (available in all states)
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Get the priority (available in all states)
    pub fn priority(&self) -> RequestPriority {
        self.priority
    }
}

impl Request<Processing> {
    /// Get when processing started
    pub fn started_at(&self) -> Instant {
        self.state.started_at
    }

    /// Get how long request was queued before processing
    pub fn queued_duration(&self) -> Duration {
        self.state.queued_duration
    }

    /// Get current processing duration
    pub fn processing_duration(&self) -> Duration {
        self.state.started_at.elapsed()
    }

    /// Transition to Completed state
    pub fn complete(self, result: String) -> Request<Completed> {
        let processing_time = self.state.started_at.elapsed();
        Request {
            id: self.id,
            agent_id: self.agent_id,
            priority: self.priority,
            state: Completed {
                completed_at: Instant::now(),
                processing_time,
                result,
            },
        }
    }

    /// Transition to Failed state
    pub fn fail(self, error: String) -> Request<Failed> {
        Request {
            id: self.id,
            agent_id: self.agent_id,
            priority: self.priority,
            state: Failed {
                failed_at: Instant::now(),
                error,
            },
        }
    }
}

impl Request<Completed> {
    /// Get when completed
    pub fn completed_at(&self) -> Instant {
        self.state.completed_at
    }

    /// Get processing time
    pub fn processing_time(&self) -> Duration {
        self.state.processing_time
    }

    /// Get result
    pub fn result(&self) -> &str {
        &self.state.result
    }
}

impl Request<Failed> {
    /// Get when failed
    pub fn failed_at(&self) -> Instant {
        self.state.failed_at
    }

    /// Get error message
    pub fn error(&self) -> &str {
        &self.state.error
    }
}

// ============================================================================
// Backward compatibility: Type-erased request for storage
// ============================================================================

/// Type-erased request for queue storage
///
/// This is a simplified version of `Request<Queued>` that stores the essential
/// fields needed for queue management without the typestate marker.
#[derive(Debug, Clone)]
pub struct QueuedRequest {
    pub id: Uuid,
    pub agent_id: String,
    pub priority: RequestPriority,
    pub queued_at: Instant,
    pub timeout: Duration,
    /// Optional input data for the request
    pub input: Option<String>,
}

impl From<Request<Queued>> for QueuedRequest {
    fn from(request: Request<Queued>) -> Self {
        QueuedRequest {
            id: request.id,
            agent_id: request.agent_id,
            priority: request.priority,
            queued_at: request.state.queued_at,
            timeout: request.state.timeout,
            input: request.state.input,
        }
    }
}

impl From<QueuedRequest> for Request<Queued> {
    fn from(request: QueuedRequest) -> Self {
        Request {
            id: request.id,
            agent_id: request.agent_id,
            priority: request.priority,
            state: Queued {
                queued_at: request.queued_at,
                timeout: request.timeout,
                input: request.input,
            },
        }
    }
}

/// Response channel for queued requests
pub type ResponseSender<T> = oneshot::Sender<Result<T, BackpressureError>>;
pub type ResponseReceiver<T> = oneshot::Receiver<Result<T, BackpressureError>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typestate_request_transitions() {
        // Create a request in Queued state
        let request = Request::new(
            "test-agent".to_string(),
            RequestPriority::Normal,
            Duration::from_secs(30),
        );

        assert_eq!(request.agent_id(), "test-agent");
        assert_eq!(request.priority(), RequestPriority::Normal);
        assert!(!request.has_timed_out());

        // Add input
        let request = request.with_input("test input".to_string());
        assert_eq!(request.input(), Some("test input"));

        // Transition to Processing
        let processing_request = request.start_processing();
        assert!(processing_request.processing_duration() < Duration::from_millis(100));

        // Transition to Completed
        let completed_request = processing_request.complete("result".to_string());
        assert_eq!(completed_request.result(), "result");
    }

    #[test]
    fn test_request_timeout_transition() {
        let request = Request::new(
            "test-agent".to_string(),
            RequestPriority::Normal,
            Duration::from_millis(1),
        );

        std::thread::sleep(Duration::from_millis(10));

        assert!(request.has_timed_out());

        let failed_request = request.fail_timeout();
        assert!(failed_request.error().contains("timed out"));
    }

    #[test]
    fn test_processing_to_failed() {
        let request = Request::new(
            "test-agent".to_string(),
            RequestPriority::Normal,
            Duration::from_secs(30),
        );

        let processing = request.start_processing();
        let failed = processing.fail("Processing failed".to_string());

        assert_eq!(failed.error(), "Processing failed");
    }
}
