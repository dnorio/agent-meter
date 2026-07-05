#!/usr/bin/env bash
# history-audit.sh — scan full git history for leaks before making repo public (AMOSS gate)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

COMMITS="$(git rev-list --all 2>/dev/null | wc -l | tr -d ' ')"
echo "=== agent-meter history audit ($COMMITS commits, $(date -Iseconds)) ==="

# Patterns that MUST be zero before going public
FORBIDDEN=(
  'agent-meter\.dnor\.io'
  'ghp_[A-Za-z0-9]{20,}'
  'github_pat_[A-Za-z0-9_]{20,}'
  'glpat-[A-Za-z0-9_-]{20,}'
  'AKIA[0-9A-Z]{16}'
  'sk-ant-[A-Za-z0-9-]{20,}'
  'BEGIN (RSA |OPENSSH |EC )?PRIVATE KEY'
  'xox[baprs]-[A-Za-z0-9-]{10,}'
)

# Organizational leaks — allowed in current tree only via oss-scrub-check allowlist;
# in history they must be scrubbed before public (see docs/pre-public-audit.md)
ORG_LEAKS=(
  'dnorio/agent-meter'
  'ghcr\.io/toolhq'
  'agent-meter-worktree-(cursor|copilot|opencode|antigravity|rust-rover|ops)'
  'founders@agent-meter\.com'
  '~/REDACTED-worktree'
)

fail=0

count_commits() {
  local pat="$1"
  git log --all -G"$pat" --oneline 2>/dev/null | wc -l | tr -d ' '
}

echo ""
echo "--- Hard secrets (must be 0 everywhere) ---"
for pat in "${FORBIDDEN[@]}"; do
  n=$(count_commits "$pat")
  if [[ "$n" -gt 0 ]]; then
    echo "❌ $pat → $n commit(s)"
    git log --all -G"$pat" --oneline 2>/dev/null | head -5 | sed 's/^/    /'
    fail=1
  else
    echo "✓  $pat → 0"
  fi
done

echo ""
echo "--- Organizational / SaaS leaks in history (scrub before public) ---"
org_hits=0
for pat in "${ORG_LEAKS[@]}"; do
  n=$(count_commits "$pat")
  if [[ "$n" -gt 0 ]]; then
    echo "⚠️  $pat → $n commit(s)"
    org_hits=$((org_hits + n))
  else
    echo "✓  $pat → 0"
  fi
done

echo ""
echo "--- Sensitive filenames ever committed ---"
if git log --all --name-only --pretty=format: | sort -u | grep -qE '^\.env$|\.pem$|\.key$|credentials\.json|id_rsa$'; then
  echo "❌ sensitive filenames found in history"
  git log --all --name-only --pretty=format: | sort -u | grep -E '^\.env$|\.pem$|\.key$|credentials\.json|id_rsa$' | sed 's/^/    /'
  fail=1
else
  echo "✓  no .env / .pem / private keys in history"
fi

echo ""
echo "--- Current tree (oss-scrub-check) ---"
if bash "$ROOT/scripts/ci/oss-scrub-check.sh"; then
  :
else
  fail=1
fi

echo ""
if [[ $fail -ne 0 ]]; then
  echo "❌ history-audit FAILED — do not make repo public"
  exit 1
fi

if [[ $org_hits -gt 0 ]]; then
  echo "⚠️  history-audit: no hard secrets, but $org_hits org-leak commit touch(es) remain in history"
  echo "    Run git-filter-repo before public — see docs/pre-public-audit.md"
  exit 2
fi

echo "✓ history-audit PASSED — history clean for public release"
exit 0
