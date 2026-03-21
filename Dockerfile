# Dockerfile for Linky C2 Framework
# Single-stage build: cargo is available at runtime for on-the-fly implant generation

FROM rust:latest

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

RUN mkdir -p /implants
ENV LINKY_OUTPUT_DIR=/implants

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 8443

ENTRYPOINT ["/entrypoint.sh"]
CMD ["/app/target/release/linky", "0.0.0.0:8443"]
