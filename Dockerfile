# OSS standalone — build from repo root (SQLite default)
FROM rust:1.97-slim-trixie AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p crates/collector/src crates/cli/src crates/mcp-wrapper/src crates/db/src crates/proxy/src && \
    echo "fn main() {}" > crates/collector/src/main.rs && \
    echo "fn main() {}" > crates/cli/src/main.rs && \
    echo "fn main() {}" > crates/mcp-wrapper/src/main.rs && \
    echo "pub struct SqliteDb;" > crates/db/src/lib.rs && \
    echo "fn main() {}" > crates/proxy/src/lib.rs
RUN cargo build --release -p agent-meter-collector 2>/dev/null || true
COPY migrations ./migrations
COPY crates ./crates
RUN touch crates/collector/src/main.rs && \
    cargo build --release -p agent-meter-collector

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/agent-meter-collector /usr/local/bin/agent-meter
ENV AGENT_METER_HOST=0.0.0.0 AGENT_METER_PORT=8081 AGENT_METER_OTLP_PORT=4318 DATABASE_URL=sqlite:///data/agent-meter.db
EXPOSE 8081 4318
VOLUME ["/data"]
CMD ["agent-meter", "serve"]
