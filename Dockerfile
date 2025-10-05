# Multi-stage Dockerfile for Skreaver
# Optimized for minimal image size with distroless runtime

# ============================================================================
# Stage 1: Build Application
# ============================================================================
FROM rustlang/rust:nightly-bookworm-slim AS builder

# Install system dependencies for compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY skreaver-cli ./skreaver-cli
COPY tests ./tests
COPY examples ./examples
COPY benches ./benches

# Build the CLI binary in release mode (requires nightly for edition2024)
RUN cargo +nightly build --release -p skreaver-cli

# Strip debug symbols to reduce binary size
RUN strip /app/target/release/skreaver-cli


# ============================================================================
# Stage 2: Runtime (Distroless)
# ============================================================================
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

# Copy binary from builder
COPY --from=builder /app/target/release/skreaver-cli /usr/local/bin/skreaver

# Run as non-root user (already set in distroless/nonroot)
USER nonroot:nonroot

# Set working directory
WORKDIR /app

# Metadata
LABEL org.opencontainers.image.title="Skreaver"
LABEL org.opencontainers.image.description="Extensible agent framework for Rust"
LABEL org.opencontainers.image.version="0.3.0"
LABEL org.opencontainers.image.source="https://github.com/yourusername/skreaver"

# Health check endpoint (assumes HTTP server running on 3000)
# Uncomment if deploying HTTP runtime
# HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
#   CMD ["/usr/local/bin/skreaver", "agent", "--help"]

# Default command shows help
ENTRYPOINT ["/usr/local/bin/skreaver"]
CMD ["--help"]
