#!/usr/bin/env bash
# release-tag.sh — validate, build, smoke, and upload a release (local maintainer flow)
set -euo pipefail

TAG="${1:-}"
if [[ -z "$TAG" ]]; then
  echo "usage: release-tag.sh v0.1.x" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

export RELEASE_TAG="$TAG"
export USE_CROSS="${USE_CROSS:-1}"
export INSTALL_MINGW="${INSTALL_MINGW:-1}"

bash scripts/ci/release-build.sh
bash scripts/ci/release-smoke.sh dist/

if [[ -z "${GITHUB_TOKEN:-}" && -z "${GH_TOKEN:-}" ]]; then
  export GITHUB_TOKEN="$(gh auth token 2>/dev/null || true)"
fi
bash scripts/ci/release-publish.sh "$TAG"

echo "✓ release $TAG ready — run: bash scripts/ci/publish-sdks.sh"
