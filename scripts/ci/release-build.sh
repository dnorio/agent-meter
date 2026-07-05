#!/usr/bin/env bash
# release-build.sh — OSS repo entry (T-365 Fase 3)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export APP_DIR="$ROOT"
export INSTALL_MINGW="${INSTALL_MINGW:-1}"
exec bash "$ROOT/scripts/ci/release-build-core.sh"
