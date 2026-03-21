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

WORKDIR /app
COPY . .

# Build server binary
RUN cargo build --release -p linky

# Build implants only if DEV_MODE is set (for development/testing)
ARG DEV_MODE=false
RUN mkdir -p /usr/local/implants && \
    if [ "$DEV_MODE" = "true" ]; then \
        echo "[DEV] Building implants..." && \
        cargo build --release --target x86_64-pc-windows-gnu -p link-windows && \
        cargo build --release --target x86_64-unknown-linux-musl -p link-linux && \
        cp target/x86_64-pc-windows-gnu/release/link-windows.exe /usr/local/implants/ 2>/dev/null || true && \
        cp target/x86_64-unknown-linux-musl/release/link-linux /usr/local/implants/ 2>/dev/null || true; \
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

RUN mkdir -p /implants /usr/local/implants

# Copy server binary
COPY --from=builder /app/target/release/linky /usr/local/bin/

# Copy implants built in DEV_MODE (empty directory otherwise)
COPY --from=builder /usr/local/implants/ /usr/local/implants/

# Entrypoint: copy DEV_MODE implants to the volume mount, then start server
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]

EXPOSE 8443

CMD ["linky", "0.0.0.0:8443"]
