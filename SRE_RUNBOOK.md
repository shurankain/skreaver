# Skreaver SRE Runbook

**Version**: 0.5.0
**Last Updated**: 2025-10-27
**Audience**: Site Reliability Engineers, DevOps, On-Call Engineers

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Monitoring & Alerts](#monitoring--alerts)
4. [Common Issues](#common-issues)
5. [Troubleshooting](#troubleshooting)
6. [Incident Response](#incident-response)
7. [Scaling](#scaling)
8. [Backup & Recovery](#backup--recovery)
9. [Performance Tuning](#performance-tuning)
10. [Security Incidents](#security-incidents)

---

## Overview

### System Description

Skreaver is a production-grade agent infrastructure platform written in Rust. It provides:
- HTTP API for agent management
- WebSocket support for real-time communication
- Redis-based agent mesh for distributed coordination
- PostgreSQL/SQLite for persistent storage
- Prometheus metrics for observability

### Key Components

```
┌─────────────────────────────────────────────────┐
│           Load Balancer (Nginx/AWS ALB)          │
└────────────────┬────────────────────────────────┘
                 │
    ┌────────────┴────────────┐
    │                         │
    ▼                         ▼
┌─────────┐             ┌─────────┐
│ Skreaver │◄───────────►│ Skreaver │
│ Instance │             │ Instance │
└────┬─────┘             └────┬─────┘
     │                        │
     └────────┬───────────────┘
              │
     ┌────────┴─────────┐
     │                  │
     ▼                  ▼
┌─────────┐       ┌──────────┐
│  Redis  │       │PostgreSQL│
│  Mesh   │       │          │
└─────────┘       └──────────┘
```

### Service Dependencies

| Service | Purpose | Impact if Down |
|---------|---------|----------------|
| **PostgreSQL** | Persistent storage | Cannot create new agents, existing agents continue |
| **Redis** | Mesh coordination | Agent communication fails, no distributed coordination |
| **Prometheus** | Metrics collection | No monitoring, system continues |
| **Load Balancer** | Traffic distribution | Service unavailable |

---

## Monitoring & Alerts

### Health Endpoints

```bash
# Basic health check
curl http://localhost:8080/health

# Readiness check (includes dependencies)
curl http://localhost:8080/ready

# Prometheus metrics
curl http://localhost:8080/metrics
```

### Key Metrics

#### Service Health
```promql
# HTTP request rate
rate(http_requests_total[5m])

# Error rate
rate(http_requests_total{status=~"5.."}[5m])

# Latency p99
histogram_quantile(0.99, http_request_duration_seconds_bucket)
```

#### Resource Usage
```promql
# CPU usage
skreaver_cpu_usage_percent

# Memory usage
skreaver_memory_usage_bytes / skreaver_memory_total_bytes * 100

# Disk usage
skreaver_disk_usage_bytes / skreaver_disk_total_bytes * 100
```

#### WebSocket
```promql
# Active connections
websocket_connections_active

# Failed connections
rate(websocket_connections_failed_total[5m])

# Message throughput
rate(websocket_messages_sent_total[5m])
```

#### Database
```promql
# Connection pool utilization
skreaver_db_connections_active / skreaver_db_connections_max

# Query latency
skreaver_db_query_duration_seconds
```

### Alert Thresholds

| Alert | Threshold | Severity | Action |
|-------|-----------|----------|--------|
| High Error Rate | > 5% of requests | **Critical** | Immediate investigation |
| High Latency | p99 > 1s | **Warning** | Check system load |
| Memory High | > 90% | **Warning** | Consider scaling |
| Memory Critical | > 95% | **Critical** | Scale immediately or restart |
| CPU High | > 80% sustained | **Warning** | Check for expensive operations |
| DB Connections Exhausted | > 95% pool used | **Critical** | Increase pool or fix leaks |
| WebSocket Failures | > 10% | **Warning** | Check network/auth |
| Disk Space Low | < 10% free | **Warning** | Clean up or expand |
| Service Down | Health check fails | **Critical** | Follow incident response |

---

## Common Issues

### 1. High Memory Usage

**Symptoms:**
- Memory usage > 90%
- Slow response times
- OOMKilled pods in Kubernetes

**Common Causes:**
- Memory leak in agent code
- Too many concurrent agents
- Large message payloads
- Connection leaks

**Diagnostic Steps:**
```bash
# Check memory usage
curl http://localhost:8080/metrics | grep skreaver_memory

# List active agents
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/agents

# Check WebSocket connections
curl http://localhost:8080/metrics | grep websocket_connections_active
```

**Resolution:**
```bash
# Option 1: Restart the service (if no state loss risk)
kubectl rollout restart deployment/skreaver

# Option 2: Scale horizontally
kubectl scale deployment/skreaver --replicas=5

# Option 3: Increase memory limits
kubectl edit deployment/skreaver
# Update resources.limits.memory

# Option 4: Enable memory profiling
export RUST_LOG=info,skreaver=debug
export SKREAVER_ENABLE_PROFILING=true
```

**Prevention:**
- Set appropriate memory limits
- Monitor memory metrics
- Implement agent lifecycle management
- Use connection pooling

---

### 2. Database Connection Pool Exhaustion

**Symptoms:**
- Requests timeout
- Errors: "connection pool timed out"
- DB metrics show pool at 100%

**Common Causes:**
- Too many concurrent requests
- Long-running transactions
- Connection leaks
- Insufficient pool size

**Diagnostic Steps:**
```bash
# Check pool status
curl http://localhost:8080/metrics | grep db_connections

# Check for long-running queries
SELECT pid, query, state, query_start
FROM pg_stat_activity
WHERE state != 'idle'
  AND query_start < NOW() - INTERVAL '1 minute';
```

**Resolution:**
```bash
# Option 1: Increase pool size (temporary)
export DATABASE_POOL_SIZE=50

# Option 2: Kill long-running queries
SELECT pg_terminate_backend(pid) FROM pg_stat_activity
WHERE state = 'active' AND query_start < NOW() - INTERVAL '5 minutes';

# Option 3: Restart application
kubectl rollout restart deployment/skreaver

# Option 4: Scale database (if bottleneck)
# Update database instance size or add read replicas
```

**Prevention:**
- Set reasonable pool sizes
- Configure connection timeouts
- Monitor connection metrics
- Use read replicas for read-heavy workloads

---

### 3. WebSocket Connection Failures

**Symptoms:**
- `websocket_connections_failed_total` increasing
- Clients can't connect
- 401/403 errors on /ws endpoint

**Common Causes:**
- Authentication failures
- Rate limiting
- Network issues
- Max connections reached

**Diagnostic Steps:**
```bash
# Check WebSocket metrics
curl http://localhost:8080/metrics | grep websocket

# Check auth logs
kubectl logs -l app=skreaver --tail=100 | grep "auth"

# Test WebSocket connection
websocat "ws://localhost:8080/ws?token=$TOKEN"
```

**Resolution:**
```bash
# Option 1: Verify authentication
# Check JWT secret is correctly configured
kubectl get secret skreaver-jwt -o yaml

# Option 2: Increase connection limits
kubectl edit configmap skreaver-config
# Update websocket.max_connections

# Option 3: Check rate limiting
# Review and adjust rate limit settings

# Option 4: Restart WebSocket manager
kubectl rollout restart deployment/skreaver
```

**Prevention:**
- Monitor auth failure rate
- Set appropriate connection limits
- Use connection pooling on client side
- Implement exponential backoff for reconnections

---

### 4. High CPU Usage

**Symptoms:**
- CPU usage > 80% sustained
- Slow request processing
- Increased latency

**Common Causes:**
- Inefficient agent logic
- Expensive tool operations
- High message volume
- JSON parsing overhead

**Diagnostic Steps:**
```bash
# Check CPU metrics
curl http://localhost:8080/metrics | grep cpu

# Profile the application
# Enable CPU profiling
export SKREAVER_ENABLE_PROFILING=true

# Check request rate
curl http://localhost:8080/metrics | grep http_requests_total
```

**Resolution:**
```bash
# Option 1: Scale horizontally
kubectl scale deployment/skreaver --replicas=5

# Option 2: Identify hot paths
# Use cargo flamegraph for profiling
kubectl exec -it skreaver-pod -- /bin/bash
# Install and run profiling tools

# Option 3: Optimize workload
# Review agent logic and tool usage
# Consider batching operations

# Option 4: Increase CPU limits
kubectl edit deployment/skreaver
# Update resources.limits.cpu
```

**Prevention:**
- Profile before deploying
- Set CPU requests and limits
- Monitor CPU metrics
- Optimize hot paths

---

### 5. Redis Connection Issues

**Symptoms:**
- Agent mesh communication fails
- Broadcast messages not delivered
- Redis connection errors in logs

**Common Causes:**
- Redis instance down
- Network connectivity
- Authentication failures
- Max connections reached

**Diagnostic Steps:**
```bash
# Test Redis connection
redis-cli -h $REDIS_HOST -p $REDIS_PORT ping

# Check Redis metrics
redis-cli -h $REDIS_HOST -p $REDIS_PORT info

# Check Skreaver logs
kubectl logs -l app=skreaver --tail=100 | grep redis
```

**Resolution:**
```bash
# Option 1: Restart Redis
kubectl rollout restart statefulset/redis

# Option 2: Check Redis credentials
kubectl get secret redis-secret -o yaml

# Option 3: Scale Redis (if needed)
# Update Redis replica count

# Option 4: Fallback to direct HTTP
# Disable mesh temporarily if needed
export SKREAVER_MESH_ENABLED=false
```

**Prevention:**
- Monitor Redis health
- Use Redis Sentinel or Cluster
- Set up Redis backups
- Configure connection pooling

---

## Troubleshooting

### Debug Mode

Enable debug logging:
```bash
export RUST_LOG=debug,skreaver=trace
kubectl set env deployment/skreaver RUST_LOG=debug,skreaver=trace
```

### Log Analysis

```bash
# Get recent logs
kubectl logs -l app=skreaver --tail=500

# Follow logs in real-time
kubectl logs -l app=skreaver -f

# Filter for errors
kubectl logs -l app=skreaver | grep -i error

# Filter for specific agent
kubectl logs -l app=skreaver | grep "agent_id=abc123"

# Check structured logs
kubectl logs -l app=skreaver --tail=100 | jq '.level, .message'
```

### Performance Profiling

```bash
# Enable profiling
export SKREAVER_ENABLE_PROFILING=true

# Generate flamegraph
cargo flamegraph --bin skreaver

# Check heap allocations
valgrind --tool=massif ./target/release/skreaver
```

### Network Debugging

```bash
# Test HTTP endpoint
curl -v http://localhost:8080/health

# Test WebSocket
websocat -v "ws://localhost:8080/ws?token=$TOKEN"

# Check port availability
netstat -tuln | grep 8080

# Test database connection
psql -h $DB_HOST -U $DB_USER -d $DB_NAME -c "SELECT 1"

# Test Redis connection
redis-cli -h $REDIS_HOST ping
```

---

## Incident Response

### Severity Levels

| Severity | Description | Response Time | Examples |
|----------|-------------|---------------|----------|
| **P0 - Critical** | Service down, data loss | 15 minutes | Complete outage, database corruption |
| **P1 - High** | Major degradation | 1 hour | High error rate, severe performance issues |
| **P2 - Medium** | Partial degradation | 4 hours | Single component failure, elevated latency |
| **P3 - Low** | Minor issues | Next business day | Non-critical warnings, cosmetic issues |

### P0 - Critical Outage

**Immediate Actions (First 15 minutes):**

1. **Acknowledge the incident**
   ```bash
   # Update status page
   # Post in incident channel
   ```

2. **Assess impact**
   ```bash
   # Check all health endpoints
   curl http://skreaver.prod/health
   curl http://skreaver.prod/ready

   # Check metrics
   curl http://skreaver.prod/metrics | grep up
   ```

3. **Quick triage**
   - Is the service responding?
   - Are dependencies up?
   - What changed recently?

4. **Immediate mitigation**
   ```bash
   # Option 1: Rollback recent deployment
   kubectl rollout undo deployment/skreaver

   # Option 2: Restart service
   kubectl rollout restart deployment/skreaver

   # Option 3: Scale up
   kubectl scale deployment/skreaver --replicas=10

   # Option 4: Failover to backup region
   # (if multi-region setup exists)
   ```

**Investigation (Next 30 minutes):**

1. **Gather evidence**
   ```bash
   # Export logs
   kubectl logs -l app=skreaver --since=1h > incident-logs.txt

   # Export metrics
   curl http://prometheus/api/v1/query_range?query=...

   # Check recent changes
   kubectl rollout history deployment/skreaver
   git log --since="1 hour ago"
   ```

2. **Root cause analysis**
   - Review error logs
   - Check system metrics
   - Investigate recent deployments
   - Review configuration changes

3. **Communicate status**
   - Update stakeholders every 30 minutes
   - Post updates on status page
   - Brief team on findings

**Resolution:**

1. **Fix the issue**
   ```bash
   # Deploy fix
   kubectl apply -f fix.yaml

   # Or rollback
   kubectl rollout undo deployment/skreaver
   ```

2. **Verify recovery**
   ```bash
   # Check health
   curl http://skreaver.prod/health

   # Monitor metrics
   watch -n 5 'curl http://skreaver.prod/metrics | grep http_requests'
   ```

3. **Post-incident**
   - Write incident report
   - Schedule post-mortem
   - Update runbooks
   - Implement preventive measures

---

## Scaling

### Horizontal Scaling

**Manual Scaling:**
```bash
# Scale up
kubectl scale deployment/skreaver --replicas=10

# Scale down
kubectl scale deployment/skreaver --replicas=3
```

**Auto-scaling:**
```yaml
# HPA is already configured in Helm chart
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: skreaver
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: skreaver
  minReplicas: 3
  maxReplicas: 20
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
```

**Scaling Triggers:**
- CPU > 70% for 5 minutes
- Memory > 80% for 5 minutes
- HTTP request rate exceeds capacity
- WebSocket connections approaching limit

### Vertical Scaling

**Increase resources:**
```yaml
resources:
  requests:
    memory: "1Gi"
    cpu: "1000m"
  limits:
    memory: "2Gi"
    cpu: "2000m"
```

**Database Scaling:**
```bash
# Increase connection pool
export DATABASE_POOL_SIZE=100

# Add read replicas
# Configure in PostgreSQL/Redis Helm charts

# Vertical scale database
# Increase instance size in cloud provider
```

---

## Backup & Recovery

### Database Backups

**Automated Backups:**
```bash
# PostgreSQL automated backups (configured in Helm)
# Default: Daily backups, 7-day retention

# Manual backup
kubectl exec -it postgresql-0 -- pg_dump -U skreaver skreaver > backup.sql

# Verify backup
ls -lh backup.sql
```

**Recovery:**
```bash
# Restore from backup
kubectl exec -it postgresql-0 -- psql -U skreaver skreaver < backup.sql

# Point-in-time recovery (if enabled)
# Use PostgreSQL PITR features
```

### Redis Backups

```bash
# Manual backup
kubectl exec -it redis-0 -- redis-cli SAVE

# Copy RDB file
kubectl cp redis-0:/data/dump.rdb ./redis-backup.rdb

# Restore
kubectl cp ./redis-backup.rdb redis-0:/data/dump.rdb
kubectl exec -it redis-0 -- redis-cli SHUTDOWN
# Redis will load from dump.rdb on restart
```

### Configuration Backups

```bash
# Export all configs
kubectl get configmap skreaver-config -o yaml > config-backup.yaml
kubectl get secret skreaver-secrets -o yaml > secrets-backup.yaml

# Backup Helm values
helm get values skreaver -n production > values-backup.yaml
```

---

## Performance Tuning

### Application Tuning

**Connection Pool Sizing:**
```bash
# Rule of thumb: (CPU cores * 2) + effective_spindle_count
export DATABASE_POOL_SIZE=20

# WebSocket connections
export WEBSOCKET_MAX_CONNECTIONS=5000
```

**Worker Threads:**
```bash
# Tokio worker threads (default: CPU cores)
export TOKIO_WORKER_THREADS=8

# For CPU-bound workloads, set to CPU cores
# For I/O-bound workloads, set to CPU cores * 2
```

**Memory Allocation:**
```bash
# Use jemalloc for better performance
export LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libjemalloc.so
```

### Database Tuning

**PostgreSQL:**
```sql
-- Increase connection limit
ALTER SYSTEM SET max_connections = 200;

-- Tune shared buffers (25% of RAM)
ALTER SYSTEM SET shared_buffers = '4GB';

-- Tune work_mem
ALTER SYSTEM SET work_mem = '64MB';

-- Enable query plan caching
ALTER SYSTEM SET plan_cache_mode = 'auto';
```

**Redis:**
```bash
# Increase max clients
redis-cli CONFIG SET maxclients 10000

# Enable persistence
redis-cli CONFIG SET save "900 1 300 10 60 10000"

# Set maxmemory policy
redis-cli CONFIG SET maxmemory-policy allkeys-lru
```

### Network Tuning

**Nginx (Load Balancer):**
```nginx
worker_connections 10000;
keepalive_timeout 65;
client_max_body_size 10M;

upstream skreaver {
    least_conn;
    keepalive 100;
    server skreaver-1:8080 max_fails=3 fail_timeout=30s;
    server skreaver-2:8080 max_fails=3 fail_timeout=30s;
}
```

---

## Security Incidents

### Authentication Breach

**If JWT secret is compromised:**

1. **Immediate action:**
   ```bash
   # Rotate JWT secret
   kubectl create secret generic skreaver-jwt-new --from-literal=secret=$(openssl rand -base64 32)

   # Update deployment
   kubectl set env deployment/skreaver JWT_SECRET_KEY=...

   # Restart all instances
   kubectl rollout restart deployment/skreaver
   ```

2. **Revoke all tokens:**
   ```bash
   # Clear token blacklist (forces re-authentication)
   redis-cli FLUSHDB
   ```

3. **Investigate:**
   - Review access logs
   - Check for unauthorized API calls
   - Audit recent changes

### DDoS Attack

**Mitigation:**

1. **Enable rate limiting:**
   ```yaml
   # Update Nginx config
   limit_req_zone $binary_remote_addr zone=api:10m rate=100r/s;
   limit_req zone=api burst=200 nodelay;
   ```

2. **Block offending IPs:**
   ```bash
   # Add to firewall
   kubectl apply -f network-policy-block.yaml
   ```

3. **Scale up:**
   ```bash
   kubectl scale deployment/skreaver --replicas=20
   ```

### Data Breach

**If unauthorized access detected:**

1. **Isolate affected systems:**
   ```bash
   # Revoke network access
   kubectl apply -f network-policy-lockdown.yaml
   ```

2. **Preserve evidence:**
   ```bash
   # Export logs
   kubectl logs -l app=skreaver --since=24h > incident-logs.txt

   # Snapshot volumes
   kubectl exec backup-pod -- tar czf /backup/snapshot.tar.gz /data
   ```

3. **Follow incident response plan:**
   - Notify security team
   - Engage forensics
   - Follow legal requirements
   - Notify affected parties

---

## Useful Commands

### Quick Health Check

```bash
#!/bin/bash
# quick-health-check.sh

echo "=== Service Health ==="
curl -s http://localhost:8080/health | jq .

echo -e "\n=== Metrics Summary ==="
curl -s http://localhost:8080/metrics | grep -E "http_requests_total|websocket_connections_active|skreaver_memory_usage"

echo -e "\n=== Database Status ==="
kubectl get pods -l app=postgresql

echo -e "\n=== Redis Status ==="
kubectl get pods -l app=redis

echo -e "\n=== Recent Errors ==="
kubectl logs -l app=skreaver --tail=20 | grep ERROR
```

### Emergency Restart

```bash
#!/bin/bash
# emergency-restart.sh

echo "🚨 Emergency restart initiated"

# Save state
echo "Saving state..."
kubectl get deployment skreaver -o yaml > pre-restart-state.yaml

# Restart
echo "Restarting..."
kubectl rollout restart deployment/skreaver

# Wait for rollout
echo "Waiting for rollout..."
kubectl rollout status deployment/skreaver --timeout=5m

# Verify
echo "Verifying health..."
sleep 10
curl http://localhost:8080/health

echo "✅ Restart complete"
```

---

## Contact Information

### On-Call Rotation

- **Primary**: Check PagerDuty schedule
- **Secondary**: Check PagerDuty schedule
- **Manager**: See internal wiki

### Escalation Path

1. **L1 Support** → On-call engineer
2. **L2 Support** → Senior SRE
3. **L3 Support** → Engineering Lead
4. **L4 Support** → CTO

### External Vendors

- **Cloud Provider**: AWS Support (Enterprise)
- **Database**: PostgreSQL Support Contract
- **Monitoring**: Datadog Support

---

## Appendix

### Metric Reference

See [WEBSOCKET_GUIDE.md](WEBSOCKET_GUIDE.md) for complete WebSocket metrics.

### Log Format

```json
{
  "timestamp": "2025-10-27T12:00:00.000Z",
  "level": "INFO",
  "target": "skreaver_http",
  "fields": {
    "message": "Request processed",
    "request_id": "req-123",
    "method": "POST",
    "path": "/agents",
    "status": 200,
    "duration_ms": 45
  }
}
```

### Configuration Reference

See [DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md) for complete configuration options.

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 0.5.0 | 2025-10-27 | Initial SRE runbook |

---

**This runbook is a living document. Update after every incident.**
