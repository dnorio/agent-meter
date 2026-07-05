#!/usr/bin/env bash
# oss-scrub-check.sh — fail CI if SaaS/monorepo leaks appear in the OSS tree (AMOSS gate)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

# Scan tracked + common source paths (exclude target/, .git/)
mapfile -t files < <(
  git ls-files \
    ':!:target/*' ':!docs/assets/*' \
    | grep -E '\.(rs|html|md|sh|ps1|toml|json|yml|yaml|groovy)$|^(install\.(sh|ps1)|Jenkinsfile|Dockerfile)$' || true
)

FORBIDDEN=(
  'agent-meter\.dnor\.io'
  'dnorio/agent-meter'
  'github\.com/dnorio'
  'ghcr\.io/toolhq'
  'agent-meter-worktree-(cursor|copilot|opencode|antigravity|rust-rover)'
  '~/REDACTED-worktree'
  'founders@agent-meter\.com'
)

# CI infra (not product SaaS) — allowed in these paths only
ALLOW_JENKINS='(README\.md|Jenkinsfile|\.github/workflows/|scripts/ci/github-status\.sh)'

fail=0
for pat in "${FORBIDDEN[@]}"; do
  hits=$(grep -EIn "$pat" "${files[@]}" 2>/dev/null | grep -Ev "$ALLOW_JENKINS" || true)
  if [[ -n "$hits" ]]; then
    echo "❌ forbidden pattern /$pat/:"
    echo "$hits" | head -20
    fail=1
  fi
done

# UI must not link to removed SaaS routes
ui_hits=$(grep -En 'href="/(pricing|setup|login|billing)"' crates/collector/ui/*.html 2>/dev/null || true)
if [[ -n "$ui_hits" ]]; then
  echo "❌ SaaS UI routes in HTML:"
  echo "$ui_hits"
  fail=1
fi

if [[ $fail -ne 0 ]]; then
  echo ""
  echo "Fix leaks or update scripts/ci/oss-scrub-check.sh allowlist intentionally."
  exit 1
fi

echo "✓ oss-scrub-check: no SaaS/monorepo leaks detected (${#files[@]} files scanned)"
