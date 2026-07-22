#!/usr/bin/env bash
# install-scripts-check.sh — syntax-check installer scripts
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

for script in install.sh install-proxy.sh install-full.sh; do
  bash -n "${ROOT}/${script}"
  echo "✓ bash -n ${script}"
done

for script in "${ROOT}"/scripts/ci/*.sh; do
  bash -n "${script}"
  echo "✓ bash -n scripts/ci/$(basename "${script}")"
done

echo "✓ all install/ci shell scripts parse cleanly"
