# Kubernetes Deployment Guide

Complete guide for deploying Skreaver on Kubernetes using Helm.

## Quick Start

```bash
# Clone repository
git clone https://github.com/shurankain/skreaver
cd skreaver

# Add Helm dependencies
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update

# Install Skreaver
helm install skreaver ./helm/skreaver --namespace skreaver --create-namespace
```

## Prerequisites

- Kubernetes cluster 1.19+
- Helm 3.8+
- kubectl configured
- Persistent volume provisioner (for Redis/PostgreSQL)
- (Optional) Ingress controller (nginx, traefik, etc.)

## Architecture

```
┌─────────────────────────────────────────┐
│           Ingress (Optional)            │
│      skreaver.example.com               │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│              Service                    │
│          ClusterIP:8080                 │
└─────────────────────────────────────────┘
                    │
         ┌──────────┴──────────┐
         ▼                     ▼
┌─────────────────┐   ┌─────────────────┐
│  Skreaver Pod 1 │   │  Skreaver Pod 2 │
│   (Deployment)  │   │   (Deployment)  │
└─────────────────┘   └─────────────────┘
         │                     │
    ┌────┴────────────────────┘
    │              │
    ▼              ▼
┌────────┐   ┌──────────────┐
│ Redis  │   │ PostgreSQL   │
│(StatefulSet) │(StatefulSet)│
└────────┘   └──────────────┘
```

## Installation Options

### Option 1: Default Installation

```bash
helm install skreaver ./helm/skreaver
```

This installs:
- 1 Skreaver replica
- Redis (with 8Gi persistence)
- PostgreSQL (with 10Gi persistence)
- ClusterIP service

### Option 2: Production Installation

```bash
helm install skreaver ./helm/skreaver \
  --set replicaCount=3 \
  --set autoscaling.enabled=true \
  --set autoscaling.minReplicas=3 \
  --set autoscaling.maxReplicas=20 \
  --set resources.limits.cpu=2000m \
  --set resources.limits.memory=1Gi \
  --set postgresql.auth.password=secure-password \
  --set redis.auth.enabled=true \
  --set redis.auth.password=secure-password
```

### Option 3: Development Installation

```bash
helm install skreaver ./helm/skreaver \
  --set redis.master.persistence.enabled=false \
  --set postgresql.primary.persistence.enabled=false \
  --set resources.limits.cpu=500m \
  --set resources.limits.memory=256Mi
```

### Option 4: With Ingress

```bash
helm install skreaver ./helm/skreaver \
  --set ingress.enabled=true \
  --set ingress.className=nginx \
  --set ingress.hosts[0].host=skreaver.example.com \
  --set ingress.annotations."cert-manager\.io/cluster-issuer"=letsencrypt-prod \
  --set ingress.tls[0].secretName=skreaver-tls \
  --set ingress.tls[0].hosts[0]=skreaver.example.com
```

## Configuration

### Resource Management

Define resource limits and requests:

```yaml
# production-values.yaml
resources:
  limits:
    cpu: 2000m
    memory: 1Gi
  requests:
    cpu: 500m
    memory: 512Mi
```

### Autoscaling

Configure Horizontal Pod Autoscaler:

```yaml
autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 20
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80
```

### Persistence

Configure persistent volumes:

```yaml
redis:
  master:
    persistence:
      enabled: true
      storageClass: fast-ssd
      size: 20Gi

postgresql:
  primary:
    persistence:
      enabled: true
      storageClass: fast-ssd
      size: 50Gi
```

## Security

### Pod Security

The deployment follows security best practices:

```yaml
securityContext:
  runAsNonRoot: true
  runAsUser: 65532  # nonroot user
  allowPrivilegeEscalation: false
  capabilities:
    drop:
    - ALL
  readOnlyRootFilesystem: true
```

### Network Policies

Create network policies to restrict traffic:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: skreaver-netpol
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: skreaver
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          app.kubernetes.io/name: nginx-ingress
    ports:
    - protocol: TCP
      port: 3000
  egress:
  - to:
    - podSelector:
        matchLabels:
          app.kubernetes.io/name: redis
    ports:
    - protocol: TCP
      port: 6379
  - to:
    - podSelector:
        matchLabels:
          app.kubernetes.io/name: postgresql
    ports:
    - protocol: TCP
      port: 5432
```

### Secrets Management

Use Kubernetes secrets for sensitive data:

```bash
# Create secret
kubectl create secret generic skreaver-secrets \
  --from-literal=redis-password=change-me \
  --from-literal=postgres-password=change-me \
  --namespace skreaver

# Reference in values.yaml
envFrom:
  - secretRef:
      name: skreaver-secrets
```

## Monitoring & Observability

### Prometheus Integration

Enable Prometheus scraping:

```yaml
podAnnotations:
  prometheus.io/scrape: "true"
  prometheus.io/port: "3000"
  prometheus.io/path: "/metrics"
```

### Health Checks

Configure liveness and readiness probes:

```yaml
livenessProbe:
  enabled: true
  httpGet:
    path: /health
    port: http
  initialDelaySeconds: 30
  periodSeconds: 10

readinessProbe:
  enabled: true
  httpGet:
    path: /ready
    port: http
  initialDelaySeconds: 10
  periodSeconds: 5
```

### Logging

Configure structured logging:

```yaml
env:
  - name: RUST_LOG
    value: "info,skreaver=debug"
  - name: LOG_FORMAT
    value: "json"
```

## High Availability

### Multi-Zone Deployment

Spread pods across availability zones:

```yaml
affinity:
  podAntiAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
    - weight: 100
      podAffinityTerm:
        labelSelector:
          matchExpressions:
          - key: app.kubernetes.io/name
            operator: In
            values:
            - skreaver
        topologyKey: topology.kubernetes.io/zone
```

### Database Replication

Enable PostgreSQL replication:

```yaml
postgresql:
  replication:
    enabled: true
    readReplicas: 2
```

## Upgrades & Rollbacks

### Upgrade Strategy

```bash
# Upgrade with new version
helm upgrade skreaver ./helm/skreaver \
  --set image.tag=0.4.0 \
  --reuse-values

# Check rollout status
kubectl rollout status deployment/skreaver

# View rollout history
helm history skreaver
```

### Rollback

```bash
# Rollback to previous version
helm rollback skreaver

# Rollback to specific revision
helm rollback skreaver 3
```

## Troubleshooting

### Pod Issues

```bash
# Check pod status
kubectl get pods -l app.kubernetes.io/name=skreaver

# View logs
kubectl logs -l app.kubernetes.io/name=skreaver --tail=100

# Describe pod
kubectl describe pod <pod-name>

# Get pod events
kubectl get events --sort-by='.lastTimestamp'
```

### Service Issues

```bash
# Check service
kubectl get svc skreaver
kubectl describe svc skreaver

# Test service connectivity
kubectl run curl-test --rm -it --image=curlimages/curl -- sh
curl http://skreaver:8080/health
```

### Redis Connection Issues

```bash
# Test Redis connection
kubectl run redis-test --rm -it --image=redis:alpine -- \
  redis-cli -h skreaver-redis-master ping

# Check Redis logs
kubectl logs -l app.kubernetes.io/name=redis
```

### PostgreSQL Connection Issues

```bash
# Test PostgreSQL connection
kubectl run pg-test --rm -it --image=postgres:alpine -- \
  psql -h skreaver-postgresql -U skreaver -d skreaver

# Check PostgreSQL logs
kubectl logs -l app.kubernetes.io/name=postgresql
```

### Resource Issues

```bash
# Check resource usage
kubectl top pods -l app.kubernetes.io/name=skreaver
kubectl top nodes

# Describe resource limits
kubectl describe resourcequota
kubectl describe limitrange
```

## Backup & Restore

### Backup PostgreSQL

```bash
# Create backup
kubectl exec -it skreaver-postgresql-0 -- \
  pg_dump -U skreaver skreaver > backup.sql

# Restore backup
kubectl exec -i skreaver-postgresql-0 -- \
  psql -U skreaver skreaver < backup.sql
```

### Backup Redis

```bash
# Trigger Redis save
kubectl exec -it skreaver-redis-master-0 -- \
  redis-cli SAVE

# Copy RDB file
kubectl cp skreaver-redis-master-0:/data/dump.rdb ./redis-backup.rdb
```

## Multi-Cluster Deployment

### Using Kustomize

```yaml
# kustomization.yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

helmCharts:
- name: skreaver
  releaseName: skreaver
  version: 0.3.0
  repo: https://charts.skreaver.io
  valuesFile: values-prod.yaml
```

### GitOps with ArgoCD

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: skreaver
spec:
  destination:
    namespace: skreaver
    server: https://kubernetes.default.svc
  source:
    chart: skreaver
    repoURL: https://charts.skreaver.io
    targetRevision: 0.3.0
    helm:
      values: |
        replicaCount: 3
        autoscaling:
          enabled: true
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
```

## Cost Optimization

### Use Node Pools

```yaml
nodeSelector:
  workload-type: agents

tolerations:
- key: "workload-type"
  operator: "Equal"
  value: "agents"
  effect: "NoSchedule"
```

### Spot Instances

```yaml
affinity:
  nodeAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
    - weight: 100
      preference:
        matchExpressions:
        - key: eks.amazonaws.com/capacityType
          operator: In
          values:
          - SPOT
```

## See Also

- [Helm Chart README](../helm/skreaver/README.md)
- [Docker Guide](./DOCKER.md)
- [Production Best Practices](../README.md)
