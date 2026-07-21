# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Node SDK published as `@dnorio/agent-meter` under the dnorio npm org.
- Python SDK available on PyPI as `agentmeter-obs` and `dnorio-agent-meter`.

## [0.1.0] - 2026-07-21

Initial public OSS release.

### Added

- SQLite-first collector with embedded dashboard and OTLP ingest.
- Proxy, MCP wrapper, Node/Python SDKs.
- Hardened Docker standalone profile (non-root, read-only rootfs, tmpfs).
- CI: rust-cache, Python wheel build, Node tarball install smoke.

### Changed

- Healthcheck uses `agent-meter check` instead of `wget` in the runtime image.
