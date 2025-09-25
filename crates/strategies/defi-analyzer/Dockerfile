# Multi-stage build for DeFi Analyzer Strategy
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    clang \
    llvm \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build the project
RUN cargo build --release --package defi-analyzer

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false defi-analyzer

# Create directories
RUN mkdir -p /app/config /app/logs /app/data \
    && chown -R defi-analyzer:defi-analyzer /app

# Copy binary from builder
COPY --from=builder /app/target/release/defi-analyzer /app/defi-analyzer

# Copy configuration
COPY config/ /app/config/

# Set permissions
RUN chmod +x /app/defi-analyzer \
    && chown defi-analyzer:defi-analyzer /app/defi-analyzer

# Switch to app user
USER defi-analyzer

# Set working directory
WORKDIR /app

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Environment variables
ENV RUST_LOG=info
ENV DEFI_ANALYZER_CONFIG_PATH=/app/config/production.toml

# Run the application
CMD ["./defi-analyzer"]
