FROM rust:1.88-slim-bookworm AS builder
WORKDIR /app
COPY apps/agent-meter/Cargo.toml apps/agent-meter/Cargo.lock ./
RUN mkdir -p crates/collector/src crates/cli/src crates/mcp-wrapper/src && \
    echo "fn main() {}" > crates/collector/src/main.rs && \
    echo "fn main() {}" > crates/cli/src/main.rs && \
    echo "fn main() {}" > crates/mcp-wrapper/src/main.rs
RUN cargo build --release -p agent-meter-collector -p agent-meter-mcp-wrapper 2>/dev/null; true
RUN rm -rf crates
# ARG BUILD_HASH invalida o cache do COPY quando o conteúdo muda
ARG BUILD_HASH=unknown
COPY apps/agent-meter/crates ./crates
COPY apps/agent-meter/migrations ./migrations
COPY apps/agent-meter/scripts ./scripts
COPY apps/agent-meter/install.ps1 ./install.ps1
RUN touch crates/collector/src/main.rs crates/mcp-wrapper/src/main.rs \
    crates/collector/src/routes/conversation_detail.rs \
    crates/collector/src/routes/dashboard.rs \
    crates/collector/src/routes/conversations.rs && \
    cargo build --release -p agent-meter-collector -p agent-meter-mcp-wrapper

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/agent-meter-collector /usr/local/bin/agent-meter-collector
COPY --from=builder /app/target/release/agent-meter-mcp-wrapper /usr/local/bin/agent-meter-mcp-wrapper
ENV PORT=3000
EXPOSE 3000
CMD ["/usr/local/bin/agent-meter-collector"]
