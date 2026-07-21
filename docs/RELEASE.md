# Release guide

Maintainer checklist for tagging a new version.

## Pre-release

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/ci/smoke-demo.sh
bash scripts/ci/node-tarball-smoke.sh
```

Update versions in:

- `Cargo.toml` (workspace)
- `sdk/node/package.json`
- `sdk/python/pyproject.toml`
- `CHANGELOG.md`

## Tag and publish

```bash
# Bump versions + changelog, merge to main, then:
git tag -a v0.1.x -m "agent-meter v0.1.x"
git push origin v0.1.x
```

Pushing a `v*` tag triggers [`.github/workflows/release.yml`](../.github/workflows/release.yml):

1. Cross-build Linux x86_64/arm64 + Windows artifacts (collector + proxy)
2. `release-smoke.sh` validation
3. GitHub Release upload with `SHA256SUMS`

Release artifacts:

| Binary | Linux tarballs | macOS tarballs | Windows zip |
|--------|----------------|----------------|-------------|
| `agent-meter` (collector) | `agent-meter-linux-{x86_64,arm64}.tar.gz` | `agent-meter-darwin-{aarch64,x86_64}.tar.gz` | `agent-meter-windows-x86_64.exe.zip` |
| `agent-meter-proxy` | `agent-meter-proxy-linux-{x86_64,arm64}.tar.gz` | `agent-meter-proxy-darwin-{aarch64,x86_64}.tar.gz` | `agent-meter-proxy-windows-x86_64.exe.zip` |

macOS artifacts are built on `macos-latest` and appended to the release after Linux/Windows upload.

To enable automated SDK publish on tag, add repository secrets `NPM_TOKEN_3` and `PYPI_TOKEN`.

## SDK registries

After the GitHub release:

```bash
source /path/to/.env_creds   # NPM_TOKEN_3, PYPI_TOKEN
bash scripts/ci/publish-sdks.sh
```

- npm: `@dnorio/agent-meter`
- PyPI: `agentmeter-obs`, `dnorio-agent-meter`

## Manual fallback

If CI release fails:

```bash
USE_CROSS=1 bash scripts/ci/release-build.sh
bash scripts/ci/release-smoke.sh dist/
export GITHUB_TOKEN=$(gh auth token)
bash scripts/ci/release-publish.sh v0.1.x
```
