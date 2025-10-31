# WebSocket Guide

**Version**: 0.5.0
**Status**: ✅ **STABLE**
**Feature Flag**: `websocket` (enabled by default)

---

## Overview

Skreaver's WebSocket implementation provides production-ready, real-time bidirectional communication for agent systems with:

- ✅ **Type-Safe**: Compile-time prevention of deadlocks and race conditions
- ✅ **Secure**: Authentication, authorization, and rate limiting
- ✅ **Scalable**: Tested with 1000+ concurrent connections
- ✅ **Observable**: Prometheus metrics and structured logging
- ✅ **Reliable**: Automatic reconnection, heartbeat, and error handling

---

## Quick Start

### 1. Enable WebSocket Feature

WebSocket support is enabled by default in `skreaver-http`:

```toml
[dependencies]
skreaver-http = { version = "0.3", features = ["websocket"] }
```

### 2. Basic Server Setup

```rust
use skreaver_http::websocket::{WebSocketConfig, WebSocketManager};
use std::sync::Arc;

// Create WebSocket configuration
let config = WebSocketConfig::default();

// Create WebSocket manager
let manager = Arc::new(WebSocketManager::new(config));
```

### 3. Run Example

```bash
cargo run --example websocket_server --features websocket
```

---

## Architecture

### Components

```
┌─────────────────────────────────────────────────┐
│          WebSocket HTTP Handler                  │
│    (Axum upgrade + authentication)               │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│         WebSocketManager                         │
│  - Connection lifecycle                          │
│  - Channel subscriptions                         │
│  - Message broadcasting                          │
│  - Rate limiting                                 │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│       Lock Ordering System                       │
│  - Type-safe lock acquisition                    │
│  - Deadlock prevention                           │
│  - Concurrent access control                     │
└─────────────────────────────────────────────────┘
```

### Message Flow

```
Client                Server              Manager
  │                     │                   │
  ├──── Upgrade ───────>│                   │
  │                     │                   │
  │<──── 101 ───────────┤                   │
  │                     │                   │
  ├──── Subscribe ──────┼──────────────────>│
  │                     │                   │
  │<──── Ack ───────────┼───────────────────┤
  │                     │                   │
  │<──── Event ─────────┼───────────────────┤
  │                     │     (broadcast)   │
  │                     │                   │
  ├──── Pong ───────────┼──────────────────>│
  │                     │    (heartbeat)    │
```

---

## Configuration

### WebSocketConfig

```rust
pub struct WebSocketConfig {
    /// Maximum number of concurrent connections
    pub max_connections: usize,              // Default: 1000

    /// Connection timeout in seconds
    pub connection_timeout: Duration,         // Default: 60s

    /// Ping interval for heartbeat
    pub ping_interval: Duration,              // Default: 30s

    /// Pong timeout (wait time after ping)
    pub pong_timeout: Duration,               // Default: 10s

    /// Maximum message size in bytes
    pub max_message_size: usize,              // Default: 64KB

    /// Enable WebSocket compression
    pub enable_compression: bool,             // Default: true

    /// Buffer size for incoming messages
    pub buffer_size: usize,                   // Default: 100

    /// Maximum subscriptions per connection
    pub max_subscriptions_per_connection: usize,  // Default: 50

    /// Maximum subscribers per channel
    pub max_subscribers_per_channel: usize,   // Default: 10000

    /// Maximum connections per IP address
    pub max_connections_per_ip: usize,        // Default: 10

    /// Broadcast channel buffer size
    pub broadcast_buffer_size: usize,         // Default: 1000
}
```

### Production Configuration Example

```rust
let config = WebSocketConfig {
    max_connections: 5000,
    connection_timeout: Duration::from_secs(300),  // 5 minutes
    ping_interval: Duration::from_secs(30),
    pong_timeout: Duration::from_secs(10),
    max_message_size: 256 * 1024,  // 256KB
    enable_compression: true,
    buffer_size: 500,
    max_subscriptions_per_connection: 100,
    max_subscribers_per_channel: 50000,
    max_connections_per_ip: 50,
    broadcast_buffer_size: 5000,
};
```

---

## Message Protocol

### Message Types

```rust
pub enum WsMessageType {
    /// Client → Server: Authentication
    Auth,

    /// Server → Client: Authentication response
    AuthResponse,

    /// Client → Server: Subscribe to channels
    Subscribe,

    /// Server → Client: Subscription confirmation
    SubscribeResponse,

    /// Client → Server: Unsubscribe from channels
    Unsubscribe,

    /// Server → Client: Unsubscription confirmation
    UnsubscribeResponse,

    /// Client → Server: Request
    Request,

    /// Server → Client: Response to request
    Response,

    /// Server → Client: Event/notification
    Event,

    /// Client → Server: Acknowledgment
    Ack,

    /// Bidirectional: Heartbeat ping
    Ping,

    /// Bidirectional: Heartbeat pong
    Pong,

    /// Server → Client: Error message
    Error,
}
```

### Message Format

All messages use JSON format:

```json
{
  "type": "subscribe",
  "data": {
    "channels": ["agent-updates", "system-events"]
  },
  "correlation_id": "req-12345",
  "metadata": {
    "client_version": "1.0.0"
  }
}
```

---

## Authentication

### Implementing AuthHandler

```rust
use skreaver_http::websocket::AuthHandler;

struct MyAuthHandler {
    db: Arc<Database>,
}

#[async_trait::async_trait]
impl AuthHandler for MyAuthHandler {
    async fn authenticate(&self, token: &str) -> Result<String, String> {
        // Validate JWT or API key
        match validate_token(token).await {
            Ok(user_id) => Ok(user_id),
            Err(e) => Err(format!("Authentication failed: {}", e))
        }
    }

    async fn check_permission(&self, user_id: &str, channel: &str) -> bool {
        // Check if user can access channel
        self.db.check_channel_permission(user_id, channel).await
    }
}
```

### Client Connection with Auth

```bash
# JWT authentication
websocat "ws://localhost:8080/ws?token=eyJhbGciOiJIUzI1NiIs..."

# API key authentication
websocat "ws://localhost:8080/ws?token=sk-live-abc123..."
```

Or via message after connection:

```json
{
  "type": "auth",
  "data": {
    "token": "eyJhbGciOiJIUzI1NiIs..."
  }
}
```

---

## Channel Subscriptions

### Subscribe to Channels

**Client → Server:**
```json
{
  "type": "subscribe",
  "data": {
    "channels": ["agent-updates", "notifications", "metrics"]
  }
}
```

**Server → Client:**
```json
{
  "type": "subscribe_response",
  "data": {
    "success": true,
    "subscribed": ["agent-updates", "notifications", "metrics"]
  }
}
```

### Unsubscribe

**Client → Server:**
```json
{
  "type": "unsubscribe",
  "data": {
    "channels": ["metrics"]
  }
}
```

### Broadcasting to Channel

```rust
// Server-side broadcasting
let message = WsMessage {
    msg_type: WsMessageType::Event,
    data: serde_json::json!({
        "type": "agent-status-changed",
        "agent_id": "agent-123",
        "status": "running"
    }),
    correlation_id: None,
    metadata: HashMap::new(),
};

let delivered = manager.broadcast("agent-updates", message).await?;
println!("Message delivered to {} subscribers", delivered);
```

---

## Connection Management

### Adding Connection

```rust
use skreaver_http::websocket::ConnectionInfo;
use std::net::SocketAddr;

let addr: SocketAddr = "127.0.0.1:12345".parse()?;
let info = ConnectionInfo::new(addr);
let conn_id = info.id;

let sender = manager.add_connection(conn_id, info).await?;
```

### Removing Connection

```rust
manager.remove_connection(conn_id).await;
```

### Getting Connection Stats

```rust
let stats = manager.get_stats().await;
println!("Active connections: {}", stats.active_connections);
println!("Total connections: {}", stats.total_connections);
println!("Peak connections: {}", stats.peak_connections);
println!("Messages sent: {}", stats.messages_sent);
println!("Messages received: {}", stats.messages_received);
```

---

## Security Features

### 1. Rate Limiting

Automatic rate limiting per IP address:

```rust
let config = WebSocketConfig {
    max_connections_per_ip: 10,  // Limit to 10 connections per IP
    ..Default::default()
};
```

### 2. Message Size Limits

Prevent DoS attacks with message size limits:

```rust
let config = WebSocketConfig {
    max_message_size: 64 * 1024,  // 64KB max
    ..Default::default()
};
```

### 3. Subscription Limits

Prevent resource exhaustion:

```rust
let config = WebSocketConfig {
    max_subscriptions_per_connection: 50,
    max_subscribers_per_channel: 10000,
    ..Default::default()
};
```

### 4. Connection Timeouts

Automatic cleanup of stale connections:

```rust
let config = WebSocketConfig {
    connection_timeout: Duration::from_secs(300),  // 5 minutes
    pong_timeout: Duration::from_secs(10),
    ..Default::default()
};
```

---

## Heartbeat & Keep-Alive

The WebSocket manager automatically sends ping messages at regular intervals:

```rust
let config = WebSocketConfig {
    ping_interval: Duration::from_secs(30),  // Ping every 30s
    pong_timeout: Duration::from_secs(10),   // Wait 10s for pong
    ..Default::default()
};
```

**Flow:**
1. Server sends `Ping` message every 30s
2. Client must respond with `Pong` within 10s
3. If no pong received, connection is closed

**Client Implementation:**
```javascript
// JavaScript client
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.type === 'ping') {
    ws.send(JSON.stringify({ type: 'pong' }));
  }
};
```

---

## Error Handling

### Connection Errors

```rust
use skreaver_http::websocket::WsError;

match manager.add_connection(conn_id, info).await {
    Ok(sender) => {
        // Connection successful
    }
    Err(WsError::ConnectionLimit) => {
        // Too many connections
    }
    Err(WsError::DuplicateConnection) => {
        // Connection already exists
    }
    Err(WsError::AuthenticationFailed(msg)) => {
        // Authentication failed
    }
    Err(e) => {
        // Other errors
    }
}
```

### Subscription Errors

```rust
match manager.subscribe(conn_id, channels).await {
    Ok(_) => {
        // Subscription successful
    }
    Err(WsError::ConnectionNotFound) => {
        // Connection doesn't exist
    }
    Err(WsError::SubscriptionLimitExceeded) => {
        // Too many subscriptions
    }
    Err(WsError::PermissionDenied { channel, .. }) => {
        // No permission for channel
    }
    Err(e) => {
        // Other errors
    }
}
```

---

## Observability

### Prometheus Metrics

WebSocket metrics are exported at `/metrics`:

```
# Connection metrics
websocket_connections_total           # Total connections
websocket_connections_active          # Currently active
websocket_connections_failed_total    # Failed connection attempts

# Message metrics
websocket_messages_sent_total         # Total messages sent
websocket_messages_received_total     # Total messages received
websocket_broadcast_delivered_total   # Broadcast delivery count

# Error metrics
websocket_errors_total{type}          # Errors by type
websocket_subscriptions_failed_total  # Failed subscriptions
```

### Structured Logging

```rust
tracing::info!(
    connection_id = %conn_id,
    client_addr = %addr,
    "WebSocket connection established"
);

tracing::warn!(
    connection_id = %conn_id,
    channel = %channel,
    "Subscription denied: insufficient permissions"
);

tracing::error!(
    connection_id = %conn_id,
    error = %e,
    "WebSocket error occurred"
);
```

---

## Performance

### Benchmarks

**Tested on**: MacBook Pro M1, 16GB RAM

| Metric | Result |
|--------|--------|
| **Max Concurrent Connections** | 1000+ |
| **Messages/Second** | 10,000+ |
| **Latency (p50)** | < 1ms |
| **Latency (p99)** | < 5ms |
| **Memory per Connection** | ~5KB |
| **CPU Usage (1000 conn)** | ~15% |

### Optimization Tips

1. **Enable Compression** - Reduces bandwidth by 60-80%
   ```rust
   config.enable_compression = true;
   ```

2. **Tune Buffer Sizes** - Balance memory vs throughput
   ```rust
   config.buffer_size = 500;           // Per-connection buffer
   config.broadcast_buffer_size = 5000; // Broadcast channel
   ```

3. **Adjust Heartbeat** - Reduce ping frequency for low-latency networks
   ```rust
   config.ping_interval = Duration::from_secs(60);
   ```

4. **Connection Pooling** - Reuse connections when possible

---

## Client Libraries

### JavaScript/TypeScript

```javascript
const ws = new WebSocket('ws://localhost:8080/ws?token=abc123');

ws.onopen = () => {
  // Subscribe to channels
  ws.send(JSON.stringify({
    type: 'subscribe',
    data: { channels: ['agent-updates'] }
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  if (msg.type === 'ping') {
    ws.send(JSON.stringify({ type: 'pong' }));
  } else if (msg.type === 'event') {
    console.log('Event received:', msg.data);
  }
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

ws.onclose = () => {
  console.log('WebSocket closed');
  // Implement reconnection logic
};
```

### Python

```python
import asyncio
import websockets
import json

async def connect():
    uri = "ws://localhost:8080/ws?token=abc123"
    async with websockets.connect(uri) as ws:
        # Subscribe
        await ws.send(json.dumps({
            "type": "subscribe",
            "data": {"channels": ["agent-updates"]}
        }))

        # Receive messages
        async for message in ws:
            msg = json.loads(message)
            if msg["type"] == "ping":
                await ws.send(json.dumps({"type": "pong"}))
            elif msg["type"] == "event":
                print(f"Event: {msg['data']}")

asyncio.run(connect())
```

### Rust Client

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::{StreamExt, SinkExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "ws://localhost:8080/ws?token=abc123";
    let (ws_stream, _) = connect_async(url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Subscribe
    let sub_msg = serde_json::json!({
        "type": "subscribe",
        "data": {"channels": ["agent-updates"]}
    });
    write.send(Message::Text(sub_msg.to_string())).await?;

    // Read messages
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if let Message::Text(text) = msg {
            let data: serde_json::Value = serde_json::from_str(&text)?;
            if data["type"] == "ping" {
                let pong = serde_json::json!({"type": "pong"});
                write.send(Message::Text(pong.to_string())).await?;
            } else {
                println!("Received: {}", text);
            }
        }
    }

    Ok(())
}
```

---

## Testing

### Unit Tests

31 unit tests cover all WebSocket functionality:

```bash
cargo test --package skreaver-http websocket
```

**Coverage:**
- ✅ Connection management
- ✅ Authentication
- ✅ Subscriptions
- ✅ Broadcasting
- ✅ Lock ordering
- ✅ Error handling
- ✅ Rate limiting
- ✅ Concurrent access

### Integration Testing

```rust
#[tokio::test]
async fn test_full_websocket_flow() {
    let manager = WebSocketManager::new(WebSocketConfig::default());

    // Add connection
    let info = ConnectionInfo::new("127.0.0.1:8080".parse().unwrap());
    let conn_id = info.id;
    let _sender = manager.add_connection(conn_id, info).await.unwrap();

    // Subscribe
    let channels = vec!["test-channel".to_string()];
    manager.subscribe(conn_id, channels).await.unwrap();

    // Broadcast
    let message = WsMessage::event(serde_json::json!({"test": "data"}));
    let delivered = manager.broadcast("test-channel", message).await.unwrap();
    assert_eq!(delivered, 1);

    // Cleanup
    manager.remove_connection(conn_id).await;
}
```

---

## Migration Guide

### From v0.4.0 to v0.5.0

**Feature Flag Renamed:**
```toml
# Old (v0.4.0)
skreaver-http = { version = "0.4", features = ["unstable-websocket"] }

# New (v0.5.0) - websocket is now stable and included in defaults
skreaver-http = "0.5"
# Or explicitly:
skreaver-http = { version = "0.5", features = ["websocket"] }
```

**No API Changes** - All WebSocket APIs are stable and backward-compatible.

---

## Stability Guarantees

### API Stability

✅ **STABLE** - The following APIs are stable and will not have breaking changes in v0.5.x:

- `WebSocketConfig` - All configuration fields
- `WebSocketManager` - Public methods
- `WsMessage` - Message structure and types
- `WsMessageType` - Message type enum
- `AuthHandler` - Authentication trait
- `WsError` - Error types

### SemVer Policy

- **Patch versions** (0.5.x): Bug fixes only, no API changes
- **Minor versions** (0.x.0): New features, backward-compatible additions
- **Major versions** (x.0.0): Breaking changes (with deprecation period)

### Deprecation Policy

1. Features marked `#[deprecated]` in minor version
2. Deprecation period: minimum 2 minor versions (e.g., 0.5.0 → 0.7.0)
3. Removal in next major version
4. Migration guide provided

---

## Troubleshooting

### Connection Refused

**Symptom:** `Connection refused` error when connecting

**Solutions:**
1. Check server is running: `netstat -an | grep 8080`
2. Verify firewall rules allow port 8080
3. Check bind address (use `0.0.0.0` for all interfaces)

### Authentication Failures

**Symptom:** Connection closes immediately after upgrade

**Solutions:**
1. Verify token is valid
2. Check token is passed correctly: `?token=...`
3. Review `AuthHandler::authenticate` implementation
4. Check logs for authentication errors

### Message Not Received

**Symptom:** Broadcast not reaching subscribers

**Solutions:**
1. Verify client subscribed to channel
2. Check channel name matches exactly
3. Ensure connection still active
4. Review subscription limits

### High Memory Usage

**Symptom:** Memory grows unbounded

**Solutions:**
1. Reduce `buffer_size` and `broadcast_buffer_size`
2. Lower `max_connections`
3. Implement connection pruning
4. Enable compression

### Slow Performance

**Symptom:** High latency or low throughput

**Solutions:**
1. Enable compression: `config.enable_compression = true`
2. Increase buffer sizes
3. Reduce ping frequency
4. Check network conditions
5. Profile with `cargo flamegraph`

---

## Production Deployment

### Docker

```dockerfile
FROM rust:1.80 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --features websocket

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/your-app /usr/local/bin/
EXPOSE 8080
CMD ["your-app"]
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: skreaver-websocket
spec:
  replicas: 3
  selector:
    matchLabels:
      app: skreaver-websocket
  template:
    metadata:
      labels:
        app: skreaver-websocket
    spec:
      containers:
      - name: skreaver
        image: your-registry/skreaver:latest
        ports:
        - containerPort: 8080
        env:
        - name: RUST_LOG
          value: "info"
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
```

### Load Balancer Configuration

**Nginx:**
```nginx
upstream websocket {
    ip_hash;  # Sticky sessions
    server backend1:8080;
    server backend2:8080;
    server backend3:8080;
}

server {
    location /ws {
        proxy_pass http://websocket;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
    }
}
```

---

## FAQ

### Q: Is WebSocket production-ready?

**A:** Yes! As of v0.5.0, WebSocket is stable and production-ready with comprehensive testing, security features, and observability.

### Q: What's the maximum number of connections?

**A:** Tested with 1000+ concurrent connections. Actual limit depends on available resources (memory, file descriptors). Configure `max_connections` accordingly.

### Q: Does it support clustering?

**A:** WebSocket connections are server-local. For clustering, use Redis Pub/Sub or similar for cross-server message delivery.

### Q: How do I handle reconnections?

**A:** Implement exponential backoff on the client side. The server automatically cleans up stale connections.

### Q: Can I use TLS/WSS?

**A:** Yes! Configure TLS in your reverse proxy (Nginx, Traefik, etc.) or use Axum's TLS support.

### Q: What about binary messages?

**A:** Currently text/JSON only. Binary support planned for v0.6.0.

---

## Resources

- **Example Code**: [examples/websocket_server.rs](examples/websocket_server.rs)
- **API Docs**: Run `cargo doc --open --features websocket`
- **Tests**: [tests/websocket_security_tests.rs](tests/websocket_security_tests.rs)
- **GitHub Issues**: Report bugs or request features

---

## License

MIT License - See [LICENSE](LICENSE) file for details.
