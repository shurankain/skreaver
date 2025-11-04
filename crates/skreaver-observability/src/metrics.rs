//! Core Metrics Collection
//!
//! Implements the metrics system as specified in DEVELOPMENT_PLAN.md with
//! strict cardinality controls and production-ready Prometheus integration.

use crate::LATENCY_BUCKETS;
use crate::tags::{CardinalTags, ErrorKind, MemoryOp, ToolName};
use prometheus::{
    CounterVec, Gauge, HistogramOpts, HistogramVec, Opts, Registry, register_counter_vec,
    register_gauge, register_histogram_vec,
};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;
use thiserror::Error;

/// Global metrics registry instance
static METRICS_REGISTRY: OnceLock<Arc<MetricsRegistry>> = OnceLock::new();

/// Core metrics as defined in DEVELOPMENT_PLAN.md
#[derive(Debug)]
pub struct CoreMetrics {
    // Agent metrics
    pub agent_sessions_active: Gauge,   // cardinality: 1
    pub agent_errors_total: CounterVec, // cardinality: ≤10

    // Tool metrics
    pub tool_exec_total: CounterVec,              // cardinality: ≤20
    pub tool_exec_duration_seconds: HistogramVec, // cardinality: ≤20

    // Memory metrics
    pub memory_ops_total: CounterVec, // cardinality: 4

    // HTTP metrics (for skreaver-http integration)
    pub http_requests_total: CounterVec, // cardinality: ≤30
    pub http_request_duration_seconds: HistogramVec, // cardinality: ≤30
    pub http_requests_by_status: CounterVec, // cardinality: ≤30 (route, method, status_code)
    pub http_request_size_bytes: HistogramVec, // cardinality: ≤30 (route, method)
    pub http_response_size_bytes: HistogramVec, // cardinality: ≤30 (route, method)
    pub http_requests_in_flight: Gauge,  // cardinality: 1

    // Per-agent metrics
    pub agent_requests_total: CounterVec, // cardinality: dynamic (agent_id, operation)
    pub agent_errors_by_type: CounterVec, // cardinality: dynamic (agent_id, error_type)
    pub agent_tool_executions: CounterVec, // cardinality: dynamic (agent_id, tool)

    // Security metrics (GAP-003 & GAP-004 resolution)
    pub security_auth_attempts_total: CounterVec, // cardinality: ≤5 (result: success|failure|invalid)
    pub security_rbac_checks_total: CounterVec,   // cardinality: ≤5 (result: allowed|denied)
    pub security_policy_violations_total: CounterVec, // cardinality: ≤10 (violation_type)
    pub security_resource_limit_exceeded_total: CounterVec, // cardinality: ≤5 (resource_type)
    pub security_rate_limit_exceeded_total: CounterVec, // cardinality: ≤4 (limit_type: global|ip|user|endpoint)
}

impl CoreMetrics {
    /// Initialize core metrics with namespace
    pub fn new(namespace: &str) -> Result<Self, MetricsError> {
        let agent_sessions_active = register_gauge!(Opts::new(
            format!("{}_agent_sessions_active", namespace),
            "Number of active agent sessions"
        ))?;

        let agent_errors_total = register_counter_vec!(
            Opts::new(
                format!("{}_agent_errors_total", namespace),
                "Total number of agent errors by kind"
            ),
            &["kind"]
        )?;

        let tool_exec_total = register_counter_vec!(
            Opts::new(
                format!("{}_tool_exec_total", namespace),
                "Total number of tool executions by tool"
            ),
            &["tool"]
        )?;

        let tool_exec_duration_seconds = register_histogram_vec!(
            HistogramOpts::new(
                format!("{}_tool_exec_duration_seconds", namespace),
                "Tool execution duration in seconds by tool"
            )
            .buckets(LATENCY_BUCKETS.to_vec()),
            &["tool"]
        )?;

        let memory_ops_total = register_counter_vec!(
            Opts::new(
                format!("{}_memory_ops_total", namespace),
                "Total number of memory operations by operation type"
            ),
            &["op"]
        )?;

        let http_requests_total = register_counter_vec!(
            Opts::new(
                format!("{}_http_requests_total", namespace),
                "Total number of HTTP requests by route and method"
            ),
            &["route", "method"]
        )?;

        let http_request_duration_seconds = register_histogram_vec!(
            HistogramOpts::new(
                format!("{}_http_request_duration_seconds", namespace),
                "HTTP request duration in seconds by route and method"
            )
            .buckets(LATENCY_BUCKETS.to_vec()),
            &["route", "method"]
        )?;

        // Security metrics
        let security_auth_attempts_total = register_counter_vec!(
            Opts::new(
                format!("{}_security_auth_attempts_total", namespace),
                "Total authentication attempts by result (success, failure, invalid)"
            ),
            &["result"]
        )?;

        let security_rbac_checks_total = register_counter_vec!(
            Opts::new(
                format!("{}_security_rbac_checks_total", namespace),
                "Total RBAC permission checks by result (allowed, denied)"
            ),
            &["result", "tool"]
        )?;

        let security_policy_violations_total = register_counter_vec!(
            Opts::new(
                format!("{}_security_policy_violations_total", namespace),
                "Total security policy violations by type"
            ),
            &["violation_type"]
        )?;

        let security_resource_limit_exceeded_total = register_counter_vec!(
            Opts::new(
                format!("{}_security_resource_limit_exceeded_total", namespace),
                "Total resource limit violations by resource type"
            ),
            &["resource_type"]
        )?;

        let security_rate_limit_exceeded_total = register_counter_vec!(
            Opts::new(
                format!("{}_security_rate_limit_exceeded_total", namespace),
                "Total rate limit violations by limit type (global, ip, user, endpoint)"
            ),
            &["limit_type"]
        )?;

        // Enhanced HTTP metrics
        let http_requests_by_status = register_counter_vec!(
            Opts::new(
                format!("{}_http_requests_by_status", namespace),
                "Total HTTP requests by route, method, and status code"
            ),
            &["route", "method", "status"]
        )?;

        let http_request_size_bytes = register_histogram_vec!(
            HistogramOpts::new(
                format!("{}_http_request_size_bytes", namespace),
                "HTTP request body size in bytes"
            )
            .buckets(vec![
                100.0,
                1_000.0,
                10_000.0,
                100_000.0,
                1_000_000.0,
                10_000_000.0
            ]),
            &["route", "method"]
        )?;

        let http_response_size_bytes = register_histogram_vec!(
            HistogramOpts::new(
                format!("{}_http_response_size_bytes", namespace),
                "HTTP response body size in bytes"
            )
            .buckets(vec![
                100.0,
                1_000.0,
                10_000.0,
                100_000.0,
                1_000_000.0,
                10_000_000.0
            ]),
            &["route", "method"]
        )?;

        let http_requests_in_flight = register_gauge!(Opts::new(
            format!("{}_http_requests_in_flight", namespace),
            "Number of HTTP requests currently being processed"
        ))?;

        // Per-agent metrics
        let agent_requests_total = register_counter_vec!(
            Opts::new(
                format!("{}_agent_requests_total", namespace),
                "Total requests per agent by operation"
            ),
            &["agent_id", "operation"]
        )?;

        let agent_errors_by_type = register_counter_vec!(
            Opts::new(
                format!("{}_agent_errors_by_type", namespace),
                "Total errors per agent by error type"
            ),
            &["agent_id", "error_type"]
        )?;

        let agent_tool_executions = register_counter_vec!(
            Opts::new(
                format!("{}_agent_tool_executions", namespace),
                "Total tool executions per agent"
            ),
            &["agent_id", "tool"]
        )?;

        Ok(Self {
            agent_sessions_active,
            agent_errors_total,
            tool_exec_total,
            tool_exec_duration_seconds,
            memory_ops_total,
            http_requests_total,
            http_request_duration_seconds,
            http_requests_by_status,
            http_request_size_bytes,
            http_response_size_bytes,
            http_requests_in_flight,
            agent_requests_total,
            agent_errors_by_type,
            agent_tool_executions,
            security_auth_attempts_total,
            security_rbac_checks_total,
            security_policy_violations_total,
            security_resource_limit_exceeded_total,
            security_rate_limit_exceeded_total,
        })
    }
}

/// Metrics registry with cardinality tracking
#[derive(Debug)]
pub struct MetricsRegistry {
    core_metrics: CoreMetrics,
    prometheus_registry: Registry,
    cardinality_tracker: RwLock<CardinalityTracker>,
}

impl MetricsRegistry {
    /// Initialize metrics registry with namespace
    pub fn new(namespace: &str) -> Result<Self, MetricsError> {
        let core_metrics = CoreMetrics::new(namespace)?;
        let prometheus_registry = Registry::new();
        let cardinality_tracker = RwLock::new(CardinalityTracker::new());

        Ok(Self {
            core_metrics,
            prometheus_registry,
            cardinality_tracker,
        })
    }

    /// Get core metrics instance
    pub fn core_metrics(&self) -> &CoreMetrics {
        &self.core_metrics
    }

    /// Get Prometheus registry for metrics export
    pub fn prometheus_registry(&self) -> &Registry {
        &self.prometheus_registry
    }

    /// Record agent session start
    pub fn record_agent_session_start(&self, _tags: &CardinalTags) -> Result<(), MetricsError> {
        self.core_metrics.agent_sessions_active.inc();
        Ok(())
    }

    /// Record agent session end
    pub fn record_agent_session_end(&self, _tags: &CardinalTags) -> Result<(), MetricsError> {
        self.core_metrics.agent_sessions_active.dec();
        Ok(())
    }

    /// Record tool execution
    pub fn record_tool_execution(
        &self,
        tool_name: &ToolName,
        duration: std::time::Duration,
    ) -> Result<(), MetricsError> {
        // Enforce cardinality limit for tools (≤20)
        {
            let mut tracker = self.cardinality_tracker.write().map_err(|_| {
                MetricsError::CardinalityTracking("Failed to acquire write lock".to_string())
            })?;

            if !tracker.tool_names.contains(tool_name) {
                if tracker.tool_names.len() >= 20 {
                    return Err(MetricsError::CardinalityViolation {
                        metric: "tool_exec_*".to_string(),
                        limit: 20,
                        current: tracker.tool_names.len(),
                    });
                }
                tracker.tool_names.insert(tool_name.clone());
            }
        }

        let tool_str = tool_name.as_str();
        self.core_metrics
            .tool_exec_total
            .with_label_values(&[tool_str])
            .inc();
        self.core_metrics
            .tool_exec_duration_seconds
            .with_label_values(&[tool_str])
            .observe(duration.as_secs_f64());

        Ok(())
    }

    /// Record agent error
    pub fn record_agent_error(&self, error_kind: &ErrorKind) -> Result<(), MetricsError> {
        let kind_str = error_kind.as_str();
        self.core_metrics
            .agent_errors_total
            .with_label_values(&[kind_str])
            .inc();
        Ok(())
    }

    /// Record memory operation
    pub fn record_memory_operation(&self, op: &MemoryOp) -> Result<(), MetricsError> {
        let op_str = op.as_str();
        self.core_metrics
            .memory_ops_total
            .with_label_values(&[op_str])
            .inc();
        Ok(())
    }

    /// Record HTTP request
    pub fn record_http_request(
        &self,
        route: &str,
        method: &str,
        duration: std::time::Duration,
    ) -> Result<(), MetricsError> {
        // Enforce cardinality limit for HTTP routes (≤30)
        let route_method = format!("{}:{}", route, method);
        {
            let mut tracker = self.cardinality_tracker.write().map_err(|_| {
                MetricsError::CardinalityTracking("Failed to acquire write lock".to_string())
            })?;

            if !tracker.http_routes.contains(&route_method) {
                if tracker.http_routes.len() >= 30 {
                    return Err(MetricsError::CardinalityViolation {
                        metric: "http_request_*".to_string(),
                        limit: 30,
                        current: tracker.http_routes.len(),
                    });
                }
                tracker.http_routes.insert(route_method);
            }
        }

        self.core_metrics
            .http_requests_total
            .with_label_values(&[route, method])
            .inc();
        self.core_metrics
            .http_request_duration_seconds
            .with_label_values(&[route, method])
            .observe(duration.as_secs_f64());

        Ok(())
    }

    /// Record HTTP request with enhanced metrics (status code, request/response sizes)
    ///
    /// # Errors
    ///
    /// Returns error if cardinality limits are exceeded
    pub fn record_http_request_detailed(
        &self,
        route: &str,
        method: &str,
        status_code: u16,
        duration: std::time::Duration,
        request_size: usize,
        response_size: usize,
    ) -> Result<(), MetricsError> {
        // Record basic metrics
        self.record_http_request(route, method, duration)?;

        // Record status code
        let status = status_code.to_string();
        self.core_metrics
            .http_requests_by_status
            .with_label_values(&[route, method, &status])
            .inc();

        // Record request/response sizes
        self.core_metrics
            .http_request_size_bytes
            .with_label_values(&[route, method])
            .observe(request_size as f64);

        self.core_metrics
            .http_response_size_bytes
            .with_label_values(&[route, method])
            .observe(response_size as f64);

        Ok(())
    }

    /// Increment in-flight requests counter
    pub fn inc_requests_in_flight(&self) {
        self.core_metrics.http_requests_in_flight.inc();
    }

    /// Decrement in-flight requests counter
    pub fn dec_requests_in_flight(&self) {
        self.core_metrics.http_requests_in_flight.dec();
    }

    /// Record per-agent request
    ///
    /// # Errors
    ///
    /// Returns error if recording fails
    pub fn record_agent_request(
        &self,
        agent_id: &str,
        operation: &str,
    ) -> Result<(), MetricsError> {
        self.core_metrics
            .agent_requests_total
            .with_label_values(&[agent_id, operation])
            .inc();
        Ok(())
    }

    /// Record per-agent error
    ///
    /// # Errors
    ///
    /// Returns error if recording fails
    pub fn record_agent_error_by_type(
        &self,
        agent_id: &str,
        error_type: &str,
    ) -> Result<(), MetricsError> {
        self.core_metrics
            .agent_errors_by_type
            .with_label_values(&[agent_id, error_type])
            .inc();
        Ok(())
    }

    /// Record per-agent tool execution
    ///
    /// # Errors
    ///
    /// Returns error if recording fails
    pub fn record_agent_tool_execution(
        &self,
        agent_id: &str,
        tool: &str,
    ) -> Result<(), MetricsError> {
        self.core_metrics
            .agent_tool_executions
            .with_label_values(&[agent_id, tool])
            .inc();
        Ok(())
    }

    /// Get current cardinality statistics
    pub fn cardinality_stats(&self) -> Result<CardinalityStats, MetricsError> {
        let tracker = self.cardinality_tracker.read().map_err(|_| {
            MetricsError::CardinalityTracking("Failed to acquire read lock".to_string())
        })?;

        Ok(CardinalityStats {
            tool_names_count: tracker.tool_names.len(),
            http_routes_count: tracker.http_routes.len(),
            error_kinds_count: 10, // Fixed cardinality from ErrorKind enum
            memory_ops_count: 4,   // Fixed cardinality from MemoryOp enum
        })
    }
}

/// Cardinality tracking to prevent metrics explosion
#[derive(Debug)]
struct CardinalityTracker {
    tool_names: std::collections::HashSet<ToolName>,
    http_routes: std::collections::HashSet<String>,
}

impl CardinalityTracker {
    fn new() -> Self {
        Self {
            tool_names: std::collections::HashSet::new(),
            http_routes: std::collections::HashSet::new(),
        }
    }
}

/// Current cardinality statistics
#[derive(Debug, Clone)]
pub struct CardinalityStats {
    pub tool_names_count: usize,
    pub http_routes_count: usize,
    pub error_kinds_count: usize,
    pub memory_ops_count: usize,
}

/// Metrics collector for easy usage patterns
#[derive(Debug)]
pub struct MetricsCollector {
    registry: Arc<MetricsRegistry>,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new(registry: Arc<MetricsRegistry>) -> Self {
        Self { registry }
    }

    /// Start timing a tool execution
    pub fn start_tool_timer(&self, tool_name: ToolName) -> ToolExecutionTimer {
        ToolExecutionTimer::new(tool_name, self.registry.clone())
    }

    /// Start timing an HTTP request
    pub fn start_http_timer(&self, route: String, method: String) -> HttpRequestTimer {
        HttpRequestTimer::new(route, method, self.registry.clone())
    }

    /// Record an error
    pub fn record_error(&self, error_kind: ErrorKind) -> Result<(), MetricsError> {
        self.registry.record_agent_error(&error_kind)
    }

    /// Record memory operation
    pub fn record_memory_op(&self, op: MemoryOp) -> Result<(), MetricsError> {
        self.registry.record_memory_operation(&op)
    }
}

/// Timer for tool execution measurements
pub struct ToolExecutionTimer {
    tool_name: ToolName,
    start_time: Instant,
    registry: Arc<MetricsRegistry>,
}

impl ToolExecutionTimer {
    fn new(tool_name: ToolName, registry: Arc<MetricsRegistry>) -> Self {
        Self {
            tool_name,
            start_time: Instant::now(),
            registry,
        }
    }

    /// Finish timing and record metric
    pub fn finish(self) -> Result<(), MetricsError> {
        let duration = self.start_time.elapsed();
        self.registry
            .record_tool_execution(&self.tool_name, duration)
    }
}

impl Drop for ToolExecutionTimer {
    fn drop(&mut self) {
        let duration = self.start_time.elapsed();
        let _ = self
            .registry
            .record_tool_execution(&self.tool_name, duration);
    }
}

/// Timer for HTTP request measurements
pub struct HttpRequestTimer {
    route: String,
    method: String,
    start_time: Instant,
    registry: Arc<MetricsRegistry>,
}

impl HttpRequestTimer {
    fn new(route: String, method: String, registry: Arc<MetricsRegistry>) -> Self {
        Self {
            route,
            method,
            start_time: Instant::now(),
            registry,
        }
    }

    /// Finish timing and record metric
    pub fn finish(self) -> Result<(), MetricsError> {
        let duration = self.start_time.elapsed();
        self.registry
            .record_http_request(&self.route, &self.method, duration)
    }
}

impl Drop for HttpRequestTimer {
    fn drop(&mut self) {
        let duration = self.start_time.elapsed();
        let _ = self
            .registry
            .record_http_request(&self.route, &self.method, duration);
    }
}

/// Initialize global metrics registry
pub fn init_metrics_registry(namespace: &str) -> Result<(), MetricsError> {
    let registry = Arc::new(MetricsRegistry::new(namespace)?);
    METRICS_REGISTRY
        .set(registry)
        .map_err(|_| MetricsError::AlreadyInitialized)?;
    Ok(())
}

/// Get global metrics registry
pub fn get_metrics_registry() -> Option<Arc<MetricsRegistry>> {
    METRICS_REGISTRY.get().cloned()
}

/// Create metrics collector from global registry
pub fn create_collector() -> Result<MetricsCollector, MetricsError> {
    let registry = get_metrics_registry().ok_or(MetricsError::NotInitialized)?;
    Ok(MetricsCollector::new(registry))
}

/// Metrics system errors
#[derive(Error, Debug)]
pub enum MetricsError {
    #[error("Prometheus error: {0}")]
    Prometheus(#[from] prometheus::Error),

    #[error("Metrics registry not initialized")]
    NotInitialized,

    #[error("Metrics registry already initialized")]
    AlreadyInitialized,

    #[error("Cardinality violation for metric {metric}: {current} >= {limit}")]
    CardinalityViolation {
        metric: String,
        limit: usize,
        current: usize,
    },

    #[error("Cardinality tracking error: {0}")]
    CardinalityTracking(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::ToolName;

    #[test]
    fn test_metrics_registry_creation() {
        // Use simple alphanumeric namespace for tests
        let id = uuid::Uuid::new_v4().simple().to_string();
        let registry = MetricsRegistry::new(&format!("test{}", &id[0..8])).unwrap();
        assert_eq!(registry.core_metrics().agent_sessions_active.get(), 0.0);
    }

    #[test]
    fn test_tool_execution_recording() {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let registry = Arc::new(MetricsRegistry::new(&format!("test{}", &id[0..8])).unwrap());
        let tool_name = ToolName::new_unchecked("test_tool");
        let duration = std::time::Duration::from_millis(100);

        registry
            .record_tool_execution(&tool_name, duration)
            .unwrap();

        // Verify metrics were recorded (implementation would check actual values)
    }

    #[test]
    fn test_cardinality_enforcement() {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let registry = Arc::new(MetricsRegistry::new(&format!("test{}", &id[0..8])).unwrap());

        // Add 20 tools (at limit)
        for i in 0..20 {
            let tool_name = ToolName::new_unchecked(format!("tool_{}", i));
            registry
                .record_tool_execution(&tool_name, std::time::Duration::from_millis(1))
                .unwrap();
        }

        // 21st tool should fail
        let tool_name = ToolName::new_unchecked("tool_21");
        let result =
            registry.record_tool_execution(&tool_name, std::time::Duration::from_millis(1));
        assert!(matches!(
            result,
            Err(MetricsError::CardinalityViolation { .. })
        ));
    }

    #[test]
    fn test_tool_timer() {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let registry = Arc::new(MetricsRegistry::new(&format!("test{}", &id[0..8])).unwrap());
        let collector = MetricsCollector::new(registry);

        let tool_name = ToolName::new_unchecked("test_tool");
        let timer = collector.start_tool_timer(tool_name);

        // Timer should finish without error
        timer.finish().unwrap();
    }
}
