# Dockerfile for Linky C2 Framework
# Uses port 8443 to avoid privileged port requirements

# Build stage
FROM rust:latest as builder

# Install cross-compilation dependencies
RUN apt-get update && apt-get install -y \
    musl-tools \
    mingw-w64 \
    clang \
    lld \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Add cross-compilation targets
RUN rustup target add \
    x86_64-pc-windows-gnu \
    x86_64-unknown-linux-musl

# Copy project and build
WORKDIR /app
COPY . .

# Build release binary
RUN cargo build --release

# Build implants only if DEV_MODE is set (for development/testing)
ARG DEV_MODE=false
RUN if [ "$DEV_MODE" = "true" ]; then \
    echo "[DEV] Building implants..." && \
    cargo build --release --target x86_64-pc-windows-gnu && \
    cargo build --release --target x86_64-unknown-linux-musl; \
    else \
    echo "[PROD] Skipping implant build (use DEV_MODE=true to enable)"; \
    fi

# Runtime stage
FROM debian:stable-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create implants directory
RUN mkdir -p /implants

# Copy server binary
COPY --from=builder /app/target/release/linky /usr/local/bin/

# Create implants directory in /usr/local for DEV_MODE builds
RUN mkdir -p /usr/local/implants

# Copy implants using a script that handles missing files gracefully
COPY copy_implants.sh /copy_implants.sh
RUN chmod +x /copy_implants.sh
RUN /copy_implants.sh

# Create entrypoint script to copy implants to volume
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]

# Expose port 8443
EXPOSE 8443

# Default command (uses port 8443)
CMD ["linky", "0.0.0.0:8443"]