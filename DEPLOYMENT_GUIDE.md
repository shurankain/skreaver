# Skreaver Production Deployment Guide

This comprehensive guide covers deploying applications built with Skreaver to production environments, with focus on Kubernetes deployments, security hardening, and operational best practices.

**Note**: Skreaver is a library crate that you integrate into your own Rust applications. This guide assumes you've built a binary using `skreaver-http` runtime and want to deploy it to production.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Build and Container Image](#build-and-container-image)
3. [Security Configuration](#security-configuration)
4. [Kubernetes Deployment](#kubernetes-deployment)
5. [Security Hardening](#security-hardening)
6. [Monitoring and Observability](#monitoring-and-observability)
7. [Backup and Disaster Recovery](#backup-and-disaster-recovery)
8. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### System Requirements

**Minimum Resources**:
- CPU: 2 cores
- Memory: 512 MB
- Disk: 1 GB
- Network: 100 Mbps

**Recommended Resources**:
- CPU: 4 cores
- Memory: 2 GB
- Disk: 10 GB (for logs and SQLite if used)
- Network: 1 Gbps

### Software Dependencies

- **Rust**: 1.75+ (for building)
- **Docker**: 20.10+ (for containerization)
- **Kubernetes**: 1.24+ (for orchestration)
- **Prometheus**: 2.40+ (for metrics)
- **Grafana**: 9.0+ (for dashboards)

### Network Requirements

**Inbound Ports**:
- `8080`: HTTP API (configurable)
- `9090`: Metrics endpoint (if separate)

**Outbound Access**:
- HTTPS (443) for tool execution (if HTTP tools enabled)
- Database ports (5432 for PostgreSQL, custom for SQLite)

---

## Build and Container Image

### Building Your Application

Skreaver is a library, so you'll need to create your own binary. Here's an example `Cargo.toml` for your application:

```toml
[package]
name = "my-skreaver-app"
version = "0.1.0"
edition = "2024"

[dependencies]
skreaver = { version = "0.3", features = ["websocket", "postgres", "observability"] }
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.5", features = ["cors"] }
```

Example `main.rs` (see `/examples/http_server.rs` for full example):

```rust
use skreaver::{
    Agent, HttpAgentRuntime, InMemoryToolRegistry, HttpGetTool,
    TextUppercaseTool, JsonParseTool,
};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create tool registry
    let registry = InMemoryToolRegistry::new()
        .with_tool("http_get", Arc::new(HttpGetTool::new()))
        .with_tool("text_uppercase", Arc::new(TextUppercaseTool::new()))
        .with_tool("json_parse", Arc::new(JsonParseTool::new()));

    // Create HTTP runtime
    let runtime = HttpAgentRuntime::new(registry);

    // Add your agents...

    // Create router
    let app = runtime.router();

    // Start server with graceful shutdown (CRITICAL for Kubernetes)
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
```

**Note**: The `.with_graceful_shutdown()` call is **critical** for production Kubernetes deployments. It ensures:
- ✅ Zero-downtime rolling updates
- ✅ Safe pod termination without data loss
- ✅ Graceful handling of SIGTERM signals from Kubernetes

See [GRACEFUL_SHUTDOWN_IMPLEMENTATION.md](GRACEFUL_SHUTDOWN_IMPLEMENTATION.md) for details.

Example with shutdown signal import:

```rust
use skreaver::{
    Agent, HttpAgentRuntime, InMemoryToolRegistry,
    runtime::shutdown_signal, // ← Import shutdown_signal
};

// ... agent setup ...

axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal())
    .await?;
}
```

Build commands:

```bash
# Development build
cargo build

# Release build with optimizations
cargo build --release

# Build with specific features
cargo build --release --features websocket,postgres

# The binary will be at: target/release/my-skreaver-app
```

### Creating Container Image

**Dockerfile Example**:

```dockerfile
# Multi-stage build for minimal image size
FROM rust:1.75-slim as builder

WORKDIR /app

# Copy your application source
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 skreaver && \
    mkdir -p /app /data /logs && \
    chown -R skreaver:skreaver /app /data /logs

WORKDIR /app

# Copy binary from builder (replace 'my-skreaver-app' with your binary name)
COPY --from=builder /app/target/release/my-skreaver-app /app/skreaver-app

# Copy default configuration
COPY config/security.toml /app/config/security.toml

USER skreaver

EXPOSE 8080

# Health check using the built-in /health endpoint
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

ENTRYPOINT ["/app/skreaver-app"]
```

**Note**: Install `wget` in runtime image for health check:
```dockerfile
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    wget \
    && rm -rf /var/lib/apt/lists/*
```

**Build and Push**:

```bash
# Build image
docker build -t myregistry/skreaver-http:v0.5.0 .

# Tag as latest
docker tag myregistry/skreaver-http:v0.5.0 myregistry/skreaver-http:latest

# Push to registry
docker push myregistry/skreaver-http:v0.5.0
docker push myregistry/skreaver-http:latest
```

### Image Optimization

**Multi-Architecture Builds**:

```bash
# Build for multiple architectures
docker buildx build --platform linux/amd64,linux/arm64 \
    -t myregistry/skreaver-http:v0.5.0 \
    --push .
```

**Image Size Reduction**:
- Use `distroless` base images for even smaller footprint
- Strip debug symbols: `strip --strip-all target/release/skreaver-http`
- Use Alpine Linux for minimal base (requires musl build)

---

## Security Configuration

### Security Policy File

Create a comprehensive security policy file (`security.toml`):

```toml
[metadata]
version = "1.0"
created = "2025-01-15T00:00:00Z"
description = "Production security policy"

# Filesystem access restrictions
[fs]
allow_paths = [
    "/app/data",
    "/tmp/skreaver"
]
deny_patterns = [
    "**/.env",
    "**/.git/**",
    "**/node_modules/**",
    "**/*.key",
    "**/*.pem"
]
max_file_size_mb = 100
allow_symlinks = false

# HTTP access restrictions
[http]
allow_domains = [
    "api.example.com",
    "*.trusted-service.com"
]
deny_patterns = [
    "*.internal",
    "localhost",
    "127.0.0.1"
]
max_request_size_mb = 50
timeout_seconds = 30

# Network restrictions
[network]
allow_ports = [80, 443, 8080]
deny_private_ips = true
dns_timeout_seconds = 5

# Resource limits
[resources]
max_memory_mb = 1024
max_cpu_percent = 80.0
max_execution_time = 300

# Audit logging
[audit]
enabled = true
log_level = "info"
include_request_bodies = false
include_response_bodies = false
retention_days = 90

# Secret management
[secrets]
redact_in_logs = true
rotation_days = 90
allowed_sources = ["env", "file"]
deny_patterns = [
    "password",
    "secret",
    "token",
    "key",
    "credential"
]

# Alerting
[alerting]
enabled = true
email_recipients = ["security@example.com", "ops@example.com"]
webhook_url = "https://alerts.example.com/webhook"
critical_cooldown_minutes = 5
warning_cooldown_minutes = 15

# Development settings (disable in production)
[development]
allow_dangerous_operations = false
verbose_errors = false
debug_endpoints = false

# Emergency controls
[emergency]
enabled = true
kill_switch_enabled = true
emergency_contacts = ["oncall@example.com"]
```

### Environment Variables

Skreaver supports comprehensive runtime configuration via environment variables, allowing you to adjust settings without rebuilding your application. This is essential for production Kubernetes deployments.

#### Required Variables

```bash
# JWT secret (REQUIRED in release builds, panics if not set)
# Generate with: openssl rand -base64 32
SKREAVER_JWT_SECRET=your-secret-key-here

# Logging (recommended)
RUST_LOG=info,my_skreaver_app=debug
```

#### HTTP Runtime Configuration

```bash
# Request handling
SKREAVER_REQUEST_TIMEOUT_SECS=30               # Request timeout (default: 30, max: 300)
SKREAVER_MAX_BODY_SIZE=16777216                # Max request body (default: 16MB, max: 100MB)
SKREAVER_ENABLE_CORS=true                      # Enable CORS (default: true)
SKREAVER_ENABLE_OPENAPI=false                  # Enable OpenAPI docs (default: true, disable in prod)

# Security configuration file path
SKREAVER_SECURITY_CONFIG_PATH=/app/config/security.toml  # Path to security.toml
```

#### Rate Limiting Configuration

```bash
SKREAVER_RATE_LIMIT_GLOBAL_RPM=1000            # Global requests/minute (default: 1000)
SKREAVER_RATE_LIMIT_PER_IP_RPM=60              # Per-IP requests/minute (default: 60)
SKREAVER_RATE_LIMIT_PER_USER_RPM=120           # Per-user requests/minute (default: 120)
```

#### Backpressure Configuration

```bash
SKREAVER_BACKPRESSURE_MAX_QUEUE_SIZE=100       # Max queue size per agent (default: 100)
SKREAVER_BACKPRESSURE_MAX_CONCURRENT=10        # Max concurrent requests/agent (default: 10)
SKREAVER_BACKPRESSURE_GLOBAL_MAX_CONCURRENT=500 # Global max concurrent (default: 500)
SKREAVER_BACKPRESSURE_QUEUE_TIMEOUT_SECS=30    # Queue timeout (default: 30)
SKREAVER_BACKPRESSURE_PROCESSING_TIMEOUT_SECS=60 # Processing timeout (default: 60)
SKREAVER_BACKPRESSURE_ENABLE_ADAPTIVE=true     # Enable adaptive backpressure (default: true)
SKREAVER_BACKPRESSURE_TARGET_PROCESSING_MS=1000 # Target processing time (default: 1000)
SKREAVER_BACKPRESSURE_LOAD_THRESHOLD=0.8       # Load threshold 0.0-1.0 (default: 0.8)
```

#### Observability Configuration

```bash
SKREAVER_OBSERVABILITY_ENABLE_METRICS=true     # Enable Prometheus metrics (default: true)
SKREAVER_OBSERVABILITY_ENABLE_TRACING=false    # Enable OpenTelemetry tracing (default: false)
SKREAVER_OBSERVABILITY_ENABLE_HEALTH=true      # Enable health checks (default: true)
SKREAVER_OBSERVABILITY_OTEL_ENDPOINT=http://otel-collector:4317  # OTLP endpoint
SKREAVER_OBSERVABILITY_NAMESPACE=skreaver      # Metrics namespace (default: "skreaver")
```

#### Database Configuration (Optional)

```bash
# PostgreSQL or SQLite (if using memory backends)
DATABASE_URL=postgresql://user:pass@postgres:5432/skreaver
# or for SQLite:
DATABASE_URL=sqlite:///data/skreaver.db
```

#### Loading Configuration in Code

**Option 1: Environment Variables Only (Recommended for Production)**

```rust
use skreaver::runtime::{HttpAgentRuntime, HttpRuntimeConfigBuilder, shutdown_signal};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load all configuration from environment variables
    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("Failed to load config from environment")
        .build()
        .expect("Failed to validate config");

    // Create runtime with environment-based configuration
    let runtime = HttpAgentRuntime::with_config(registry, config);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, runtime.router())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
```

**Option 2: Mix Environment Variables with Code-based Configuration**

```rust
use skreaver::runtime::{HttpAgentRuntime, HttpRuntimeConfigBuilder};
use std::path::PathBuf;

// Start with env vars, then override specific settings
let config = HttpRuntimeConfigBuilder::from_env()?
    .security_config_path(PathBuf::from("/custom/security.toml"))
    .enable_openapi(false)  // Force disable OpenAPI in production
    .build()?;

let runtime = HttpAgentRuntime::with_config(registry, config);
```

**Option 3: Fully Code-based Configuration (Not Recommended for Production)**

```rust
use skreaver::runtime::{HttpRuntimeConfig, HttpRuntimeConfigBuilder, RateLimitConfig, BackpressureConfig};
use std::path::PathBuf;

let mut rate_limit = RateLimitConfig::default();
rate_limit.global_rpm = 500;

let config = HttpRuntimeConfigBuilder::new()
    .rate_limit(rate_limit)
    .request_timeout_secs(45)
    .max_body_size(10 * 1024 * 1024)  // 10MB
    .enable_cors(false)
    .enable_openapi(false)
    .security_config_path(PathBuf::from("/app/config/security.toml"))
    .build()?;

let runtime = HttpAgentRuntime::with_config(registry, config);
```

### Secrets Management

**Kubernetes Secrets**:

```bash
# Create secret for JWT key
kubectl create secret generic skreaver-secrets \
    --from-literal=jwt-secret=$(openssl rand -base64 32) \
    --namespace skreaver

# Create secret for database credentials
kubectl create secret generic skreaver-db \
    --from-literal=username=skreaver_user \
    --from-literal=password=$(openssl rand -base64 32) \
    --from-literal=database=skreaver \
    --namespace skreaver
```

**Using External Secret Managers**:

```yaml
# Example: Using External Secrets Operator with AWS Secrets Manager
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: skreaver-secrets
spec:
  secretStoreRef:
    name: aws-secrets-manager
    kind: SecretStore
  target:
    name: skreaver-secrets
  data:
    - secretKey: jwt-secret
      remoteRef:
        key: skreaver/prod/jwt-secret
    - secretKey: db-password
      remoteRef:
        key: skreaver/prod/db-password
```

---

## Kubernetes Deployment

### Namespace Setup

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: skreaver
  labels:
    name: skreaver
    environment: production
---
apiVersion: v1
kind: ResourceQuota
metadata:
  name: skreaver-quota
  namespace: skreaver
spec:
  hard:
    requests.cpu: "8"
    requests.memory: 16Gi
    limits.cpu: "16"
    limits.memory: 32Gi
    persistentvolumeclaims: "10"
---
apiVersion: v1
kind: LimitRange
metadata:
  name: skreaver-limits
  namespace: skreaver
spec:
  limits:
    - max:
        cpu: "4"
        memory: 4Gi
      min:
        cpu: "100m"
        memory: 128Mi
      type: Container
```

### ConfigMap for Security Policy

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: skreaver-security-config
  namespace: skreaver
data:
  security.toml: |
    [metadata]
    version = "1.0"
    description = "Production security policy"

    [fs]
    allow_paths = ["/app/data", "/tmp/skreaver"]
    deny_patterns = ["**/.env", "**/.git/**"]
    max_file_size_mb = 100
    allow_symlinks = false

    [http]
    allow_domains = ["api.example.com"]
    max_request_size_mb = 50
    timeout_seconds = 30

    [resources]
    max_memory_mb = 1024
    max_cpu_percent = 80.0

    [audit]
    enabled = true
    log_level = "info"
    retention_days = 90
```

### Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: skreaver-http
  namespace: skreaver
  labels:
    app: skreaver-http
    version: v0.5.0
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: skreaver-http
  template:
    metadata:
      labels:
        app: skreaver-http
        version: v0.5.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8080"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: skreaver-http
      # Graceful shutdown configuration
      # Allows time for in-flight requests to complete before forceful termination
      terminationGracePeriodSeconds: 30
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
        seccompProfile:
          type: RuntimeDefault

      containers:
        - name: skreaver-http
          image: myregistry/skreaver-http:v0.5.0
          imagePullPolicy: IfNotPresent

          ports:
            - name: http
              containerPort: 8080
              protocol: TCP

          env:
            - name: SKREAVER_SECURITY_CONFIG
              value: /config/security.toml
            - name: SKREAVER_JWT_SECRET
              valueFrom:
                secretKeyRef:
                  name: skreaver-secrets
                  key: jwt-secret
            - name: RUST_LOG
              value: info,skreaver_http=debug
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: skreaver-db
                  key: connection-string
                  optional: true

          volumeMounts:
            - name: config
              mountPath: /config
              readOnly: true
            - name: data
              mountPath: /app/data
            - name: tmp
              mountPath: /tmp/skreaver

          resources:
            requests:
              cpu: 500m
              memory: 512Mi
            limits:
              cpu: 2000m
              memory: 2Gi

          livenessProbe:
            httpGet:
              path: /health
              port: http
            initialDelaySeconds: 10
            periodSeconds: 30
            timeoutSeconds: 5
            failureThreshold: 3

          readinessProbe:
            httpGet:
              path: /ready
              port: http
            initialDelaySeconds: 5
            periodSeconds: 10
            timeoutSeconds: 3
            failureThreshold: 2

          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            runAsNonRoot: true
            runAsUser: 1000
            capabilities:
              drop:
                - ALL

      volumes:
        - name: config
          configMap:
            name: skreaver-security-config
        - name: data
          persistentVolumeClaim:
            claimName: skreaver-data
        - name: tmp
          emptyDir:
            sizeLimit: 1Gi

      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
            - weight: 100
              podAffinityTerm:
                labelSelector:
                  matchLabels:
                    app: skreaver-http
                topologyKey: kubernetes.io/hostname
```

### Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: skreaver-http
  namespace: skreaver
  labels:
    app: skreaver-http
  annotations:
    prometheus.io/scrape: "true"
    prometheus.io/port: "8080"
spec:
  type: ClusterIP
  selector:
    app: skreaver-http
  ports:
    - name: http
      port: 80
      targetPort: http
      protocol: TCP
  sessionAffinity: ClientIP
  sessionAffinityConfig:
    clientIP:
      timeoutSeconds: 3600
```

### Ingress

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: skreaver-http
  namespace: skreaver
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/rate-limit: "100"
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/force-ssl-redirect: "true"
    nginx.ingress.kubernetes.io/proxy-body-size: "50m"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "300"
spec:
  ingressClassName: nginx
  tls:
    - hosts:
        - skreaver.example.com
      secretName: skreaver-tls
  rules:
    - host: skreaver.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: skreaver-http
                port:
                  name: http
```

### PersistentVolumeClaim

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: skreaver-data
  namespace: skreaver
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: fast-ssd
  resources:
    requests:
      storage: 10Gi
```

### ServiceAccount and RBAC

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: skreaver-http
  namespace: skreaver
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: skreaver-http
  namespace: skreaver
rules:
  - apiGroups: [""]
    resources: ["configmaps"]
    verbs: ["get", "list", "watch"]
  - apiGroups: [""]
    resources: ["secrets"]
    verbs: ["get"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: skreaver-http
  namespace: skreaver
subjects:
  - kind: ServiceAccount
    name: skreaver-http
    namespace: skreaver
roleRef:
  kind: Role
  name: skreaver-http
  apiGroup: rbac.authorization.k8s.io
```

### HorizontalPodAutoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: skreaver-http
  namespace: skreaver
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: skreaver-http
  minReplicas: 3
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 80
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
        - type: Percent
          value: 50
          periodSeconds: 60
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
        - type: Percent
          value: 100
          periodSeconds: 30
```

### PodDisruptionBudget

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: skreaver-http
  namespace: skreaver
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: skreaver-http
```

---

## Security Hardening

### Security Hardening Checklist

#### Container Security

- [ ] **Run as non-root user** (UID 1000)
- [ ] **Read-only root filesystem** enabled
- [ ] **Drop all capabilities**
- [ ] **No privilege escalation** allowed
- [ ] **Seccomp profile** set to RuntimeDefault
- [ ] **AppArmor/SELinux** profiles configured
- [ ] **Image scanning** in CI/CD pipeline
- [ ] **Signed images** with Cosign/Notary
- [ ] **Minimal base image** (distroless/Alpine)
- [ ] **No secrets in image layers**

#### Network Security

- [ ] **TLS/HTTPS** enforced for all external traffic
- [ ] **mTLS** for service-to-service communication
- [ ] **Network policies** restrict pod communication
- [ ] **Ingress rate limiting** configured
- [ ] **DDoS protection** via CDN/WAF
- [ ] **Private subnets** for databases
- [ ] **Egress filtering** for outbound traffic
- [ ] **VPC/network isolation** between environments

#### Authentication & Authorization

- [ ] **JWT tokens** with strong secrets (32+ bytes)
- [ ] **API key rotation** every 90 days
- [ ] **RBAC policies** enforcing least privilege
- [ ] **Service accounts** with minimal permissions
- [ ] **Multi-factor authentication** for admin access
- [ ] **OAuth2/OIDC** integration for user auth
- [ ] **IP whitelisting** for admin endpoints
- [ ] **Audit logging** for all auth events

#### Data Security

- [ ] **Encryption at rest** for PVCs
- [ ] **Encryption in transit** (TLS 1.3)
- [ ] **Database credentials** from secret manager
- [ ] **Sensitive data redaction** in logs
- [ ] **PII handling** compliance (GDPR/CCPA)
- [ ] **Backup encryption** enabled
- [ ] **Secret rotation** automated
- [ ] **Data retention policies** enforced

#### Application Security

- [ ] **Security policy file** comprehensive
- [ ] **Filesystem access** restricted
- [ ] **HTTP domains** whitelisted
- [ ] **Resource limits** enforced
- [ ] **Input validation** enabled
- [ ] **Output sanitization** for errors
- [ ] **Rate limiting** configured
- [ ] **Request size limits** set
- [ ] **Timeout policies** applied
- [ ] **Dependency scanning** automated

#### Monitoring & Incident Response

- [ ] **Security alerts** configured
- [ ] **Audit logs** centralized
- [ ] **Intrusion detection** active
- [ ] **Vulnerability scanning** scheduled
- [ ] **Incident response plan** documented
- [ ] **On-call rotation** established
- [ ] **Security metrics** tracked
- [ ] **Compliance reporting** automated

### Network Policies

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: skreaver-http-netpol
  namespace: skreaver
spec:
  podSelector:
    matchLabels:
      app: skreaver-http
  policyTypes:
    - Ingress
    - Egress
  ingress:
    # Allow from ingress controller
    - from:
        - namespaceSelector:
            matchLabels:
              name: ingress-nginx
        - podSelector:
            matchLabels:
              app: nginx-ingress
      ports:
        - protocol: TCP
          port: 8080
    # Allow from Prometheus
    - from:
        - namespaceSelector:
            matchLabels:
              name: monitoring
        - podSelector:
            matchLabels:
              app: prometheus
      ports:
        - protocol: TCP
          port: 8080
  egress:
    # Allow DNS
    - to:
        - namespaceSelector:
            matchLabels:
              name: kube-system
        - podSelector:
            matchLabels:
              k8s-app: kube-dns
      ports:
        - protocol: UDP
          port: 53
    # Allow database
    - to:
        - podSelector:
            matchLabels:
              app: postgres
      ports:
        - protocol: TCP
          port: 5432
    # Allow HTTPS egress
    - to:
        - namespaceSelector: {}
      ports:
        - protocol: TCP
          port: 443
```

### Pod Security Standards

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: skreaver
  labels:
    pod-security.kubernetes.io/enforce: restricted
    pod-security.kubernetes.io/audit: restricted
    pod-security.kubernetes.io/warn: restricted
```

---

## Monitoring and Observability

### Prometheus ServiceMonitor

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: skreaver-http
  namespace: skreaver
  labels:
    app: skreaver-http
spec:
  selector:
    matchLabels:
      app: skreaver-http
  endpoints:
    - port: http
      path: /metrics
      interval: 30s
      scrapeTimeout: 10s
```

### PrometheusRule for Alerts

```yaml
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: skreaver-http-alerts
  namespace: skreaver
spec:
  groups:
    - name: skreaver-http
      interval: 30s
      rules:
        - alert: SkreaverHighErrorRate
          expr: |
            rate(http_requests_total{status=~"5.."}[5m])
            / rate(http_requests_total[5m]) > 0.05
          for: 5m
          labels:
            severity: critical
          annotations:
            summary: "High error rate detected"
            description: "Error rate is {{ $value | humanizePercentage }}"

        - alert: SkreaverHighMemoryUsage
          expr: |
            container_memory_usage_bytes{pod=~"skreaver-http-.*"}
            / container_spec_memory_limit_bytes > 0.9
          for: 5m
          labels:
            severity: warning
          annotations:
            summary: "High memory usage"
            description: "Memory usage is {{ $value | humanizePercentage }}"

        - alert: SkreaverPodNotReady
          expr: |
            kube_pod_status_ready{pod=~"skreaver-http-.*", condition="true"} == 0
          for: 5m
          labels:
            severity: critical
          annotations:
            summary: "Pod not ready"
            description: "Pod {{ $labels.pod }} is not ready"

        - alert: SkreaverSlowResponseTime
          expr: |
            histogram_quantile(0.95,
              rate(http_request_duration_seconds_bucket[5m])
            ) > 1
          for: 10m
          labels:
            severity: warning
          annotations:
            summary: "Slow response times"
            description: "P95 latency is {{ $value }}s"
```

### Grafana Dashboard

See [GRAFANA_DASHBOARD.json](./docs/grafana-dashboard.json) for a comprehensive monitoring dashboard including:

- Request rate and latency
- Error rates by endpoint
- Memory and CPU usage
- WebSocket connections (if enabled)
- Security events
- Health check status

### Logging Configuration

**Structured Logging with FluentBit**:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: fluent-bit-config
  namespace: skreaver
data:
  fluent-bit.conf: |
    [SERVICE]
        Flush         5
        Log_Level     info

    [INPUT]
        Name              tail
        Path              /var/log/containers/skreaver-http*.log
        Parser            docker
        Tag               kube.*
        Refresh_Interval  5

    [FILTER]
        Name                kubernetes
        Match               kube.*
        Kube_URL            https://kubernetes.default.svc:443
        Merge_Log           On
        Keep_Log            Off

    [OUTPUT]
        Name   es
        Match  *
        Host   elasticsearch
        Port   9200
        Index  skreaver-logs
        Type   _doc
```

### Distributed Tracing

**OpenTelemetry Collector**:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: otel-collector-config
  namespace: skreaver
data:
  otel-collector-config.yaml: |
    receivers:
      otlp:
        protocols:
          grpc:
            endpoint: 0.0.0.0:4317

    processors:
      batch:
        timeout: 10s

      attributes:
        actions:
          - key: environment
            value: production
            action: insert

    exporters:
      jaeger:
        endpoint: jaeger:14250
        tls:
          insecure: true

      prometheus:
        endpoint: "0.0.0.0:8889"

    service:
      pipelines:
        traces:
          receivers: [otlp]
          processors: [batch, attributes]
          exporters: [jaeger]
        metrics:
          receivers: [otlp]
          processors: [batch]
          exporters: [prometheus]
```

---

## Backup and Disaster Recovery

### Database Backup

**Automated Backup CronJob**:

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: skreaver-db-backup
  namespace: skreaver
spec:
  schedule: "0 2 * * *"  # Daily at 2 AM
  concurrencyPolicy: Forbid
  successfulJobsHistoryLimit: 3
  failedJobsHistoryLimit: 1
  jobTemplate:
    spec:
      template:
        spec:
          restartPolicy: OnFailure
          containers:
            - name: backup
              image: postgres:15-alpine
              env:
                - name: PGPASSWORD
                  valueFrom:
                    secretKeyRef:
                      name: skreaver-db
                      key: password
              command:
                - /bin/sh
                - -c
                - |
                  pg_dump -h postgres -U skreaver_user skreaver | \
                  gzip > /backup/skreaver-$(date +%Y%m%d-%H%M%S).sql.gz

                  # Retain last 30 days
                  find /backup -name "skreaver-*.sql.gz" -mtime +30 -delete
              volumeMounts:
                - name: backup
                  mountPath: /backup
          volumes:
            - name: backup
              persistentVolumeClaim:
                claimName: skreaver-backup
```

### Configuration Backup

```bash
# Backup all ConfigMaps and Secrets
kubectl get configmap -n skreaver -o yaml > skreaver-configmaps-backup.yaml
kubectl get secret -n skreaver -o yaml > skreaver-secrets-backup.yaml

# Backup via Velero
velero backup create skreaver-backup \
  --include-namespaces skreaver \
  --ttl 720h
```

### Disaster Recovery Plan

**RTO (Recovery Time Objective)**: 1 hour
**RPO (Recovery Point Objective)**: 24 hours

**Recovery Steps**:

1. **Deploy from backup**:
   ```bash
   # Restore namespace
   kubectl apply -f skreaver-namespace.yaml

   # Restore configs and secrets
   kubectl apply -f skreaver-configmaps-backup.yaml
   kubectl apply -f skreaver-secrets-backup.yaml

   # Restore database from backup
   gunzip < backup.sql.gz | psql -h postgres -U skreaver_user skreaver

   # Deploy application
   kubectl apply -f skreaver-deployment.yaml
   ```

2. **Verify health**:
   ```bash
   # Check pods
   kubectl get pods -n skreaver

   # Check health endpoints
   kubectl port-forward -n skreaver svc/skreaver-http 8080:80
   curl http://localhost:8080/health
   curl http://localhost:8080/ready
   ```

3. **Restore traffic**:
   ```bash
   # Update DNS/ingress
   kubectl apply -f skreaver-ingress.yaml
   ```

---

## Graceful Shutdown

**Status**: ✅ **Implemented** (v0.5.0)

Skreaver now supports graceful shutdown for production Kubernetes deployments. This is **critical** for zero-downtime rolling updates.

### What It Does

When Kubernetes sends SIGTERM to terminate a pod:
1. ✅ Server stops accepting new connections
2. ✅ Existing in-flight requests complete
3. ✅ Database transactions finish
4. ✅ Tool executions complete
5. ✅ Server shuts down cleanly

**Result**: Zero request failures during rolling updates, scale-down, or pod evictions.

### Implementation

All examples in this guide include graceful shutdown:

```rust
use skreaver::runtime::{HttpAgentRuntime, shutdown_signal};

axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal())  // ← This line is CRITICAL
    .await?;
```

### Kubernetes Configuration

```yaml
spec:
  template:
    spec:
      # Allow 30 seconds for graceful shutdown before SIGKILL
      terminationGracePeriodSeconds: 30
```

**Recommended Values**:
- **Standard web services**: 30 seconds (default)
- **Long-running operations**: 60-90 seconds
- **Batch processing**: 120+ seconds

### Verification

Test graceful shutdown locally:

```bash
# Start server
cargo run --example http_server

# Send SIGTERM (simulates Kubernetes pod termination)
kill -TERM <pid>

# Expected output:
# Received SIGTERM, initiating graceful shutdown
# Shutdown signal processed, Axum will now drain connections
# ✅ Server shutdown complete
```

Test in Kubernetes:

```bash
# Trigger rolling update
kubectl set image deployment/skreaver-http skreaver-http=myregistry/skreaver:v2

# Watch pods terminate gracefully
kubectl logs -f <old-pod-name>

# Expected: No 502/503 errors during rollout
```

### Advanced Usage

**With custom timeout**:
```rust
use skreaver::runtime::shutdown_signal_with_timeout;
use std::time::Duration;

axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal_with_timeout(Duration::from_secs(60)))
    .await?;
```

**With cleanup tasks**:
```rust
use skreaver::runtime::shutdown_with_cleanup;

let cleanup = || async {
    println!("Flushing metrics...");
    metrics_client.flush().await;
    println!("Closing database connections...");
    db_pool.close().await;
};

axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_with_cleanup(cleanup))
    .await?;
```

### Documentation

See [GRACEFUL_SHUTDOWN_IMPLEMENTATION.md](GRACEFUL_SHUTDOWN_IMPLEMENTATION.md) for:
- Complete implementation details
- Performance impact analysis
- Kubernetes best practices
- Testing procedures

---

## Troubleshooting

### Common Issues

#### Pod Crashes with OOMKilled

**Symptoms**: Pods restart frequently with OOMKilled status

**Solution**:
```bash
# Increase memory limits
kubectl patch deployment skreaver-http -n skreaver --patch '
spec:
  template:
    spec:
      containers:
      - name: skreaver-http
        resources:
          limits:
            memory: 4Gi
'
```

#### High Response Latency

**Symptoms**: P95 latency > 1 second

**Debugging**:
```bash
# Check resource utilization
kubectl top pods -n skreaver

# Check database connections
kubectl exec -it <pod> -n skreaver -- psql -c "SELECT count(*) FROM pg_stat_activity;"

# Enable debug logging
kubectl set env deployment/skreaver-http -n skreaver RUST_LOG=debug
```

#### Security Config Issues

**Symptoms**: `/ready` endpoint returns 503 with degraded security

**Solution**:
```bash
# Check security config
kubectl exec -it <pod> -n skreaver -- cat /config/security.toml

# Validate configuration
kubectl logs <pod> -n skreaver | grep -i security

# Update ConfigMap
kubectl edit configmap skreaver-security-config -n skreaver
```

#### WebSocket Connection Failures

**Symptoms**: WebSocket clients can't connect

**Solution**:
```bash
# Check ingress annotations
kubectl get ingress skreaver-http -n skreaver -o yaml | grep websocket

# Add WebSocket support to ingress
kubectl annotate ingress skreaver-http -n skreaver \
  nginx.ingress.kubernetes.io/websocket-services=skreaver-http
```

### Debug Commands

```bash
# Get pod logs
kubectl logs -f <pod> -n skreaver

# Get previous logs (after crash)
kubectl logs <pod> -n skreaver --previous

# Exec into pod
kubectl exec -it <pod> -n skreaver -- /bin/sh

# Port forward for local testing
kubectl port-forward -n skreaver svc/skreaver-http 8080:80

# Check events
kubectl get events -n skreaver --sort-by='.lastTimestamp'

# Describe pod for detailed status
kubectl describe pod <pod> -n skreaver
```

---

## Production Checklist

Before going to production, verify:

### Pre-Deployment

- [ ] Security policy file reviewed and approved
- [ ] All secrets rotated and stored in secret manager
- [ ] TLS certificates configured and valid
- [ ] Resource limits appropriate for expected load
- [ ] Health checks configured and tested
- [ ] Monitoring and alerts configured
- [ ] Backup and restore procedures tested
- [ ] Incident response plan documented
- [ ] Load testing completed
- [ ] Security scanning passed

### Post-Deployment

- [ ] Health endpoints returning 200
- [ ] Metrics visible in Prometheus
- [ ] Logs flowing to centralized logging
- [ ] Alerts firing correctly (test)
- [ ] Backup jobs running successfully
- [ ] SSL/TLS certificates valid
- [ ] DNS resolution working
- [ ] Load balancer health checks passing
- [ ] Autoscaling tested
- [ ] Disaster recovery plan verified

---

## Additional Resources

- [Health Check Documentation](HEALTH_CHECKS.md)
- [Security Configuration Guide](crates/skreaver-core/src/security/README.md)
- [WebSocket Security](crates/skreaver-http/src/websocket/README.md)
- [Kubernetes Best Practices](https://kubernetes.io/docs/concepts/configuration/overview/)
- [OWASP Security Guidelines](https://owasp.org/www-project-kubernetes-top-ten/)

---

## Support

For production support:
- **Security Issues**: security@example.com
- **Operations**: ops@example.com
- **On-call**: oncall@example.com
- **Documentation**: https://docs.skreaver.example.com
