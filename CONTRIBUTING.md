# Contributing to agent-meter

Thanks for helping improve agent-meter. This guide covers local development,
validation, and release expectations.

## Repository layout

| Path | Purpose |
|------|---------|
| `crates/collector/` | Main `agent-meter` binary — UI, REST, OTLP |
| `crates/db/` | SQLite + Postgres storage layer |
| `crates/cli/` | Legacy CLI client (not shipped in release artifacts) |
| `crates/proxy/` | HTTPS proxy helpers |
| `sdk/node/`, `sdk/python/` | Published client SDKs |
| `scripts/ci/` | CI smoke and release helpers |

The release artifact is the **collector** binary (`agent-meter` in Docker/releases).

## Local setup

```bash
git clone https://github.com/dnorio/agent-meter.git
cd agent-meter
cargo run -p agent-meter-collector -- demo
cargo run -p agent-meter-collector -- serve
```

Requirements: Rust 1.75+, optional Docker for container smoke.

## Validation before opening a PR

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/ci/check-versions.sh
bash scripts/ci/smoke-demo.sh
bash scripts/ci/sdk-integration-smoke.sh   # after npm build + pytest deps
```

## Version bumps

Keep these in sync (CI enforces via `check-versions.sh`):

- `Cargo.toml` (workspace)
- `sdk/node/package.json`
- `sdk/python/pyproject.toml`
- `CHANGELOG.md`

See [docs/RELEASE.md](docs/RELEASE.md) for tagging and publishing.

## Security

- Do not commit secrets or private patterns.
- Run `bash scripts/ci/history-audit.sh` before large history rewrites.
- Report vulnerabilities via [SECURITY.md](SECURITY.md).

## Docker profiles

- **Default:** `docker-compose.standalone.yml` — local network, open ingest
- **Hardened:** `docker-compose.secure.yml` — `AGENT_METER_REQUIRE_API_KEY=1`

See [docs/docker-runtime.md](docs/docker-runtime.md) for runtime notes.
