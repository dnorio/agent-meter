# Security Policy

## Supported versions

| Version | Supported |
| ------- | --------- |
| latest release (`v*`) | ✅ |
| `main` | ✅ (pre-release) |

## Reporting a vulnerability

**Do not** open a public GitHub issue for security problems.

Email the maintainer privately (contact via GitHub profile `dnorio`) with:

- Description and impact
- Steps to reproduce
- Affected version/commit

We aim to acknowledge within **72 hours** and provide a fix or mitigation timeline for confirmed issues.

## Scope

In scope:

- `agent-meter-collector` (REST/OTLP ingest, SQLite, embedded UI)
- `agent-meter-proxy` (local HTTPS proxy)
- Supply chain: `Cargo.lock`, release artifacts, install scripts

Out of scope:

- Hosted SaaS product (separate deployment; not this repo)
- User-configured PostgreSQL backends (`DATABASE_URL`) — operator responsibility

## Safe defaults

- Binds to `127.0.0.1` by default
- No telemetry phone-home
- Admin reset/delete endpoints are localhost-only

## Pre-public checklist

Before making this repository **public**, maintainers must run:

```bash
bash scripts/ci/secret-scan-history.sh
bash scripts/ci/history-audit.sh
bash scripts/ci/oss-scrub-check.sh
```

Organization-specific deny patterns should live in local-only `scripts/ci/private-patterns.txt` or the path pointed to by `OSS_PRIVATE_PATTERNS_FILE`; do not commit private audit reports or private infrastructure names.
