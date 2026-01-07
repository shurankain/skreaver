# Skreaver Development Plan v4.0

> **Status**: Protocol Infrastructure Pivot
> **Updated**: January 2026
> **Current Version**: v0.5.0
> **Next Milestone**: v0.6.0 (Protocol Core)

---

## Vision

Skreaver becomes the high-performance Rust protocol bridge for AI agents, implementing MCP and A2A protocols with production-grade reliability.

---

## Current State Assessment

### What We Have (Verified)

| Component | LOC | Status | Reusable for New Work |
|-----------|-----|--------|----------------------|
| skreaver-core | ~8K | Stable | Security, types, validation |
| skreaver-mcp | 820 | 54% complete | Server works, bridge stubbed |
| skreaver-mesh | 5,444 | Production | Message types, backpressure, DLQ |
| skreaver-http | ~12K | Production | Router, WebSocket, auth middleware |
| skreaver-memory | ~6K | Stable | All backends working |
| skreaver-observability | ~2K | Stable | Metrics, tracing |

### MCP Current State (skreaver-mcp)

**Implemented (85%)**:
- `McpServer` - Exposes Skreaver tools to Claude Desktop
- `ToolAdapter` - Converts Skreaver tools to MCP format
- Stdio transport (standard for Claude Desktop)
- Tool name validation (security)
- 7 unit tests passing

**Not Implemented**:
- `McpBridge` - Only skeleton exists (~5% done)
- MCP client to consume external servers
- Resources, Prompts, Sampling (MCP 2025 spec)
- Async streaming responses

**Dependency**: `rmcp = "0.2.0"` - May need upgrade for 2025 spec features

### Mesh Components Reusable for A2A

| Component | File | Lines | Reuse Value |
|-----------|------|-------|-------------|
| Message types | message/*.rs | 715 | HIGH - adapt for A2A |
| TypedMessage<R> | message/typed.rs | 295 | HIGH - typestate pattern |
| Backpressure | backpressure.rs | 392 | HIGH - throttling |
| Dead Letter Queue | dlq.rs | 434 | MEDIUM - retry logic |
| Metrics | metrics.rs | 285 | HIGH - observability |
| Request/Reply | patterns/request_reply.rs | 179 | HIGH - A2A RPC |

### HTTP Runtime for Gateway

**Reusable**:
- Axum router structure (`runtime/router.rs`)
- Authentication middleware (`runtime/auth.rs`)
- WebSocket manager (`websocket/manager.rs` - 1,514 lines)
- SSE streaming (`handlers/observations.rs`)
- Error handling patterns

**New Code Needed**: ~600-850 lines for gateway handlers

---

## Phase 1: Protocol Core (v0.6.0)

**Duration**: 6-8 weeks realistic
**Goal**: Working MCP client + A2A basic implementation

### 1.1 MCP Bridge Implementation (Week 1-2)

**Current State**: `bridge.rs` has 268 lines but only skeleton code

**Tasks**:
- [ ] Implement `McpBridge::connect_stdio()` - spawn MCP server process
- [ ] Implement `BridgedTool::call()` - route calls to external MCP server
- [ ] Add tool discovery from connected servers
- [ ] Test with filesystem MCP server

**Files to Modify**:
```
crates/skreaver-mcp/src/bridge.rs    # Main implementation
crates/skreaver-mcp/src/error.rs     # Add client error types
crates/skreaver-mcp/Cargo.toml       # May need rmcp client feature
```

**Dependencies to Check**:
```toml
# Current
rmcp = { version = "0.2.0", features = ["server", "transport-io"] }
# May need
rmcp = { version = "0.2.0", features = ["server", "client", "transport-io"] }
```

**Effort**: ~400-500 lines new code
**Risk**: rmcp client API availability - need to verify

### 1.2 A2A Protocol Types (Week 2-3)

**A2A Core Concepts** (from spec):
- Agent Card - JSON describing agent capabilities
- Task - Long-running collaboration unit
- Message - JSON-RPC 2.0 over HTTP(S)
- Transport - HTTP, SSE, Push notifications

**Create New Crate**:
```
crates/skreaver-a2a/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── agent_card.rs    # Agent discovery schema
│   ├── task.rs          # Task management types
│   ├── message.rs       # JSON-RPC 2.0 messages
│   ├── transport.rs     # HTTP + SSE handlers
│   └── error.rs
```

**Agent Card Schema** (from A2A spec):
```rust
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,                    // Agent endpoint
    pub capabilities: Vec<Capability>,
    pub authentication: Option<AuthInfo>,
    pub version: String,
}

pub struct Capability {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
}
```

**Reuse from skreaver-mesh**:
- `Message` structure → adapt to JSON-RPC 2.0
- `Route` enum → simplify to A2A routing
- `Backpressure` → use directly
- `Metrics` → extend for A2A

**Effort**: ~800-1000 lines new code

### 1.3 A2A HTTP Transport (Week 3-4)

**Endpoints to Implement**:
```
POST /a2a/agents                    # Register agent
GET  /a2a/agents                    # List agents
GET  /a2a/agents/{id}/card          # Get agent card
POST /a2a/tasks                     # Create task
GET  /a2a/tasks/{id}                # Get task status
POST /a2a/tasks/{id}/messages       # Send message to task
GET  /a2a/tasks/{id}/stream         # SSE for task updates
```

**Reuse from skreaver-http**:
- Router pattern from `runtime/router.rs`
- Auth middleware from `runtime/auth.rs`
- SSE streaming from `handlers/observations.rs`
- WebSocket for bidirectional (optional)

**Files to Create**:
```
crates/skreaver-http/src/runtime/handlers/a2a.rs  # ~300 lines
crates/skreaver-http/src/runtime/router.rs        # Add A2A routes
```

**Effort**: ~400-500 lines new code

### 1.4 Protocol Gateway (Week 5-6)

**Goal**: Bridge MCP tools ↔ A2A agents

**Gateway Logic**:
```
MCP Tool Call → Gateway → A2A Task Message
A2A Task Result → Gateway → MCP Tool Response
```

**Create New Crate** (or module in skreaver-http):
```
crates/skreaver-gateway/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── bridge.rs        # MCP ↔ A2A translation
│   ├── router.rs        # Protocol detection & routing
│   └── registry.rs      # Connected agents/tools registry
```

**Key Functions**:
```rust
// Translate MCP tool call to A2A task
pub fn mcp_to_a2a_task(tool_call: CallToolRequest) -> TaskRequest;

// Translate A2A task result to MCP response
pub fn a2a_to_mcp_response(task_result: TaskResult) -> CallToolResult;

// Route incoming request to appropriate protocol handler
pub async fn route_request(req: GatewayRequest) -> GatewayResponse;
```

**Effort**: ~500-600 lines new code

---

## Phase 1 Summary

| Task | New Lines | Modified Lines | Weeks |
|------|-----------|----------------|-------|
| MCP Bridge | 400-500 | 50 | 1-2 |
| A2A Types | 800-1000 | 0 | 1-2 |
| A2A Transport | 400-500 | 100 | 1-2 |
| Gateway | 500-600 | 50 | 1-2 |
| **Total** | **2100-2600** | **200** | **6-8** |

---

## Phase 2: Python Bindings (v0.6.0)

**Duration**: 3-4 weeks
**Goal**: PyPI package for protocol access

### 2.1 PyO3 Setup (Week 7)

**Create Python Package**:
```
skreaver-py/
├── Cargo.toml
├── pyproject.toml
├── src/
│   ├── lib.rs           # PyO3 module definition
│   ├── mcp.rs           # MCP type bindings
│   ├── a2a.rs           # A2A type bindings
│   └── gateway.rs       # Gateway client
└── python/
    └── skreaver/
        └── __init__.py
```

**Cargo.toml**:
```toml
[package]
name = "skreaver-py"
version = "0.6.0"

[lib]
name = "skreaver"
crate-type = ["cdylib"]

[dependencies]
pyo3 = { version = "0.22", features = ["extension-module"] }
pyo3-asyncio = { version = "0.22", features = ["tokio-runtime"] }
skreaver-mcp = { path = "../crates/skreaver-mcp" }
skreaver-a2a = { path = "../crates/skreaver-a2a" }
skreaver-gateway = { path = "../crates/skreaver-gateway" }
tokio = { workspace = true }
```

**Effort**: ~300-400 lines setup

### 2.2 Core Bindings (Week 8-9)

**Python API**:
```python
import skreaver

# MCP Client
client = skreaver.McpClient()
await client.connect_stdio("npx", ["-y", "@anthropic/mcp-server-fs"])
tools = await client.list_tools()
result = await client.call_tool("read_file", {"path": "/tmp/test.txt"})

# A2A Agent
agent = skreaver.A2aAgent(card=my_card)
task = await agent.create_task(target_agent="other-agent", input=data)
result = await task.wait()

# Gateway
gateway = skreaver.Gateway()
gateway.register_mcp_server("fs", client)
gateway.register_a2a_agent("my-agent", agent)
await gateway.start(port=8080)
```

**Effort**: ~600-800 lines

### 2.3 Package Publication (Week 10)

- [ ] maturin build configuration
- [ ] PyPI test publication
- [ ] Documentation (basic README)
- [ ] Example scripts

**Effort**: ~200 lines + config

---

## Phase 3: Edge & WASM (v0.7.0)

**Duration**: 4-6 weeks
**Goal**: <5MB binary, WASM support

**Deferred until Phase 1-2 proven** - This is complex and depends on:
- Which dependencies can be made no_std
- WASM ecosystem maturity
- Actual demand from users

### Preliminary Analysis

**Blockers for WASM**:
- `tokio` - needs `tokio_wasm` or alternative
- `reqwest` - needs `wasm32` target support
- `rmcp` - unknown WASM support
- File I/O tools - not applicable in browser

**Realistic Scope**:
- Protocol types only (no runtime)
- Message serialization
- Schema validation

---

## Phase 4: Enterprise Features (v0.7.0+)

**Deferred** - Focus on protocol core first

### Guardrails (If Needed)
- Already have security framework in skreaver-core
- Can extend with policy engine later
- ~500-800 lines if implemented

### K8s Operator (If Needed)
- Separate repo likely
- kube-rs based
- ~1500-2000 lines

---

## Realistic Timeline

| Phase | Weeks | Deliverable |
|-------|-------|-------------|
| 1.1 MCP Bridge | 2 | External MCP server consumption |
| 1.2 A2A Types | 2 | Agent Card, Task, Message types |
| 1.3 A2A Transport | 2 | HTTP endpoints for A2A |
| 1.4 Gateway | 2 | MCP ↔ A2A bridging |
| 2.1-2.3 Python | 4 | PyPI package |
| **Total v0.6.0** | **12** | Protocol bridge + Python |

---

## Dependencies

### New Dependencies (Phase 1)

```toml
# skreaver-a2a
jsonrpc-core = "18"         # JSON-RPC 2.0

# skreaver-py
pyo3 = "0.22"
pyo3-asyncio = "0.22"
maturin = "1.4"             # Build tool
```

### Dependency Risks

| Dependency | Risk | Mitigation |
|------------|------|------------|
| rmcp client | May not exist in 0.2.0 | Check crate, may need manual impl |
| pyo3-asyncio | Tokio compatibility | Test early |
| jsonrpc-core | Maintenance status | Consider jsonrpsee if needed |

---

## Testing Strategy

### Phase 1 Tests

| Component | Test Type | Count |
|-----------|-----------|-------|
| MCP Bridge | Integration | 5-10 |
| A2A Types | Unit | 15-20 |
| A2A Transport | Integration | 10-15 |
| Gateway | Integration | 10-15 |

### Test Infrastructure Needed

- [ ] Mock MCP server for bridge tests
- [ ] Mock A2A agent for transport tests
- [ ] Gateway integration test harness

---

## Success Criteria

### v0.6.0 Release

- [ ] MCP client connects to external MCP servers
- [ ] A2A agent card discovery works
- [ ] A2A task create/status/complete flow works
- [ ] Gateway bridges MCP ↔ A2A
- [ ] Python package installable via pip
- [ ] 50+ new tests passing
- [ ] Documentation for new features

### Performance Targets

| Metric | Target |
|--------|--------|
| MCP tool call latency | <10ms (local) |
| A2A message latency | <5ms (same host) |
| Gateway translation | <1ms overhead |
| Python call overhead | <5ms vs Rust |

---

**Document Status**: Revised based on code analysis
**Next Review**: After MCP Bridge implementation
