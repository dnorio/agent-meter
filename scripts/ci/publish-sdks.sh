#!/usr/bin/env bash
# publish-sdks.sh — publish Node + Python SDKs to npm and PyPI
set -euo pipefail

log() { printf '[publish-sdks] %s\n' "$*"; }

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DRY_RUN="${DRY_RUN:-false}"
PYPI_NAMES=(agentmeter-obs dnorio-agent-meter)

publish_npm() {
  local sdk="$ROOT/sdk/node"
  local token="${NPM_TOKEN_3:-${NPM_TOKEN_2:-${NPM_TOKEN:-}}}"
  [[ -n "$token" ]] || {
    log "ERROR: NPM_TOKEN_3, NPM_TOKEN_2, or NPM_TOKEN required"
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
  printf '//registry.npmjs.org/:_authToken=%s\n' "$token" > "$sdk/.npmrc"
  trap 'rm -f "$sdk/.npmrc"' RETURN
  local npm_args=(publish --access public)
  [[ -n "${NPM_OTP:-}" ]] && npm_args+=(--otp="$NPM_OTP")
  npm "${npm_args[@]}"
  log "✓ npm publish OK (@dnorio/agent-meter)"
}

publish_pypi_name() {
  local name="$1"
  local sdk="$ROOT/sdk/python"
  local pyproject="$sdk/pyproject.toml"
  local backup
  backup="$(mktemp)"
  cp "$pyproject" "$backup"

  sed -i "s/^name = \".*\"/name = \"$name\"/" "$pyproject"
  cd "$sdk"
  rm -rf dist build *.egg-info
  python3 -m build
  if [[ "$DRY_RUN" == "true" ]]; then
    log "DRY_RUN — would twine upload $name from $sdk/dist"
    ls -la dist/
    mv "$backup" "$pyproject"
    return 0
  fi
  if TWINE_USERNAME=__token__ TWINE_PASSWORD="$PYPI_TOKEN" \
    python3 -m twine upload --non-interactive dist/*; then
    log "✓ PyPI publish OK ($name)"
  elif [[ "${ALLOW_EXISTING:-0}" == "1" ]]; then
    log "WARN: PyPI upload failed for $name (ALLOW_EXISTING=1)"
  else
    log "ERROR: PyPI upload failed for $name"
    mv "$backup" "$pyproject"
    return 1
  fi
  mv "$backup" "$pyproject"
}

publish_pypi() {
  [[ -n "${PYPI_TOKEN:-}" ]] || {
    log "ERROR: PYPI_TOKEN required for PyPI publish"
    return 1
  }
  python3 -m pip install -q --upgrade build twine 2>/dev/null || pip install -q --upgrade build twine
  local name failed=0
  for name in "${PYPI_NAMES[@]}"; do
    publish_pypi_name "$name" || failed=1
  done
  return "$failed"
}

failed=0
publish_npm || failed=1
publish_pypi || failed=1
exit "$failed"
