# Skreaver

**Skreaver** is a Rust-native coordination runtime for building modular AI agents and agentic infrastructures.

Skreaver aims to be the *Tokio* of agent systems: lightweight, pluggable, and ready for real-world orchestration.

---

## ✨ Highlights

- Rust-native agent architecture
- Decoupled `Agent` / `Memory` / `Tool` model
- Multi-tool execution with result aggregation
- Built-in tool registry system
- Multiple memory backends (File, Redis, SQLite, PostgreSQL)
- **HTTP runtime with RESTful API endpoints**
- **Multi-agent communication with Redis Pub/Sub messaging**
- **Coordination patterns (Supervisor/Worker, Request/Reply, Broadcast/Gather, Pipeline)**
- **Standard tool library (HTTP, File, JSON, Text processing)**
- **Production-grade benchmarking framework with CI integration**
- **Comprehensive observability framework (metrics, tracing, health checks)**
- **Enterprise-grade security framework with threat modeling and audit logging**
- **Built-in backpressure and dead letter queue support**
- Designed for performance and modularity

---

## 🧠 Why Skreaver?

Modern AI agents suffer from:

- Complex stacks (Python + LangChain + glue code)
- Implicit architectures and fragile wrappers
- Poor performance in constrained or embedded environments

**Skreaver** solves this with a strict, high-performance, type-safe platform built in Rust, designed for real-world agent deployment.

---

## ⚙️ Core Principles

- **Rust 2024-first**: zero-cost abstractions, full control  
- **Agent-centric**: traits and modules for memory, tools, goals  
- **Composable runtime**: run agents locally or integrate with infra  
- **Open by design**: build your own memory/tool systems, no lock-in  

---

## 📐 Architecture Preview

```text
[Agent] → [ToolCall] → [ExecutionResult]
   ↓             ↑
[Memory] ← [ContextUpdate]
   ↓
[Coordinator Runtime]
   ↓
[Agent Mesh] ← [Redis Pub/Sub] → [Other Agents]
````

Skreaver gives you the scaffolding. You build the logic.

---

📦 Status: Skreaver is in active development.

Core components implemented:

* `Agent`, `Memory`, and `Tool` traits
* Modular `Coordinator` runtime
* `ToolRegistry` with dispatch and test coverage
* Support for multiple tool calls per step
* Multiple memory backends (`FileMemory`, `RedisMemory`, SQLite, PostgreSQL)
* **Axum-based HTTP runtime with RESTful endpoints**
* **Multi-agent communication layer (`skreaver-mesh`) with Redis Pub/Sub backend**
* **Coordination patterns: Supervisor/Worker, Request/Reply, Broadcast/Gather, Pipeline**
* **Agent messaging with backpressure monitoring and dead letter queues**
* **Standard tool library (HTTP client, file ops, JSON/XML, text processing)**
* **Production benchmarking framework with resource monitoring and CI integration**
* **Comprehensive observability framework with Prometheus metrics, distributed tracing, and health checks**
* **Enterprise security framework with threat modeling, input validation, and audit logging**
* Fully working examples (`echo`, `multi_tool`, `http_server`, `standard_tools`, `mesh_ping_pong`, `mesh_broadcast`, `mesh_task_coordinator`)
* Self-hosted CI pipeline

Next steps:

* Agent test harness and mock tools (✅ `skreaver-testing` implemented)
* OpenTelemetry integration for distributed tracing (✅ Phase 0.3 implemented)
* Enterprise security framework (✅ Phase 0.4 implemented)
* Authentication and rate limiting (🚧 In progress - Phase 1.1)
* Enhanced memory backends (SQLite, PostgreSQL) (🚧 Planned - Phase 1.1)
* Playground & live examples  
* Developer docs (powered by skreaver-docs-starter)

---

## 🖥️ `skreaver-cli`

A dedicated command-line interface for running agents directly from terminal.

Examples:

```bash
cargo run -p skreaver-cli -- --name echo
cargo run -p skreaver-cli -- --name multi
```

* Supports interactive agent execution
* Uses `FileMemory` for persistent state
* Includes tool dispatch (e.g., `uppercase`, `reverse`)
* Echo and MultiTool agents available out of the box

> Add your own agents in `skreaver-cli/src/agents/`, plug into the CLI via `match`, and extend.

---

## 🌐 HTTP Runtime

Skreaver now includes a production-ready HTTP server for remote agent interaction:

```bash
cargo run --example http_server
```

**Available endpoints:**
- `GET /health` - Health check
- `GET /ready` - Kubernetes readiness check with component health
- `GET /metrics` - Prometheus metrics endpoint
- `GET /agents` - List all agents  
- `GET /agents/{id}/status` - Get agent status
- `POST /agents/{id}/observe` - Send observation to agent
- `DELETE /agents/{id}` - Remove agent

**Example requests:**
```bash
# Health checks
curl http://localhost:3000/health
curl http://localhost:3000/ready

# Metrics (Prometheus format)
curl http://localhost:3000/metrics

# List agents
curl http://localhost:3000/agents

# Send observation
curl -X POST http://localhost:3000/agents/demo-agent-1/observe \
     -H 'Content-Type: application/json' \
     -d '{"input": "uppercase:hello world"}'
```

The HTTP runtime supports all standard tools and provides full agent lifecycle management.

---

## 🕸️ Multi-Agent Communication

Skreaver includes a production-ready multi-agent messaging layer for coordinating distributed agent systems:

```bash
cargo run --example mesh_ping_pong
cargo run --example mesh_broadcast
cargo run --example mesh_task_coordinator
```

**Features:**
- **Redis Pub/Sub Backend**: Reliable agent-to-agent messaging with connection pooling
- **Typed Messages**: Strongly-typed schemas (Text/JSON/Binary) with automatic serialization
- **Coordination Patterns**: Request/Reply (RPC-style), Supervisor/Worker (task distribution), Broadcast/Gather (scatter-gather), Pipeline (sequential processing)
- **Reliability**: Dead letter queues with TTL, automatic retry, and backpressure monitoring
- **Observability**: Cardinality-safe metrics (≤20 topics) and distributed tracing

**Example Usage:**
```rust
use skreaver_mesh::{AgentMesh, RedisMesh, Message, AgentId};

// Connect to mesh
let mesh = RedisMesh::new("redis://localhost:6379").await?;

// Point-to-point messaging
mesh.send(&AgentId::from("worker-1"), Message::new("task data")).await?;

// Broadcast to all agents
mesh.broadcast(Message::new("shutdown")).await?;

// Supervisor pattern with load balancing
let supervisor = Supervisor::new(mesh.clone(), config);
supervisor.submit_task(message).await;
```

**Performance:**
- 45 tests covering all patterns and reliability features
- Redis integration tested with connection pooling
- Backpressure with 3-level signals (Normal/Warning/Critical)

---

## 📊 Observability Framework

Skreaver includes a production-ready observability framework for monitoring agent infrastructure:

**Features:**
- **Prometheus Metrics**: Core metrics with strict cardinality controls (≤20 tools, ≤30 HTTP routes)
- **Distributed Tracing**: Session correlation and tool execution tracking
- **Health Checks**: Component monitoring with degradation detection
- **OpenTelemetry**: OTLP endpoint support for external monitoring systems

**Metrics Collected:**
```bash
# Agent metrics
agent_sessions_active
agent_errors_total{kind}

# Tool execution metrics  
tool_exec_total{tool}
tool_exec_duration_seconds{tool}

# Memory operation metrics
memory_ops_total{op}

# HTTP runtime metrics
http_requests_total{route,method}
http_request_duration_seconds{route,method}
```

**Usage:**
```rust
use skreaver_observability::{init_observability, ObservabilityConfig, MetricsCollector};

// Initialize observability
let config = ObservabilityConfig::default();
init_observability(config)?;

// Collect metrics
let collector = MetricsCollector::new(registry);
let _timer = collector.start_tool_timer(tool_name);
```

**Performance Targets:**
- p50 < 30ms, p95 < 200ms, p99 < 400ms for tool execution
- Cardinality limits prevent metrics explosion
- Production-grade sampling for log volume control

---

## 🔒 Security Framework

Skreaver includes an enterprise-grade security framework designed for production agent deployments:

**Features:**
- **Threat Model**: Comprehensive threat analysis with documented attack scenarios and mitigations
- **Input Validation**: Automatic detection of secrets, suspicious patterns, and malicious content
- **Path Traversal Protection**: Directory allowlists and canonicalization to prevent file system attacks
- **SSRF Prevention**: Domain filtering and private network blocking for HTTP tools
- **Resource Limits**: Memory, CPU, and concurrency controls with enforcement
- **Audit Logging**: Structured security event logging with secret redaction

**Security Policies:**
```rust
use skreaver_core::security::{SecurityConfig, SecurityManager};

// Load security configuration
let config = SecurityConfig::load_from_file("security.toml")?;
let security_manager = SecurityManager::new(config);

// All tools can be wrapped with security enforcement
let secure_tool = security_manager.secure_tool(my_tool);
```

**Protection Against:**
- Path traversal attacks (`../../../etc/passwd`)
- SSRF attacks to internal networks (localhost, 169.254.169.254)
- Resource exhaustion (memory bombs, CPU loops)
- Secret leakage in inputs and outputs
- Command injection attempts

**Configuration Example:**
```toml
[fs]
allow_paths = ["/var/app/data"]
deny_patterns = ["..", "/etc", "*.key"]

[http]
allow_domains = ["api.example.com"]
deny_domains = ["localhost", "169.254.169.254"]

[resources]
max_memory_mb = 128
max_concurrent_operations = 10
```

**Security Testing**: 48 comprehensive tests covering threat scenarios, policy enforcement, and audit logging.

---

## ⚡ Performance Benchmarks

Skreaver includes a comprehensive performance benchmarking framework designed for production environments:

```bash
# Quick benchmarks (completes in ~30 seconds)
cargo bench --bench quick_benchmark

# Production benchmark with resource monitoring
cargo bench --bench production_benchmark

# Memory and CPU profiling benchmarks
cargo bench --bench memory_operations

# Realistic agent workload benchmarks
cargo bench --bench realistic_benchmarks
```

**Production Benchmark Framework Features:**
- **Resource Monitoring**: Real-time memory and CPU tracking during benchmarks
- **Performance Thresholds**: Automated pass/fail based on latency targets (p50<30ms, p95<200ms, p99<400ms)
- **Baseline Management**: Historical performance tracking with regression detection
- **Multi-format Reports**: JSON, Markdown, GitHub Actions, and JUnit XML outputs
- **CI Integration**: Automated performance gates in GitHub Actions workflow

**Performance Highlights:**
- Memory operations: ~350ns store, ~130ns load
- File I/O: ~17μs read (1KB), ~111μs write (1KB) 
- JSON processing: ~3.5μs simple, ~5.5μs complex
- **8-500x faster than Python agent frameworks** (see `PERFORMANCE_COMPARISON.md`)

---

▶️ Try it now:

```bash
# Local examples
cargo run --example echo
cargo run --example multi_tool
cargo run --example standard_tools

# HTTP server (RESTful agent runtime)
cargo run --example http_server
# Then test with: curl http://localhost:3000/health

# Multi-agent communication
cargo run --example mesh_ping_pong
cargo run --example mesh_task_coordinator

# Performance benchmarks
cargo bench --bench quick_benchmark
cargo bench --bench production_benchmark
```

---

## 🤝 Contribute / Follow

* ⭐ Star the repo
* 👀 Watch for progress
* 💬 Feedback via GitHub Discussions
* 💸 Support via [GitHub Sponsors](https://github.com/sponsors/shurankain)

---

## 🔗 Links

* [ohusiev.com](https://ohusiev.com)
* [Medium](https://medium.com/@ohusiev_6834)
* [Skreaver.com](https://skreaver.com)

---

## 📄 License

MIT — see [LICENSE](./LICENSE)

## ☕ Support Skreaver

Skreaver is an open-source Rust-native agentic infrastructure platform.
If you believe in the mission, consider supporting its development:

* 💛💙 [Sponsor via GitHub](https://github.com/sponsors/shurankain)
  → [View all sponsor tiers](./sponsorship/SPONSORS.md)
  → [Hall of Sponsors](./sponsorship/hall-of-sponsors.md)

* 💸 [Donate via PayPal](https://www.paypal.com/paypalme/olhusiev)

> Every contribution helps keep the core open and evolving.
