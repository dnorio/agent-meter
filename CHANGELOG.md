# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.6] - 2026-07-21

### Added

- macOS release artifacts (collector + proxy, arm64 + x86_64) via `macos-latest` CI job.
- `install-proxy.ps1` Windows installer for HTTPS proxy.
- Expanded Postgres CI smoke tests (query, top-tools, delete conversation).

### Fixed

- Release workflow SDK publish step no longer fails the run when registry secrets are missing.

## [0.1.5] - 2026-07-21

### Added

- `agent-meter-proxy` prebuilt artifacts in GitHub releases (Linux x86_64/arm64, Windows).
- `install-proxy.sh` one-liner for HTTPS proxy install.

## [0.1.4] - 2026-07-21

### Added

- REST ingest uses shared async buffer + per-IP rate limit (parity with OTLP).
- OTLP on main port (`:8081/v1/traces`) so SDKs work out of the box.
- `503` + `Retry-After` when ingest buffer is full (no silent drops).
- CI: version sync check, SDK→collector integration smoke.
- `docker-compose.secure.yml` with API key auth enabled.
- `CONTRIBUTING.md` contributor guide.

### Fixed

- SDK OTLP payloads use `execute_tool` span naming (collector-compatible).
- PyPI publish script fails on upload errors unless `ALLOW_EXISTING=1`.

## [0.1.3] - 2026-07-21

### Added

- Tag-triggered GitHub Actions release workflow (cross-build + smoke + upload).
- Maintainer release guide (`docs/RELEASE.md`) and `scripts/ci/release-tag.sh`.
- Windows installer SHA256 verification (mandatory), embedded API keys docs, keys integration test
- Embedded docs: API keys section + `AGENT_METER_API_KEY`.

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
