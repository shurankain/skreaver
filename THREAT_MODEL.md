# Skreaver Threat Model

> **Security Model Version**: 0.1.0  
> **Created**: 2025-09-08  
> **Status**: Active Implementation  
> **Scope**: Core platform security boundaries and controls  

## Executive Summary

This document defines the security threat model for Skreaver, a Rust-native AI agent coordination runtime. It identifies security boundaries, potential threats, and mitigation strategies to enable secure deployment in production environments.

## System Architecture & Trust Boundaries

### Core Components
- **Agent Runtime**: Executes user-defined agent logic
- **Tool System**: Provides capabilities (HTTP, File I/O, Network)
- **Memory Backends**: Stores agent state and data
- **HTTP Runtime**: Exposes REST API endpoints
- **Configuration System**: Manages security policies

### Trust Boundaries
```
┌─────────────────────────────────────────────────────────────┐
│                    UNTRUSTED ZONE                           │
├─────────────────────────────────────────────────────────────┤
│ • User Input (HTTP requests, tool inputs)                  │
│ • External Network Resources                               │
│ • File System Paths                                       │
│ • Environment Variables                                    │
└─────────────────┬───────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│                 SECURITY BOUNDARY                           │
│  • Input validation & sanitization                         │
│  • Resource quotas & limits                               │
│  • Access control policies                                │
└─────────────────┬───────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│                   TRUSTED ZONE                             │
├─────────────────────────────────────────────────────────────┤
│ • Skreaver Core Runtime                                    │
│ • Memory Backends                                          │
│ • Tool Registry                                           │
│ • Agent Execution Context                                  │
└─────────────────────────────────────────────────────────────┘
```

## Threat Model Matrix

| **Asset** | **Threat** | **Impact** | **Likelihood** | **Control** | **Risk Level** |
|-----------|------------|------------|----------------|-------------|----------------|
| **File System** | Path traversal, data exfiltration | High | Medium | Path allowlists, size limits | **High** |
| **HTTP Requests** | SSRF, data leakage, credential theft | High | High | Domain allowlists, response limits | **Critical** |
| **Memory/CPU** | DoS, resource exhaustion | Medium | High | Resource quotas, timeouts | **Medium** |
| **Secrets** | Credential leakage, privilege escalation | Critical | Medium | Environment-only, audit logs | **High** |
| **Agent State** | Data corruption, unauthorized access | High | Low | Transaction isolation, access control | **Medium** |
| **Network** | Lateral movement, data exfiltration | High | Low | Network policies, port restrictions | **Medium** |

## Threat Scenarios

### T1: Path Traversal Attack
**Description**: Malicious input attempts to access files outside allowed directories
```bash
# Attack vector
curl -X POST /agents/file-agent/observe \
  -d '{"input": "read_file ../../../etc/passwd"}'
```
**Impact**: Information disclosure, system compromise
**Mitigation**: Path canonicalization, allowlist validation

### T2: Server-Side Request Forgery (SSRF)
**Description**: Agent makes HTTP requests to internal/restricted endpoints
```bash
# Attack vector  
curl -X POST /agents/http-agent/observe \
  -d '{"input": "http_get http://localhost:22/ssh-keys"}'
```
**Impact**: Internal network scanning, credential theft
**Mitigation**: Domain allowlists, IP address blocking

### T3: Resource Exhaustion
**Description**: Malicious input causes excessive resource consumption
```bash
# Attack vector
curl -X POST /agents/memory-agent/observe \
  -d '{"input": "create_large_object 10GB"}'
```
**Impact**: Service denial, system instability
**Mitigation**: Memory/CPU quotas, timeout enforcement

### T4: Secret Leakage
**Description**: Sensitive data exposed through logs or responses
```bash
# Vulnerable code
log::info!("Processing request: {}", user_input); // May log API keys
```
**Impact**: Credential compromise, unauthorized access
**Mitigation**: Secret redaction, secure logging practices

### T5: Tool Chain Injection
**Description**: Malicious tool calls executed with elevated privileges
```bash
# Attack vector
curl -X POST /agents/shell-agent/observe \
  -d '{"input": "execute rm -rf /"}'
```
**Impact**: System compromise, data destruction  
**Mitigation**: Tool sandboxing, capability restrictions

## Security Controls Implementation

### 1. Input Validation & Sanitization
```rust
pub trait SecureInput {
    fn validate(&self) -> Result<(), SecurityError>;
    fn sanitize(self) -> Self;
}

pub struct PathValidator {
    allowed_paths: Vec<PathBuf>,
    deny_patterns: Vec<Regex>,
}

impl PathValidator {
    pub fn validate_path(&self, path: &Path) -> Result<PathBuf, SecurityError> {
        let canonical = path.canonicalize()
            .map_err(|_| SecurityError::InvalidPath)?;
        
        // Check allowlist
        if !self.allowed_paths.iter().any(|p| canonical.starts_with(p)) {
            return Err(SecurityError::PathNotAllowed);
        }
        
        // Check deny patterns
        let path_str = canonical.to_string_lossy();
        if self.deny_patterns.iter().any(|p| p.is_match(&path_str)) {
            return Err(SecurityError::PathDenied);
        }
        
        Ok(canonical)
    }
}
```

### 2. Resource Quotas & Limits
```rust
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f64,
    pub max_execution_time: Duration,
    pub max_file_size_bytes: u64,
    pub max_network_connections: u32,
}

pub struct ResourceTracker {
    limits: ResourceLimits,
    current_usage: ResourceUsage,
}

impl ResourceTracker {
    pub fn check_memory_limit(&self, requested: u64) -> Result<(), SecurityError> {
        if self.current_usage.memory_mb + requested > self.limits.max_memory_mb {
            return Err(SecurityError::MemoryLimitExceeded);
        }
        Ok(())
    }
    
    pub fn enforce_timeout<F, T>(&self, operation: F) -> Result<T, SecurityError> 
    where F: Future<Output = T> + Send + 'static,
          T: Send + 'static,
    {
        tokio::time::timeout(self.limits.max_execution_time, operation)
            .await
            .map_err(|_| SecurityError::TimeoutExceeded)
    }
}
```

### 3. Access Control & Sandboxing
```rust
pub struct SecurityPolicy {
    pub fs_policy: FileSystemPolicy,
    pub network_policy: NetworkPolicy,
    pub resource_policy: ResourcePolicy,
}

pub struct FileSystemPolicy {
    pub enabled: bool,
    pub allow_paths: Vec<PathBuf>,
    pub deny_patterns: Vec<String>,
    pub max_file_size_bytes: u64,
    pub follow_symlinks: bool,
}

pub struct NetworkPolicy {
    pub enabled: bool,
    pub allow_domains: Vec<String>,
    pub allow_methods: Vec<String>,
    pub timeout_seconds: u64,
    pub max_response_bytes: u64,
    pub allow_local: bool,
}

pub trait SecureTool: Tool {
    fn security_policy(&self) -> &SecurityPolicy;
    fn validate_input(&self, input: &str) -> Result<(), SecurityError>;
    fn execute_secure(&self, input: String) -> Result<String, SecurityError>;
}
```

## Security Configuration

### Default Security Policy (`skreaver-security.toml`)
```toml
[fs]
enabled = true
allow_paths = ["/var/app/data", "./runtime/tmp"]
deny_patterns = ["..", "/etc", "/proc", "/sys", "*.ssh", "*.key"]
max_file_size_bytes = 16_777_216  # 16MB
max_files_per_operation = 100
follow_symlinks = false

[http]
enabled = true
allow_domains = ["api.internal.local", "*.example.org"]
deny_domains = ["localhost", "127.0.0.1", "169.254.169.254"]
allow_methods = ["GET", "POST", "PUT"]
timeout_seconds = 30
max_response_bytes = 33_554_432  # 32MB
max_redirects = 3
user_agent = "skreaver-agent/0.1.0"
allow_local = false

[network]
enabled = false  # Requires explicit opt-in
allow_ports = []
deny_ports = [22, 23, 3389, 5432, 6379]  # SSH, Telnet, RDP, PostgreSQL, Redis
ttl_seconds = 300

[resources]
max_memory_mb = 128
max_cpu_percent = 50
max_execution_time_seconds = 300
max_concurrent_operations = 10
max_open_files = 100

[audit]
log_all_operations = true
redact_secrets = true
retain_logs_days = 90
log_level = "INFO"
```

## Audit & Monitoring

### Security Event Logging
```rust
pub struct SecurityAuditLog {
    operation: String,
    agent_id: String,
    tool_name: String,
    input_hash: String,  // SHA-256 of input (redacted)
    result: SecurityResult,
    timestamp: DateTime<Utc>,
    session_id: Uuid,
}

pub enum SecurityResult {
    Allowed,
    Denied { reason: String },
    LimitExceeded { limit_type: String, requested: u64, limit: u64 },
}

impl SecurityAuditLog {
    pub fn log_access_attempt(&self) {
        match &self.result {
            SecurityResult::Denied { reason } => {
                tracing::warn!(
                    agent_id = %self.agent_id,
                    tool_name = %self.tool_name,
                    reason = %reason,
                    session_id = %self.session_id,
                    "Security policy violation"
                );
            }
            SecurityResult::LimitExceeded { limit_type, requested, limit } => {
                tracing::error!(
                    agent_id = %self.agent_id,
                    tool_name = %self.tool_name,
                    limit_type = %limit_type,
                    requested = %requested,
                    limit = %limit,
                    session_id = %self.session_id,
                    "Resource limit exceeded"
                );
            }
            SecurityResult::Allowed => {
                tracing::info!(
                    agent_id = %self.agent_id,
                    tool_name = %self.tool_name,
                    session_id = %self.session_id,
                    "Operation authorized"
                );
            }
        }
    }
}
```

### Metrics & Alerting
```rust
// Security-specific metrics
pub struct SecurityMetrics {
    pub access_denied_total: Counter,
    pub resource_limit_exceeded_total: Counter,
    pub security_violations_by_type: CounterVec,
    pub authentication_failures: Counter,
    pub suspicious_activity_score: Gauge,
}

static SECURITY_METRICS: Lazy<SecurityMetrics> = Lazy::new(|| {
    SecurityMetrics {
        access_denied_total: register_counter!(
            "skreaver_security_access_denied_total",
            "Total number of access denied events"
        ).unwrap(),
        resource_limit_exceeded_total: register_counter!(
            "skreaver_security_resource_limit_exceeded_total", 
            "Total number of resource limit violations"
        ).unwrap(),
        security_violations_by_type: register_counter_vec!(
            "skreaver_security_violations_total",
            "Security violations by type",
            &["violation_type", "tool_name"]
        ).unwrap(),
        authentication_failures: register_counter!(
            "skreaver_security_auth_failures_total",
            "Total authentication failures"
        ).unwrap(),
        suspicious_activity_score: register_gauge!(
            "skreaver_security_suspicious_activity_score",
            "Suspicious activity risk score (0-100)"
        ).unwrap(),
    }
});
```

## Security Testing Strategy

### Automated Security Tests
1. **Path Traversal Tests**: Verify path validation blocks malicious inputs
2. **SSRF Protection Tests**: Confirm HTTP client blocks internal endpoints  
3. **Resource Limit Tests**: Validate memory/CPU/timeout enforcement
4. **Input Validation Tests**: Test sanitization of malicious payloads
5. **Authentication Tests**: Verify JWT/API key validation
6. **Authorization Tests**: Confirm role-based access controls

### Penetration Testing Scenarios
1. **File System Attacks**: Attempt to read sensitive files
2. **Network Attacks**: Try to access internal services
3. **DoS Attacks**: Resource exhaustion attempts
4. **Injection Attacks**: Code/command injection via tool inputs
5. **Privilege Escalation**: Attempt to bypass security controls

## Incident Response

### Security Incident Classification
- **P0 Critical**: Active exploitation, system compromise
- **P1 High**: Privilege escalation, data exfiltration  
- **P2 Medium**: Policy violations, suspicious activity
- **P3 Low**: Security warnings, configuration issues

### Response Procedures
1. **Detection**: Automated alerts from security metrics
2. **Containment**: Disable affected agents/tools
3. **Investigation**: Analyze audit logs and traces
4. **Remediation**: Apply fixes and update policies
5. **Recovery**: Restore service with enhanced monitoring
6. **Lessons Learned**: Update threat model and controls

## Implementation Roadmap

### Phase 1: Core Security (Week 1-2)
- [ ] Security policy configuration system
- [ ] Input validation framework
- [ ] Resource limits enforcement
- [ ] Basic audit logging

### Phase 2: Advanced Controls (Week 3-4)  
- [ ] Tool sandboxing implementation
- [ ] Network policy enforcement
- [ ] Security metrics and alerting
- [ ] Automated security testing

### Phase 3: Production Hardening (Week 5-6)
- [ ] Penetration testing
- [ ] Security documentation
- [ ] Incident response procedures
- [ ] Security review and audit

---

*This threat model is a living document that will be updated as new threats are identified and security controls evolve.*