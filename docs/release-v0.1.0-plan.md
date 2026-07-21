# agent-meter v0.1.0 release plan

> **Superseded.** Use [RELEASE.md](./RELEASE.md) for the current maintainer checklist.
> v0.1.0–v0.1.2 shipped. Tag-triggered CI handles artifact builds from v0.1.3+.

## Historical gate (v0.1.0)

All items passed before the first public tag:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/ci/smoke-demo.sh
bash scripts/ci/history-audit.sh
```

## Shipped releases

| Version | Date | Highlights |
|---------|------|------------|
| v0.1.0 | 2026-07-21 | Initial OSS, binaries, SDKs |
| v0.1.1 | 2026-07-21 | API key auth, Postgres CI, install checksums |
| v0.1.2 | 2026-07-21 | `keys` CLI, TS7, release binaries |
| v0.1.3 | 2026-07-21 | Release automation, docs polish |
