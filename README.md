# Skreaver

**Skreaver** is a Rust-native coordination runtime for building modular AI agents and agentic infrastructures.

Skreaver aims to be the *Tokio* of agent systems: lightweight, pluggable, and ready for real-world orchestration.

---

## ✨ Highlights

- **🔒 API Stability Guarantee**: Clear versioning policy and deprecation process ([API_STABILITY.md](API_STABILITY.md))
- **🦀 Rust-native** agent architecture with zero-cost abstractions
- **🧩 Decoupled** `Agent` / `Memory` / `Tool` model for composability
- **💾 Multiple memory backends**: File, Redis (clustering), SQLite (WAL), PostgreSQL (ACID)
- **🌐 HTTP runtime** with RESTful API endpoints and OpenAPI documentation
- **🕸️ Multi-agent communication** with Redis Pub/Sub messaging
- **🤖 Coordination patterns**: Supervisor/Worker, Request/Reply, Broadcast/Gather, Pipeline
- **🔌 MCP protocol support** for Claude Desktop integration
- **⚡ WebSocket support** for real-time communication (experimental)
- **🛠️ Standard tool library**: HTTP, File I/O, JSON/XML, Text processing
- **📊 Production observability**: Prometheus metrics, OpenTelemetry tracing, health checks
- **🔒 Enterprise security**: Threat modeling, input validation, SSRF/path traversal protection
- **⚙️ Built-in reliability**: Backpressure monitoring, dead letter queues, automatic retry
- **🚀 Performance**: Comprehensive benchmarking with CI integration
- **🎛️ Feature flags**: Granular control over dependencies and build size

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

📦 **Status**: Skreaver v0.3.0 is production-ready for core use cases.

### ✅ Implemented (v0.3.0)

**Core Framework**:
* `Agent`, `Memory`, and `Tool` trait system
* Type-safe error handling with structured error types
* Non-empty collections (`NonEmptyVec`, `NonEmptyQueue`)
* Modular `Coordinator` runtime with multi-tool execution

**Memory Backends**:
* File-based persistence (`FileMemory`)
* Redis with clustering support (`RedisMemory`)
* SQLite with WAL mode (`SqliteMemory`)
* PostgreSQL with ACID compliance (`PostgresMemory`)
* Connection pooling and health monitoring

**HTTP Runtime** (`skreaver-http`):
* RESTful API endpoints with Axum
* OpenAPI 3.0 documentation generation
* JWT and API key authentication
* WebSocket support (experimental)
* Compression and streaming

**Multi-Agent Communication** (`skreaver-mesh`):
* Redis Pub/Sub messaging
* Coordination patterns: Supervisor, Request/Reply, Broadcast/Gather, Pipeline
* Backpressure monitoring and dead letter queues
* Type-safe message schemas

**MCP Integration** (`skreaver-mcp`):
* MCP server for exposing tools to Claude Desktop
* MCP bridge for using external MCP servers
* Full protocol compliance with type safety

**Standard Tools** (`skreaver-tools`):
* HTTP/network operations
* File system operations
* JSON/XML/text processing
* Tool registry with dispatch

**Observability** (`skreaver-observability`):
* Prometheus metrics with cardinality controls
* OpenTelemetry distributed tracing
* Health checks and monitoring
* Performance targets (p50<30ms, p95<200ms)

**Security** (`skreaver-core/security`):
* Threat modeling and security policies
* Input validation and sanitization
* Path traversal and SSRF protection
* Resource limits and audit logging

**Testing & CI**:
* Comprehensive test suite (554+ test points)
* Property-based testing with proptest
* Golden tests for regression detection
* Automated benchmarking with CI integration
* cargo-semver-checks for API stability

**Deployment**:
* Docker images with multi-stage builds
* Kubernetes Helm charts
* Health checks and HPA support

### 🚧 Roadmap (v0.4.0 - v0.5.0)

* CLI scaffolding tools (`skreaver new agent`, `skreaver generate tool`)
* Auth middleware integration with HTTP endpoints
* Migration framework for schema evolution
* Enhanced developer documentation
* Live examples and playground

See [DEVELOPMENT_PLAN.md](DEVELOPMENT_PLAN.md) and [TODO.md](TODO.md) for detailed roadmap.

---

## 🔒 API Stability Guarantee

Skreaver provides **clear API stability guarantees** starting from v0.3.0:

✅ **Stable APIs**: When you import from the `skreaver` meta-crate, you get backwards-compatible APIs
```rust
use skreaver::{Agent, Memory, Tool};  // ✅ Stable
```

⚠️ **Unstable Features**: Features prefixed with `unstable-` (like WebSockets) may change
```toml
[dependencies]
skreaver = { version = "0.3", features = ["unstable-websocket"] }
```

📚 **Learn More**:
- **[API_STABILITY.md](API_STABILITY.md)** - What's stable, versioning policy, deprecation process
- **[DEPRECATION_POLICY.md](DEPRECATION_POLICY.md)** - How we handle API changes
- **[MIGRATION.md](MIGRATION.md)** - Upgrade guides between versions

**Pre-1.0 Notice**: While in 0.x versions, minor releases (0.x.0) may include breaking changes. All changes are documented with migration guides. Post-1.0, we'll follow strict [Semantic Versioning](https://semver.org/).

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

### 🔌 WebSocket Support (Experimental)

Skreaver includes **experimental WebSocket support** for real-time agent communication:

```toml
[dependencies]
skreaver = { version = "0.3", features = ["unstable-websocket"] }
```

**Features:**
- **Real-time Bidirectional Communication**: Send and receive agent messages over WebSockets
- **Connection Management**: Automatic connection tracking and cleanup
- **Authentication**: Integrate with existing auth systems
- **Protocol**: Text and binary message support

**Example Usage:**
```rust
use skreaver_http::websocket::{WebSocketManager, WebSocketConfig};

// Configure WebSocket manager
let ws_config = WebSocketConfig::default()
    .with_heartbeat_interval(Duration::from_secs(30))
    .with_max_connections(1000);

let ws_manager = WebSocketManager::new(ws_config);

// WebSocket endpoint: ws://localhost:3000/ws/agents/{agent_id}
```

**Client Example:**
```javascript
const ws = new WebSocket('ws://localhost:3000/ws/agents/my-agent');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    console.log('Agent response:', data);
};

ws.send(JSON.stringify({
    type: 'observe',
    input: 'hello world'
}));
```

**⚠️ Experimental Notice**: WebSocket support is marked as `unstable-websocket` and may change in future versions. See [API_STABILITY.md](API_STABILITY.md) for details on unstable features.

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

## 🔌 Model Context Protocol (MCP) Support

Skreaver includes **native MCP integration** for seamless interoperability with Claude Desktop and other MCP-compatible clients:

```bash
cargo run --example mcp_server
```

**Features:**
- **MCP Server**: Expose your Skreaver tools as MCP resources for Claude Desktop
- **MCP Bridge**: Use external MCP servers as Skreaver tools
- **Type-Safe Protocol**: Full implementation of MCP specification with Rust type safety
- **Bidirectional**: Both server and client capabilities

**Example - Expose Tools to Claude Desktop:**
```rust
use skreaver_mcp::McpServer;
use skreaver_tools::InMemoryToolRegistry;

// Create your tool registry
let mut tools = InMemoryToolRegistry::new();
tools.register(HttpGetTool::new());
tools.register(FileReadTool::new());

// Start MCP server (stdio mode for Claude Desktop)
let server = McpServer::new(&tools);
server.serve_stdio().await?;
```

**Example - Use External MCP Servers:**
```rust
use skreaver_mcp::McpBridge;

// Connect to external MCP server
let bridge = McpBridge::connect("http://localhost:3000").await?;

// Use external tools as if they were local
let result = bridge.call_tool("search", query).await?;
```

**Integration with Claude Desktop:**
Add to your `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "skreaver": {
      "command": "cargo",
      "args": ["run", "-p", "skreaver-cli", "--", "mcp"],
      "env": {}
    }
  }
}
```

**Protocol Compliance**: Full MCP specification support including resource listing, tool schemas, and error handling.

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

## 🎛️ Feature Flags

Skreaver uses feature flags for optional dependencies and experimental features. Configure them in your `Cargo.toml`:

### Core Features (Stable)

```toml
[dependencies]
skreaver = { version = "0.3", features = [
    # Memory backends
    "redis",      # Redis memory backend with clustering
    "sqlite",     # SQLite memory backend with WAL mode
    "postgres",   # PostgreSQL memory backend with ACID

    # HTTP runtime features
    "auth",          # Authentication (JWT, API keys, RBAC)
    "openapi",       # OpenAPI 3.0 documentation generation
    "openapi-ui",    # Swagger UI for API documentation
    "compression",   # HTTP compression (gzip, br)
    "streaming",     # Server-sent events and streaming responses

    # Observability
    "metrics",       # Prometheus metrics collection
    "tracing",       # Distributed tracing
    "observability", # Full observability (metrics + tracing)
    "opentelemetry", # OpenTelemetry OTLP export

    # Tools
    "io",       # File system tools
    "network",  # HTTP/network tools
    "data",     # JSON/XML/text processing tools

    # Testing
    "testing",  # Test harness and mock tools
]}
```

### Experimental Features (Unstable)

```toml
[dependencies]
skreaver = { version = "0.3", features = [
    "unstable-websocket",  # WebSocket support (may change)
]}
```

**⚠️ Unstable Features**: Features prefixed with `unstable-` are experimental and may have breaking changes in minor releases. See [API_STABILITY.md](API_STABILITY.md) for details.

### Feature Combinations

**Minimal (Core Only)**:
```toml
skreaver = "0.3"  # Just core agent framework
```

**Full Production Stack**:
```toml
skreaver = { version = "0.3", features = [
    "redis", "postgres",           # Persistent memory
    "auth", "openapi", "openapi-ui",  # HTTP + docs
    "observability", "opentelemetry", # Full monitoring
    "io", "network", "data",       # All standard tools
]}
```

**Development**:
```toml
skreaver = { version = "0.3", features = [
    "testing",                     # Test utilities
    "openapi-ui",                  # API exploration
]}
```

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
