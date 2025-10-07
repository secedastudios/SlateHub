# Build stage - use bookworm to match runtime GLIBC version
FROM debian:bookworm-slim AS builder

# Install build dependencies and Rust
RUN apt-get update && apt-get install -y \
    curl \
    ca-certificates \
    pkg-config \
    libssl-dev \
    gcc \
    && rm -rf /var/lib/apt/lists/*

# Install Rust using rustup
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain stable && \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME && \
    rustup --version && \
    cargo --version && \
    rustc --version

# Create app directory
WORKDIR /usr/src/app

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy src/main.rs for dependency compilation
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs

# Build dependencies - this is cached unless Cargo.toml or Cargo.lock change
RUN cargo build --release

# Remove dummy source
RUN rm -rf src

# Copy the actual source code
COPY src ./src

# Copy templates and static directories
COPY templates ./templates
COPY static ./static

# Build the actual application
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage - use the same bookworm base
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user to run the app
RUN useradd -m -u 1001 -s /bin/bash slatehub

# Create necessary directories
RUN mkdir -p /app/templates /app/static && \
    chown -R slatehub:slatehub /app

WORKDIR /app

# Copy the binary from builder
COPY --from=builder --chown=slatehub:slatehub /usr/src/app/target/release/slatehub /app/slatehub

# Copy templates and static files from builder
COPY --from=builder --chown=slatehub:slatehub /usr/src/app/templates /app/templates
COPY --from=builder --chown=slatehub:slatehub /usr/src/app/static /app/static

# Switch to non-root user
USER slatehub

# Expose the port the app runs on
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

# Run the binary
CMD ["./slatehub"]
