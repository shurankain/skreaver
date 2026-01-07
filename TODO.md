# Skreaver TODO

> **Updated**: January 2026
> **Current Version**: v0.5.0
> **Next Milestone**: v0.6.0

---

## Immediate (Week 1)

### Verify rmcp Client Capability

- [ ] Check if `rmcp` 0.2.0 has client feature
  ```bash
  cargo doc -p rmcp --all-features --open
  ```
- [ ] If no client: evaluate alternatives or plan manual implementation
- [ ] Document finding in `.dev/` notes

### MCP Bridge - Unblock

- [ ] Read `crates/skreaver-mcp/src/bridge.rs` skeleton code
- [ ] Identify what rmcp APIs are needed for client
- [ ] Create minimal test: connect to filesystem MCP server

---

## v0.6.0 - Phase 1: Protocol Core

### 1.1 MCP Bridge (Weeks 1-2)

**Location**: `crates/skreaver-mcp/src/bridge.rs`

- [ ] `McpBridge::connect_stdio(command, args)` - Spawn process
  - Use `tokio::process::Command`
  - Pipe stdin/stdout to rmcp transport
- [ ] `McpBridge::list_tools()` - Discover available tools
- [ ] `BridgedTool::call(name, params)` - Execute remote tool
- [ ] Add timeout handling (default 30s)
- [ ] Add reconnection logic on disconnect

**Tests** (create `tests/mcp_bridge.rs`):
- [ ] Connect to mock MCP server
- [ ] List tools from server
- [ ] Call tool and get result
- [ ] Handle server crash gracefully

**Files**:
```
crates/skreaver-mcp/src/bridge.rs      # ~400 lines to implement
crates/skreaver-mcp/src/error.rs       # Add ClientError variant
crates/skreaver-mcp/tests/bridge.rs    # New integration tests
```

### 1.2 A2A Protocol Types (Weeks 2-3)

**Create**: `crates/skreaver-a2a/`

- [ ] Initialize crate
  ```bash
  mkdir -p crates/skreaver-a2a/src
  ```
- [ ] Add to workspace `Cargo.toml`

**Core Types** (`src/agent_card.rs`):
- [ ] `AgentCard` struct (name, description, url, capabilities)
- [ ] `Capability` struct (name, input/output schema)
- [ ] `AuthInfo` enum (None, ApiKey, OAuth)
- [ ] JSON Schema validation for agent cards
- [ ] Serde serialization

**Task Types** (`src/task.rs`):
- [ ] `TaskId` newtype
- [ ] `TaskStatus` enum (Pending, Running, Completed, Failed, Cancelled)
- [ ] `Task` struct (id, status, input, output, created_at, updated_at)
- [ ] `TaskRequest` / `TaskResponse` for API

**Message Types** (`src/message.rs`):
- [ ] JSON-RPC 2.0 request/response wrappers
- [ ] `A2aRequest` enum (CreateTask, GetTask, SendMessage, CancelTask)
- [ ] `A2aResponse` enum with result/error
- [ ] Error codes per A2A spec

**Tests**:
- [ ] Agent card serialization round-trip
- [ ] Task state transitions
- [ ] JSON-RPC message parsing

**Files**:
```
crates/skreaver-a2a/Cargo.toml
crates/skreaver-a2a/src/lib.rs
crates/skreaver-a2a/src/agent_card.rs   # ~150 lines
crates/skreaver-a2a/src/task.rs         # ~200 lines
crates/skreaver-a2a/src/message.rs      # ~250 lines
crates/skreaver-a2a/src/error.rs        # ~50 lines
```

### 1.3 A2A HTTP Transport (Weeks 3-4)

**Location**: `crates/skreaver-http/src/runtime/handlers/a2a.rs`

**Endpoints**:
- [ ] `POST /a2a/agents` - Register agent card
- [ ] `GET /a2a/agents` - List registered agents
- [ ] `GET /a2a/agents/{id}/card` - Get agent card
- [ ] `POST /a2a/tasks` - Create task
- [ ] `GET /a2a/tasks/{id}` - Get task status
- [ ] `POST /a2a/tasks/{id}/messages` - Send message
- [ ] `GET /a2a/tasks/{id}/stream` - SSE for updates

**State Management**:
- [ ] Agent registry (in-memory HashMap + optional persistence)
- [ ] Task store (in-memory, use skreaver-memory later)

**Router Integration**:
- [ ] Add A2A routes to `runtime/router.rs`
- [ ] Apply auth middleware to protected routes

**Files**:
```
crates/skreaver-http/src/runtime/handlers/a2a.rs    # ~300 lines
crates/skreaver-http/src/runtime/handlers/mod.rs    # Add export
crates/skreaver-http/src/runtime/router.rs          # Add routes
crates/skreaver-a2a/src/transport.rs                # HTTP client
```

### 1.4 Protocol Gateway (Weeks 5-6)

**Option A**: New crate `crates/skreaver-gateway/`
**Option B**: Module in `crates/skreaver-http/src/gateway/`

Choose based on complexity after 1.1-1.3 done.

**Core Functions**:
- [ ] `mcp_to_a2a(CallToolRequest) -> TaskRequest`
- [ ] `a2a_to_mcp(TaskResult) -> CallToolResult`
- [ ] `GatewayRegistry` - Track connected MCP servers + A2A agents

**HTTP Endpoints**:
- [ ] `POST /gateway/mcp/{server}/tools/{tool}` - Call MCP tool
- [ ] `POST /gateway/a2a/{agent}/tasks` - Create A2A task
- [ ] `GET /gateway/status` - List all connected endpoints

**Files**:
```
crates/skreaver-gateway/src/lib.rs
crates/skreaver-gateway/src/bridge.rs      # Translation logic
crates/skreaver-gateway/src/registry.rs    # Connection tracking
crates/skreaver-gateway/src/handlers.rs    # HTTP endpoints
```

---

## v0.6.0 - Phase 2: Python Bindings

### 2.1 PyO3 Setup (Week 7)

- [ ] Create `skreaver-py/` directory
- [ ] `Cargo.toml` with pyo3 dependencies
- [ ] `pyproject.toml` for maturin
- [ ] Basic `lib.rs` with module definition
- [ ] Verify build: `maturin develop`

**Files**:
```
skreaver-py/Cargo.toml
skreaver-py/pyproject.toml
skreaver-py/src/lib.rs
skreaver-py/python/skreaver/__init__.py
```

### 2.2 MCP Bindings (Week 8)

- [ ] `McpClient` class
  - `connect_stdio(command, args)` async
  - `list_tools()` async
  - `call_tool(name, params)` async
  - `disconnect()`

**File**: `skreaver-py/src/mcp.rs`

### 2.3 A2A Bindings (Week 8-9)

- [ ] `AgentCard` dataclass
- [ ] `A2aClient` class
  - `register_agent(card)` async
  - `create_task(agent_id, input)` async
  - `get_task(task_id)` async
  - `wait_for_task(task_id)` async

**File**: `skreaver-py/src/a2a.rs`

### 2.4 Gateway Bindings (Week 9)

- [ ] `Gateway` class
  - `register_mcp_server(name, client)`
  - `register_a2a_agent(name, card)`
  - `start(host, port)` async
  - `stop()`

**File**: `skreaver-py/src/gateway.rs`

### 2.5 Publication (Week 10)

- [ ] Test on PyPI test server
- [ ] Write README for Python package
- [ ] Create example scripts
- [ ] Publish to PyPI

---

## Testing Checklist

### MCP Bridge Tests
- [ ] `test_connect_to_filesystem_server`
- [ ] `test_list_tools_from_server`
- [ ] `test_call_tool_success`
- [ ] `test_call_tool_not_found`
- [ ] `test_server_disconnect_handling`
- [ ] `test_connection_timeout`

### A2A Tests
- [ ] `test_agent_card_serialization`
- [ ] `test_agent_registration`
- [ ] `test_task_creation`
- [ ] `test_task_status_updates`
- [ ] `test_task_completion`
- [ ] `test_task_cancellation`
- [ ] `test_sse_streaming`

### Gateway Tests
- [ ] `test_mcp_to_a2a_translation`
- [ ] `test_a2a_to_mcp_translation`
- [ ] `test_gateway_routing`
- [ ] `test_registry_management`

### Python Tests
- [ ] `test_mcp_client_basic`
- [ ] `test_a2a_client_basic`
- [ ] `test_gateway_integration`
- [ ] `test_async_operations`

---

## Documentation TODO

### New Docs Needed
- [ ] `docs/mcp-bridge.md` - How to use MCP client
- [ ] `docs/a2a-protocol.md` - A2A implementation guide
- [ ] `docs/gateway.md` - Gateway configuration
- [ ] `docs/python-quickstart.md` - Python package usage

### Update Existing
- [ ] README.md - New protocol focus
- [ ] CHANGELOG.md - v0.6.0 changes
- [ ] API_STABILITY.md - New crate stability levels

---

## Deferred (v0.7.0+)

### Edge Runtime
- [ ] Investigate WASM feasibility
- [ ] Identify no_std compatible types
- [ ] Binary size analysis

### Guardrails
- [ ] Policy engine design
- [ ] Integration with existing security

### Deprecation
- [ ] skreaver-mesh deprecation warnings
- [ ] Migration guide to A2A

---

## Completed (v0.5.0)

- [x] Core framework (Agent, Memory, Tool traits)
- [x] Security framework (A+ grade)
- [x] Memory backends (File, Redis, SQLite, PostgreSQL)
- [x] HTTP runtime (Axum, WebSocket, OpenAPI)
- [x] MCP server (expose tools to Claude Desktop)
- [x] Mesh coordination (Redis pub/sub)
- [x] Observability (Prometheus, OpenTelemetry)
- [x] 400+ tests passing

---

**Last Updated**: January 2026
