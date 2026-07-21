# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-07-21

### Added

- `agent-meter keys create` and `agent-meter keys list` for API key management.

### Changed

- SDK dev-deps: TypeScript 7, `@types/node` 26.

## [0.1.1] - 2026-07-21

### Added

- Optional API key auth for ingest (`AGENT_METER_REQUIRE_API_KEY=1`).
- Postgres CI smoke test (migrate + insert + cost summary).
- `install.sh` verifies release tarball SHA256 when `SHA256SUMS` is available.
- SVG embed badges at `/badge/cost.svg` and `/badge/events.svg` (5-minute cache).
- Docker build + health smoke job in CI.
- Dependabot coverage for npm and pip SDK ecosystems.
- SQLite seeds default `personal` org for API key management.

### Security

- Admin delete/reset routes reject non-loopback clients (403).

### Changed

- README badges for CI, npm, and PyPI.
- Python CI runs `twine check` on built wheels.

## [0.1.0] - 2026-07-21

Initial public OSS release.

### Added

- SQLite-first collector with embedded dashboard and OTLP ingest.
- Proxy, MCP wrapper, Node/Python SDKs.
- Hardened Docker standalone profile (non-root, read-only rootfs, tmpfs).
- CI: rust-cache, Python wheel build, Node tarball install smoke.

### Changed

- Healthcheck uses `agent-meter check` instead of `wget` in the runtime image.
