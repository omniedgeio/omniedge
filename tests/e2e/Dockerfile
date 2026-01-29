# E2E Test Dockerfile for OmniEdge
# Builds the CLI and provides tools for P2P connectivity testing

FROM rust:1.87-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release -p omniedge-cli

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    iproute2 \
    iputils-ping \
    netcat-openbsd \
    procps \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/omniedge /usr/local/bin/omniedge

# Create config directory
RUN mkdir -p /root/.config/omniedge

ENTRYPOINT ["/usr/local/bin/omniedge"]
