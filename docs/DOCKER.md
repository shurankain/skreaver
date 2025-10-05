# Docker Deployment Guide

This guide covers Docker deployment for the Skreaver framework.

## Quick Start

### Build the Image

```bash
# Using the build script
./scripts/docker-build.sh

# Or manually with Docker
docker build -t skreaver:latest .
```

### Run the Container

```bash
# Show help
docker run --rm skreaver:latest --help

# Show version
docker run --rm skreaver:latest --version

# Generate a new agent
docker run --rm -v $(pwd):/workspace skreaver:latest new --name my-agent --template simple
```

## Multi-Stage Build

The Dockerfile uses a multi-stage build process for optimal image size:

1. **Builder Stage**: Compiles the Rust code with all dependencies
2. **Runtime Stage**: Uses Google's distroless base image for minimal attack surface

### Image Sizes

- Builder image: ~2.5GB (contains Rust toolchain and build artifacts)
- Final runtime image: ~50-100MB (only the binary and minimal runtime dependencies)

## Docker Compose

For development and testing with dependencies:

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Stop all services
docker-compose down
```

### Services

The `docker-compose.yml` includes:

- **skreaver**: Main CLI container
- **redis**: Redis instance for memory backend and mesh communication
- **postgres**: PostgreSQL database for persistent memory

## Configuration

### Environment Variables

```bash
# Redis connection
docker run --rm \
  -e REDIS_URL=redis://redis:6379 \
  skreaver:latest agent run

# PostgreSQL connection
docker run --rm \
  -e DATABASE_URL=postgresql://user:pass@postgres:5432/skreaver \
  skreaver:latest agent run

# Logging
docker run --rm \
  -e RUST_LOG=debug \
  skreaver:latest agent run
```

### Volume Mounts

```bash
# Mount current directory as workspace
docker run --rm \
  -v $(pwd):/workspace \
  -w /workspace \
  skreaver:latest new --name my-agent

# Mount config directory
docker run --rm \
  -v $(pwd)/config:/app/config:ro \
  skreaver:latest agent run --config /app/config/agent.toml
```

## Health Checks

The image includes health check configuration (commented out by default):

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD ["/usr/local/bin/skreaver", "agent", "--help"]
```

Uncomment in the Dockerfile if deploying agents as long-running services.

## Security

### Non-Root User

The runtime image runs as a non-root user (`nonroot:nonroot` from distroless):

```bash
# Verify user
docker run --rm skreaver:latest sh -c "whoami"
# Output: nonroot
```

### Minimal Attack Surface

The distroless base image:
- No shell or package managers
- Only essential runtime libraries
- Reduced CVE exposure

### Network Isolation

Use Docker networks for service isolation:

```bash
# Create isolated network
docker network create skreaver-net

# Run with network
docker run --rm --network skreaver-net skreaver:latest
```

## Production Deployment

### Resource Limits

```bash
docker run --rm \
  --cpus="1.0" \
  --memory="512m" \
  --memory-swap="512m" \
  skreaver:latest agent run
```

### Restart Policies

```bash
docker run -d \
  --restart=unless-stopped \
  --name skreaver-agent-1 \
  skreaver:latest agent run
```

### Logging

```bash
# JSON logging driver
docker run -d \
  --log-driver=json-file \
  --log-opt max-size=10m \
  --log-opt max-file=3 \
  skreaver:latest agent run
```

## Kubernetes Integration

For Kubernetes deployment, see [KUBERNETES.md](./KUBERNETES.md).

## Troubleshooting

### Build Issues

**Problem**: Build fails with "disk space" error
```bash
# Clean Docker build cache
docker builder prune -a
```

**Problem**: Build is slow
```bash
# Use BuildKit for better caching
export DOCKER_BUILDKIT=1
docker build -t skreaver:latest .
```

### Runtime Issues

**Problem**: Permission denied errors
```bash
# Check if directory is writable by nonroot user
docker run --rm -v $(pwd):/workspace skreaver:latest ls -la /workspace
```

**Problem**: Cannot connect to Redis/PostgreSQL
```bash
# Verify network connectivity
docker run --rm --network skreaver-network skreaver:latest \
  sh -c "nc -zv redis 6379"
```

## Advanced Usage

### Multi-Architecture Builds

```bash
# Build for multiple platforms
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t skreaver:latest \
  --push \
  .
```

### Custom Build Arguments

```bash
# Build with specific Rust version
docker build \
  --build-arg RUST_VERSION=1.83 \
  -t skreaver:latest \
  .
```

## CI/CD Integration

### GitHub Actions

```yaml
- name: Build Docker image
  run: docker build -t skreaver:${{ github.sha }} .

- name: Test image
  run: docker run --rm skreaver:${{ github.sha }} --version
```

### GitLab CI

```yaml
build:
  script:
    - docker build -t $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA .
    - docker push $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA
```

## Image Registry

### Docker Hub

```bash
# Tag and push
docker tag skreaver:latest username/skreaver:0.3.0
docker push username/skreaver:0.3.0
```

### GitHub Container Registry

```bash
# Login
echo $GITHUB_TOKEN | docker login ghcr.io -u username --password-stdin

# Tag and push
docker tag skreaver:latest ghcr.io/username/skreaver:0.3.0
docker push ghcr.io/username/skreaver:0.3.0
```

## Best Practices

1. **Use specific tags** instead of `latest` in production
2. **Scan images** for vulnerabilities with `docker scan`
3. **Pin base image versions** for reproducible builds
4. **Use multi-stage builds** to minimize image size
5. **Run as non-root user** for security
6. **Set resource limits** to prevent resource exhaustion
7. **Use health checks** for container orchestration
8. **Enable logging** for observability

## See Also

- [Kubernetes Deployment](./KUBERNETES.md)
- [Development Guide](../README.md)
- [Security Configuration](../SECURITY.md)
