# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- SVG embed badges at `/badge/cost.svg` and `/badge/events.svg` (5-minute cache).
- Docker build + health smoke job in CI.
- Dependabot coverage for npm and pip SDK ecosystems.

### Security

- Admin delete/reset routes now reject non-loopback clients (403).

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
