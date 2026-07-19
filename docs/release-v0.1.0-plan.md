# agent-meter v0.1.0 release plan

This is the maintainer checklist for the first public release. It separates the core OSS release from later package/installer work.

## Release gate

All items must pass before tagging `v0.1.0`:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/ci/smoke-demo.sh
bash scripts/ci/history-audit.sh

cd sdk/python && python -m pytest -q
cd ../node && npm ci && npm run build && npm test && npm pack --dry-run

docker build -t agent-meter:v0.1.0-rc .
```

GitHub checks required on the release PR:

- `cargo-test`
- `sdk-python`
- `sdk-node`
- CodeQL checks for actions, JavaScript/TypeScript, Python, and Rust

The CodeQL Rust check may report analysis-quality warnings. Treat that as a release review item: either switch to a maintained advanced CodeQL workflow with an explicit Rust build, or document the warning in the release notes if accepted.

## GitHub release

1. Merge the release-readiness PR.
2. Create and push the tag:

```bash
git checkout main
git pull --ff-only origin main
git tag -a v0.1.0 -m "agent-meter v0.1.0"
git push origin v0.1.0
```

3. Build release assets from a clean checkout:

```bash
TAG_NAME=v0.1.0 bash scripts/ci/release-build.sh
ls -lah dist/
```

4. Publish assets:

```bash
GITHUB_TOKEN=... bash scripts/ci/release-publish.sh v0.1.0
```

Expected first-release assets:

- `agent-meter-linux-x86_64.tar.gz`
- `agent-meter-linux-arm64.tar.gz`
- `agent-meter-windows-x86_64.exe.zip` if the cross toolchain is available

macOS, DEB/RPM, MSI, and signed installers are post-v0.1.0 unless built and tested separately.

## npm package

Only publish after `npm pack --dry-run` includes `dist/` files.

```bash
cd sdk/node
npm ci
npm test
npm pack --dry-run
npm publish --access public
```

## PyPI package

Only publish after building and inspecting the sdist/wheel.

```bash
cd sdk/python
python -m pip install --upgrade build twine
python -m build --sdist --wheel
python -m twine check dist/*
python -m twine upload dist/*
```

## Not in v0.1.0 unless explicitly validated

- Docker image registry publication
- Homebrew, apt, yum, winget, scoop
- DEB/RPM packages
- WiX/MSI installers
- Authenticode signing
- End-to-end live validation against every third-party CLI

For v0.1.0, live capture claims must be phrased as supported paths with known setup constraints, not guaranteed capture of every IDE or CLI.
