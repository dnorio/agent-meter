#!/usr/bin/env bash
# publish-sdks.sh — publish Node + Python SDKs to npm and PyPI
set -euo pipefail

log() { printf '[publish-sdks] %s\n' "$*'; }

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DRY_RUN="${DRY_RUN:-false}"

publish_npm() {
  local sdk="$ROOT/sdk/node"
  [[ -n "${NPM_TOKEN:-}" ]] || {
    log "ERROR: NPM_TOKEN required for npm publish"
    return 1
  }
  cd "$sdk"
  npm ci
  npm run build
  if [[ "$DRY_RUN" == "true" ]]; then
    log "DRY_RUN — would npm publish from $sdk"
    npm pack --dry-run
    return 0
  fi
  printf '//registry.npmjs.org/:_authToken=%s\n' "$NPM_TOKEN" > "$sdk/.npmrc"
  trap 'rm -f "$sdk/.npmrc"' RETURN
  npm publish --access public
  log "✓ npm publish OK"
}

publish_pypi() {
  local sdk="$ROOT/sdk/python"
  [[ -n "${PYPI_TOKEN:-}" ]] || {
    log "ERROR: PYPI_TOKEN required for PyPI publish"
    return 1
  }
  cd "$sdk"
  python3 -m pip install -q --upgrade build twine 2>/dev/null || pip install -q --upgrade build twine
  rm -rf dist build *.egg-info
  python3 -m build
  if [[ "$DRY_RUN" == "true" ]]; then
    log "DRY_RUN — would twine upload from $sdk/dist"
    ls -la dist/
    return 0
  fi
  TWINE_USERNAME=__token__ TWINE_PASSWORD="$PYPI_TOKEN" \
    python3 -m twine upload --non-interactive dist/*
  log "✓ PyPI publish OK"
}

failed=0
publish_npm || failed=1
publish_pypi || failed=1
exit "$failed"
