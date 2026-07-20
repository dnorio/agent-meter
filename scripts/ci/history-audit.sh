#!/usr/bin/env bash
# history-audit.sh — scan full git history for secret/credential leaks
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

COMMIT_REFS=(--branches --tags)
COMMITS="$(git rev-list "${COMMIT_REFS[@]}" 2>/dev/null | wc -l | tr -d ' ')"
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

# Optional private/org patterns live outside git by default.
ORG_LEAKS=()
PRIVATE_PATTERNS_FILE="${OSS_PRIVATE_PATTERNS_FILE:-scripts/ci/private-patterns.txt}"
if [[ -f "$PRIVATE_PATTERNS_FILE" ]]; then
  while IFS= read -r pat; do
    [[ -z "$pat" || "$pat" =~ ^[[:space:]]*# ]] && continue
    ORG_LEAKS+=("$pat")
  done < "$PRIVATE_PATTERNS_FILE"
fi

fail=0

# Count commits where pattern appears outside security-audit allowlist paths
count_commits() {
  local pat="$1"
  local n=0
  local commit file
  while read -r commit; do
    [[ -z "$commit" ]] && continue
    local bad=0
    while read -r file; do
      [[ "$file" =~ ^(SECURITY\.md|scripts/ci/) ]] && continue
      bad=1
    done < <(git grep -l -E "$pat" "$commit" 2>/dev/null || true)
    [[ $bad -eq 1 ]] && n=$((n + 1))
  done < <(git log "${COMMIT_REFS[@]}" -G"$pat" --pretty=format:%H 2>/dev/null)
  echo "$n"
}

count_commits_all() {
  local pat="$1"
  git log "${COMMIT_REFS[@]}" -G"$pat" --oneline 2>/dev/null | wc -l | tr -d ' '
}

echo ""
echo "--- Hard secrets (must be 0 everywhere) ---"
for pat in "${FORBIDDEN[@]}"; do
  n=$(count_commits "$pat")
  if [[ "$n" -gt 0 ]]; then
    echo "❌ $pat → $n commit(s)"
    git log "${COMMIT_REFS[@]}" -G"$pat" --oneline 2>/dev/null | head -5 | sed 's/^/    /'
    fail=1
  else
    echo "✓  $pat → 0"
  fi
done

echo ""
echo "--- Organizational/private leaks in history (scrub before public) ---"
org_hits=0
for pat in "${ORG_LEAKS[@]}"; do
  n=$(count_commits "$pat")
  if [[ "$n" -gt 0 ]]; then
    echo "⚠️  private pattern → $n commit(s)"
    org_hits=$((org_hits + n))
  else
    echo "✓  private pattern → 0"
  fi
done

echo ""
echo "--- Sensitive filenames ever committed ---"
if git log "${COMMIT_REFS[@]}" --name-only --pretty=format: | sort -u | grep -qE '^\.env$|\.pem$|\.key$|credentials\.json|id_rsa$'; then
  echo "❌ sensitive filenames found in history"
  git log "${COMMIT_REFS[@]}" --name-only --pretty=format: | sort -u | grep -E '^\.env$|\.pem$|\.key$|credentials\.json|id_rsa$' | sed 's/^/    /'
  fail=1
else
  echo "✓  no .env / .pem / private keys in history"
fi

echo ""
echo "--- Secret scan (full history: gitleaks + all blobs) ---"
if bash "$ROOT/scripts/ci/secret-scan-history.sh"; then
  :
else
  fail=1
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
  echo "    Run git-filter-repo before public, then rerun this audit."
  exit 2
fi

echo "✓ history-audit PASSED — history clean for public release"
exit 0
