# Skreaver Helm Chart

Official Helm chart for deploying Skreaver agent infrastructure on Kubernetes.

## Prerequisites

- Kubernetes 1.19+
- Helm 3.8+
- PV provisioner support in the underlying infrastructure (for Redis/PostgreSQL persistence)

## Installing the Chart

```bash
# Add required repositories for dependencies
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update

# Install with default configuration
helm install skreaver ./helm/skreaver

# Install with custom values
helm install skreaver ./helm/skreaver -f custom-values.yaml

# Install in specific namespace
helm install skreaver ./helm/skreaver --namespace skreaver --create-namespace
```

## Uninstalling the Chart

```bash
helm uninstall skreaver
```

## Configuration

### Core Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Number of Skreaver replicas | `1` |
| `image.repository` | Skreaver image repository | `skreaver` |
| `image.tag` | Image tag (defaults to chart appVersion) | `""` |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |

### Service Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `service.type` | Kubernetes service type | `ClusterIP` |
| `service.port` | Service port | `8080` |
| `service.targetPort` | Container port | `3000` |

### Resource Limits

| Parameter | Description | Default |
|-----------|-------------|---------|
| `resources.limits.cpu` | CPU limit | `1000m` |
| `resources.limits.memory` | Memory limit | `512Mi` |
| `resources.requests.cpu` | CPU request | `250m` |
| `resources.requests.memory` | Memory request | `256Mi` |

### Autoscaling

| Parameter | Description | Default |
|-----------|-------------|---------|
| `autoscaling.enabled` | Enable HPA | `false` |
| `autoscaling.minReplicas` | Minimum replicas | `1` |
| `autoscaling.maxReplicas` | Maximum replicas | `10` |
| `autoscaling.targetCPUUtilizationPercentage` | Target CPU % | `80` |

### Redis Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `redis.enabled` | Enable Redis dependency | `true` |
| `redis.auth.enabled` | Enable Redis auth | `false` |
| `redis.master.persistence.size` | Redis master PV size | `8Gi` |

### PostgreSQL Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `postgresql.enabled` | Enable PostgreSQL dependency | `true` |
| `postgresql.auth.username` | PostgreSQL username | `skreaver` |
| `postgresql.auth.password` | PostgreSQL password | `skreaver` |
| `postgresql.auth.database` | PostgreSQL database | `skreaver` |
| `postgresql.primary.persistence.size` | PostgreSQL PV size | `10Gi` |

### Ingress

| Parameter | Description | Default |
|-----------|-------------|---------|
| `ingress.enabled` | Enable ingress | `false` |
| `ingress.className` | Ingress class name | `""` |
| `ingress.hosts[0].host` | Hostname | `skreaver.local` |

## Examples

### Production Deployment

```bash
helm install skreaver ./helm/skreaver \
  --set replicaCount=3 \
  --set resources.limits.cpu=2000m \
  --set resources.limits.memory=1Gi \
  --set autoscaling.enabled=true \
  --set postgresql.auth.password=secure-password \
  --set redis.auth.enabled=true
```

### Development Deployment

```bash
helm install skreaver ./helm/skreaver \
  --set redis.master.persistence.enabled=false \
  --set postgresql.primary.persistence.enabled=false
```

### With Ingress

```bash
helm install skreaver ./helm/skreaver \
  --set ingress.enabled=true \
  --set ingress.className=nginx \
  --set ingress.hosts[0].host=skreaver.example.com \
  --set ingress.tls[0].secretName=skreaver-tls \
  --set ingress.tls[0].hosts[0]=skreaver.example.com
```

## Values File Examples

### Production values.yaml

```yaml
replicaCount: 3

resources:
  limits:
    cpu: 2000m
    memory: 1Gi
  requests:
    cpu: 500m
    memory: 512Mi

autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 20
  targetCPUUtilizationPercentage: 70

redis:
  enabled: true
  auth:
    enabled: true
    password: "change-me"
  master:
    persistence:
      size: 20Gi

postgresql:
  enabled: true
  auth:
    password: "change-me"
  primary:
    persistence:
      size: 50Gi

ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
  hosts:
    - host: skreaver.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: skreaver-tls
      hosts:
        - skreaver.example.com
```

## Architecture

The chart deploys:

1. **Skreaver Deployment** - Main application pods
2. **Service** - ClusterIP service for internal access
3. **Redis** (optional) - For memory backend and mesh communication
4. **PostgreSQL** (optional) - For persistent storage
5. **Ingress** (optional) - External access
6. **HPA** (optional) - Horizontal Pod Autoscaler
7. **ServiceAccount** - Kubernetes service account
8. **ConfigMap** - Configuration data

## Dependency Management

Dependencies are managed via Helm:

```bash
# Update dependencies
helm dependency update ./helm/skreaver

# List dependencies
helm dependency list ./helm/skreaver
```

## Testing

```bash
# Dry run to see generated manifests
helm install skreaver ./helm/skreaver --dry-run --debug

# Template rendering
helm template skreaver ./helm/skreaver

# Lint chart
helm lint ./helm/skreaver
```

## Upgrading

```bash
# Upgrade with new values
helm upgrade skreaver ./helm/skreaver \
  --set image.tag=0.4.0 \
  --reuse-values

# Rollback
helm rollback skreaver 1
```

## Monitoring

The chart supports Prometheus monitoring via pod annotations:

```yaml
podAnnotations:
  prometheus.io/scrape: "true"
  prometheus.io/port: "3000"
  prometheus.io/path: "/metrics"
```

## Troubleshooting

### Check pod status
```bash
kubectl get pods -l app.kubernetes.io/name=skreaver
kubectl logs -l app.kubernetes.io/name=skreaver
```

### Check services
```bash
kubectl get svc -l app.kubernetes.io/name=skreaver
kubectl describe svc skreaver
```

### Debug Redis connection
```bash
kubectl run redis-test --rm -it --image=redis:alpine -- redis-cli -h skreaver-redis-master ping
```

### Debug PostgreSQL connection
```bash
kubectl run pg-test --rm -it --image=postgres:alpine -- psql -h skreaver-postgresql -U skreaver
```

## Security

- Runs as non-root user (UID 65532)
- Read-only root filesystem
- No privilege escalation
- Drops all capabilities
- Network policies (configure separately)
- Pod security standards compliant

## License

MIT
