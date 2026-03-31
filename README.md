# Skreaver

**The Rust protocol backbone for AI agents** - High-performance MCP and A2A protocol infrastructure with enterprise-grade security.

Skreaver is a Rust-native protocol bridge connecting the Model Context Protocol (MCP) and Agent-to-Agent (A2A) protocol ecosystems, enabling seamless interoperability between tool-based and task-based agent communication.

---

## Why Skreaver?

The AI agent landscape is converging on two protocols:
- **MCP** (Model Context Protocol) - Tool calls, resources, sampling (97M+ SDK downloads)
- **A2A** (Agent-to-Agent) - Task coordination, agent cards, streaming (150+ organizations)

**No production-grade bridge exists** between these protocols - until now.

| Challenge | Skreaver Solution |
|-----------|-------------------|
| Protocol fragmentation | Bidirectional MCP <-> A2A translation |
| Performance bottlenecks | Sub-millisecond translation, zero GC pauses |
| Enterprise security | RBAC, audit trails, input validation |
| Python ecosystem lock-in | Rust core with PyO3 bindings (roadmap) |
| Edge deployment | <5MB binary, WASM support (roadmap) |

---

## Highlights

- **Protocol Gateway**: Automatic MCP <-> A2A message translation with protocol detection
- **Full MCP Support**: Server SDK for Claude Desktop, client SDK for external servers
- **A2A Implementation**: Agent cards, task coordination, streaming events, artifacts
- **Enterprise Security**: A+ security grade (98/100), SSRF/path traversal protection
- **Production Observability**: Prometheus metrics, OpenTelemetry tracing, health checks
- **Multiple Memory Backends**: File, Redis (clustering), SQLite (WAL), PostgreSQL (ACID)
- **HTTP Runtime**: RESTful API, WebSocket support, OpenAPI documentation
- **1,349+ Tests**: Comprehensive test coverage with property-based testing

---

## Architecture

```
                    +-------------------+
                    |  Protocol Gateway |
                    +-------------------+
                           |   |
              +------------+   +------------+
              |                             |
      +-------v-------+             +-------v-------+
      |      MCP      |             |      A2A      |
      | (Tool Calls)  |             | (Task Coord)  |
      +---------------+             +---------------+
              |                             |
      +-------v-------+             +-------v-------+
      | Claude Desktop|             | Agent Network |
      | MCP Servers   |             | A2A Clients   |
      +---------------+             +---------------+
```

### Crate Structure

```
skreaver/
├── skreaver-gateway      # Protocol bridge (MCP <-> A2A translation)
├── skreaver-mcp          # Model Context Protocol implementation
├── skreaver-a2a          # Agent-to-Agent protocol types & client
├── skreaver-agent        # Unified agent interface
├── skreaver-core         # Core traits, security, types
├── skreaver-http         # Axum runtime, WebSocket, REST API
├── skreaver-memory       # File, Redis, SQLite, PostgreSQL backends
├── skreaver-tools        # Standard tools (HTTP, File I/O, JSON/XML)
├── skreaver-observability # Prometheus, OpenTelemetry, health checks
└── skreaver-testing      # Test utilities, MockTool, TestHarness
```

---

## Quick Start

### Protocol Gateway

```rust
use skreaver_gateway::{ProtocolGateway, Protocol};
use serde_json::json;

let gateway = ProtocolGateway::new();

// Translate MCP tool call to A2A task
let mcp_request = json!({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {"name": "search", "arguments": {"query": "rust"}}
});

let a2a_task = gateway.translate_to(mcp_request, Protocol::A2a)?;
// Result: A2A task with taskId, status, messages
```

### MCP Server (Claude Desktop)

```rust
use skreaver_mcp::McpServer;
use skreaver_tools::InMemoryToolRegistry;

let mut tools = InMemoryToolRegistry::new();
tools.register(HttpGetTool::new());
tools.register(FileReadTool::new());

let server = McpServer::new(&tools);
server.serve_stdio().await?;
```

Add to `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "skreaver": {
      "command": "cargo",
      "args": ["run", "-p", "skreaver-cli", "--", "mcp"]
    }
  }
}
```

### A2A Server

```rust
use skreaver_a2a::{A2aServer, AgentCard, TaskHandler};

let agent_card = AgentCard::builder()
    .name("calculator-agent")
    .description("Performs calculations")
    .build();

let server = A2aServer::new(agent_card, handler);
server.serve("0.0.0.0:3001").await?;
```

---

## v0.6.0 Release Highlights

**Protocol Gateway** (NEW)
- Bidirectional MCP <-> A2A translation
- Automatic protocol detection from message format
- Connection registry with lifecycle management
- Sub-millisecond translation overhead

**A2A Protocol** (NEW)
- Full A2A v0.3 spec implementation
- Agent card discovery and registration
- Task coordination (create, status, cancel)
- Streaming events via SSE
- HTTP transport with REST endpoints

**MCP Enhancements**
- MCP spec 2025-11-25 compliance (tasks, sampling)
- Full server SDK for Claude Desktop
- Client SDK for external MCP servers

**Test Coverage**
- 1,349+ tests (237% of initial target)
- 14 gateway integration tests
- Property-based testing with proptest

---

## Examples

```bash
# Protocol bridge demo
cargo run --example protocol_bridge_demo

# A2A server/client
cargo run --example a2a_server
cargo run --example a2a_client

# HTTP runtime
cargo run --example http_server

# Benchmarks
cargo bench --bench quick_benchmark
```

---

## Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Protocol translation | <1ms | MCP <-> A2A |
| Memory store | ~350ns | In-memory |
| JSON processing | ~3.5us | Simple objects |
| Tool execution | p50<30ms | With validation |

**8-500x faster than Python agent frameworks** (see benchmarks)

---

## Feature Flags

```toml
[dependencies]
skreaver = { version = "0.6", features = [
    # Memory backends
    "redis", "sqlite", "postgres",

    # HTTP features
    "auth", "openapi", "websocket",

    # Observability
    "metrics", "tracing", "opentelemetry",

    # Tools
    "io", "network", "data",
]}
```

---

## Roadmap

### v0.7.0 - Python Accessibility
- PyO3 bindings for core protocol types
- `pip install skreaver` package
- LangChain/CrewAI integration examples

### v0.8.0 - Edge Runtime
- WASM compilation target
- <5MB binary size
- Sub-10ms cold starts
- Cloudflare Workers support

### v0.9.0 - Enterprise Features
- Guardrails policy engine
- AI-BOM generation
- Kubernetes operator

See [ROADMAP.md](.dev/ROADMAP.md) for detailed planning.

---

## API Stability

Skreaver provides **clear API stability guarantees**:

```rust
use skreaver::{Agent, Memory, Tool};  // Stable APIs
```

- **Stable**: Core traits, protocol types, memory backends
- **Unstable**: Features prefixed with `unstable-`

See [API_STABILITY.md](API_STABILITY.md) for versioning policy.

---

## Contributing

- Star the repo
- Report issues on GitHub
- Join discussions
- [Sponsor development](https://github.com/sponsors/shurankain)

---

## Links

- [Documentation](https://docs.rs/skreaver)
- [GitHub](https://github.com/shurankain/skreaver)
- [Skreaver.com](https://skreaver.com)
- [Author](https://ohusiev.com)

---

## License

MIT - see [LICENSE](./LICENSE)

---

**Support Skreaver**: [GitHub Sponsors](https://github.com/sponsors/shurankain) | [PayPal](https://www.paypal.com/paypalme/olhusiev)
