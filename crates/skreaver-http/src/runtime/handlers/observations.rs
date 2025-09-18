//! Agent observation HTTP handlers
//!
//! This module provides endpoints for sending observations to agents,
//! including streaming and batch operations.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, sse::Sse},
};
use futures::Stream;
use skreaver_observability::{AgentId as ObsAgentId, SessionId, metrics::get_metrics_registry};
use skreaver_tools::ToolRegistry;

use crate::runtime::{
    HttpAgentRuntime,
    backpressure::RequestPriority,
    streaming::{self, StreamingAgentExecutor},
    types::{
        BatchObserveRequest, BatchObserveResponse, BatchResult, ErrorResponse, ObserveRequest,
        ObserveResponse, StreamRequest,
    },
};
use std::sync::Arc;

/// GET /agents/{agent_id}/stream - Stream agent execution in real-time
#[utoipa::path(
    get,
    path = "/agents/{agent_id}/stream",
    params(
        ("agent_id" = String, Path, description = "Agent identifier"),
        ("input" = Option<String>, Query, description = "Optional input to send to agent"),
        ("debug" = Option<bool>, Query, description = "Include debug information in stream"),
        ("timeout_seconds" = Option<u64>, Query, description = "Custom timeout in seconds")
    ),
    responses(
        (status = 200, description = "Server-Sent Events stream of agent updates"),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn stream_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Query(params): Query<StreamRequest>,
) -> Result<
    Sse<impl Stream<Item = Result<axum::response::sse::Event, axum::BoxError>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    // Verify agent exists
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    // Create streaming executor
    let (executor, receiver) = StreamingAgentExecutor::new();

    // Start background task to execute agent with input if provided
    if let Some(input) = params.input {
        let runtime_clone = runtime.clone();
        let agent_id_clone = agent_id.clone();
        let debug = params.debug;
        let timeout = params.timeout_seconds.unwrap_or(300); // Default 5 minutes

        tokio::spawn(async move {
            let agent_id_for_timeout = agent_id_clone.clone();

            // Apply timeout to the entire operation
            let execution_result =
                tokio::time::timeout(std::time::Duration::from_secs(timeout), async {
                    executor
                        .execute_with_streaming(agent_id_clone.clone(), |exec| async move {
                            if debug {
                                exec.thinking(&agent_id_clone, "Starting agent processing")
                                    .await;
                                exec.partial(
                                    &agent_id_clone,
                                    &format!(
                                        "Debug: Processing input of {} characters",
                                        input.len()
                                    ),
                                )
                                .await;
                            }

                            exec.thinking(&agent_id_clone, "Processing observation")
                                .await;

                            // Access the agent instance here
                            let response = {
                                let mut agents = runtime_clone.agents.write().await;
                                if let Some(instance) = agents.get_mut(&agent_id_clone) {
                                    let response = instance.coordinator.step(input);
                                    drop(agents); // Release lock immediately
                                    Ok(response)
                                } else {
                                    Err("Agent not found".to_string())
                                }
                            }?;

                            if debug {
                                exec.partial(
                                    &agent_id_clone,
                                    &format!(
                                        "Debug: Generated response of {} characters",
                                        response.len()
                                    ),
                                )
                                .await;
                            }

                            exec.partial(&agent_id_clone, &response).await;
                            Ok(response)
                        })
                        .await
                })
                .await;

            // Handle timeout
            if execution_result.is_err() {
                let _ = executor
                    .send_update(streaming::AgentUpdate::Error {
                        agent_id: agent_id_for_timeout,
                        error: format!("Operation timed out after {} seconds", timeout),
                        timestamp: chrono::Utc::now(),
                    })
                    .await;
            }
        });
    } else {
        // Send a status ping for connection health
        let agent_id_clone = agent_id.clone();
        tokio::spawn(async move {
            let _ = executor
                .send_update(streaming::AgentUpdate::Ping {
                    timestamp: chrono::Utc::now(),
                })
                .await;

            // Send initial status for this agent
            let _ = executor
                .send_update(streaming::AgentUpdate::Started {
                    agent_id: agent_id_clone,
                    timestamp: chrono::Utc::now(),
                })
                .await;
        });
    }

    // Return SSE stream
    Ok(streaming::create_sse_stream(receiver))
}

/// POST /agents/{agent_id}/observe - Send observation to agent
#[utoipa::path(
    post,
    path = "/agents/{agent_id}/observe",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    request_body = ObserveRequest,
    responses(
        (status = 200, description = "Agent response to observation", body = ObserveResponse),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn observe_agent<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Json(request): Json<ObserveRequest>,
) -> Result<Json<ObserveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    // Record HTTP request metrics
    if let Some(registry) = get_metrics_registry() {
        let route = format!("/agents/{}/observe", "{agent_id}");
        let _ = registry.record_http_request(&route, "POST", start_time.elapsed());
    }

    // Check if agent exists first
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    // Use backpressure manager for request processing
    let priority = RequestPriority::Normal; // TODO: Allow setting priority in request
    let timeout = Some(std::time::Duration::from_secs(30)); // TODO: Make configurable

    let (_request_id, rx) = runtime
        .backpressure_manager
        .queue_request_with_input(agent_id.clone(), request.input.clone(), priority, timeout)
        .await
        .map_err(|e| {
            let status = match e {
                crate::runtime::backpressure::BackpressureError::QueueFull { .. } => {
                    StatusCode::TOO_MANY_REQUESTS
                }
                crate::runtime::backpressure::BackpressureError::SystemOverloaded { .. } => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(ErrorResponse {
                    error: "backpressure_error".to_string(),
                    message: e.to_string(),
                    details: None,
                }),
            )
        })?;

    // Start processing the queued request
    let runtime_clone = runtime.clone();
    let agent_id_clone = agent_id.clone();
    tokio::spawn(async move {
        let agent_id_for_processing = agent_id_clone.clone();
        let runtime_for_closure = runtime_clone.clone();
        if runtime_clone
            .backpressure_manager
            .process_next_queued_request(&agent_id_clone, move |input| {
                let runtime_inner = runtime_for_closure.clone();
                let agent_id_for_closure = agent_id_for_processing.clone();
                async move {
                    // Process the request within backpressure constraints
                    let mut agents = runtime_inner.agents.write().await;
                    if let Some(instance) = agents.get_mut(&agent_id_for_closure) {
                        // Create agent session for observability
                        let session_id = SessionId::generate();

                        // Record agent session start
                        if let Some(registry) = get_metrics_registry() {
                            let obs_agent_id = ObsAgentId::new(agent_id_for_closure.clone())
                                .unwrap_or_else(|_| ObsAgentId::new("invalid-agent").unwrap());
                            let tags = skreaver_observability::CardinalTags::for_agent_session(
                                obs_agent_id.clone(),
                                session_id.clone(),
                            );
                            let _ = registry.record_agent_session_start(&tags);
                        }

                        let response = instance.coordinator.step(input);

                        // Record agent session end
                        if let Some(registry) = get_metrics_registry() {
                            let obs_agent_id = ObsAgentId::new(agent_id_for_closure.clone())
                                .unwrap_or_else(|_| ObsAgentId::new("invalid-agent").unwrap());
                            let tags = skreaver_observability::CardinalTags::for_agent_session(
                                obs_agent_id,
                                session_id,
                            );
                            let _ = registry.record_agent_session_end(&tags);
                        }

                        response
                    } else {
                        "Agent not found".to_string()
                    }
                }
            })
            .await
            .is_some()
        {
            // Processing started successfully
        }
    });

    // Wait for the response
    match rx.await {
        Ok(result) => match result {
            Ok(response) => Ok(Json(ObserveResponse {
                agent_id: agent_id.clone(),
                response,
                timestamp: chrono::Utc::now(),
            })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "processing_failed".to_string(),
                    message: e.to_string(),
                    details: None,
                }),
            )),
        },
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "processing_timeout".to_string(),
                message: "Request processing timed out".to_string(),
                details: None,
            }),
        )),
    }
}

/// POST /agents/{agent_id}/observe/stream - Stream agent observation in real-time
#[utoipa::path(
    post,
    path = "/agents/{agent_id}/observe/stream",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    request_body = ObserveRequest,
    responses(
        (status = 200, description = "Server-Sent Events stream of agent processing"),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn observe_agent_stream<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Json(request): Json<ObserveRequest>,
) -> Result<
    Sse<impl Stream<Item = Result<axum::response::sse::Event, axum::BoxError>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    // Verify agent exists
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    // Create streaming executor
    let (executor, receiver) = streaming::StreamingAgentExecutor::new();

    // Start background task to process observation
    let runtime_clone = runtime.clone();
    let agent_id_clone = agent_id.clone();
    let input = request.input;

    tokio::spawn(async move {
        let mut agents = runtime_clone.agents.write().await;
        if let Some(instance) = agents.get_mut(&agent_id_clone) {
            let _result = executor
                .execute_with_streaming(agent_id_clone.clone(), |exec| async move {
                    exec.thinking(&agent_id_clone, "Analyzing input").await;
                    let response = instance.coordinator.step(input);
                    exec.partial(&agent_id_clone, &response).await;
                    Ok(response)
                })
                .await;
        }
    });

    // Return SSE stream
    Ok(streaming::create_sse_stream(receiver))
}

/// POST /agents/{agent_id}/batch - Process multiple observations in batch
#[utoipa::path(
    post,
    path = "/agents/{agent_id}/batch",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    request_body = BatchObserveRequest,
    responses(
        (status = 200, description = "Batch processing results", body = BatchObserveResponse),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn batch_observe_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Json(request): Json<BatchObserveRequest>,
) -> Result<Json<BatchObserveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();

    // Verify agent exists
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&agent_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    // Validate batch size
    if request.inputs.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "empty_batch".to_string(),
                message: "Batch request must contain at least one input".to_string(),
                details: None,
            }),
        ));
    }

    if request.inputs.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "batch_too_large".to_string(),
                message: "Batch size cannot exceed 100 inputs".to_string(),
                details: None,
            }),
        ));
    }

    // Process inputs with semaphore for concurrency control
    let semaphore = Arc::new(tokio::sync::Semaphore::new(request.parallel_limit));
    let results = Arc::new(tokio::sync::Mutex::new(vec![
        BatchResult {
            index: 0,
            input: String::new(),
            response: String::new(),
            processing_time_ms: 0,
            success: false,
            error: None,
        };
        request.inputs.len()
    ]));

    let mut handles = Vec::new();

    for (index, input) in request.inputs.into_iter().enumerate() {
        let permit = semaphore.clone().acquire_owned().await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "semaphore_error".to_string(),
                    message: "Failed to acquire processing permit".to_string(),
                    details: None,
                }),
            )
        })?;
        let runtime_clone = runtime.clone();
        let agent_id_clone = agent_id.clone();
        let results_clone = Arc::clone(&results);
        let timeout_duration = std::time::Duration::from_secs(request.timeout_seconds);
        let input_clone = input.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit; // Hold the permit for the duration of this task
            let op_start = std::time::Instant::now();

            let result = tokio::time::timeout(timeout_duration, async {
                // Minimize lock scope within timeout
                let mut agents = runtime_clone.agents.write().await;
                if let Some(instance) = agents.get_mut(&agent_id_clone) {
                    let response = instance.coordinator.step(input);
                    drop(agents); // Release lock immediately after step
                    Ok(response)
                } else {
                    drop(agents); // Release lock even when agent not found
                    Err("Agent not found".to_string())
                }
            })
            .await;

            let batch_result = match result {
                Ok(Ok(response)) => BatchResult {
                    index,
                    input: input_clone,
                    response,
                    processing_time_ms: op_start.elapsed().as_millis() as u64,
                    success: true,
                    error: None,
                },
                Ok(Err(error)) => BatchResult {
                    index,
                    input: input_clone.clone(),
                    response: String::new(),
                    processing_time_ms: op_start.elapsed().as_millis() as u64,
                    success: false,
                    error: Some(error),
                },
                Err(_) => BatchResult {
                    index,
                    input: input_clone.clone(),
                    response: String::new(),
                    processing_time_ms: timeout_duration.as_millis() as u64,
                    success: false,
                    error: Some("Operation timed out".to_string()),
                },
            };

            let mut results_guard = results_clone.lock().await;
            results_guard[index] = batch_result;
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let _ = handle.await;
    }

    let results = Arc::try_unwrap(results)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal_error".to_string(),
                    message: "Failed to collect batch results".to_string(),
                    details: None,
                }),
            )
        })?
        .into_inner();
    let total_time = start_time.elapsed().as_millis() as u64;

    Ok(Json(BatchObserveResponse {
        agent_id,
        results,
        total_time_ms: total_time,
        timestamp: chrono::Utc::now(),
    }))
}
