#!/usr/bin/env bash
# Gate: every REST endpoint mounted by the collector must be described in the OpenAPI spec.
#
# `build_spec()` is written by hand, so a new route can be mounted without a matching entry and
# the spec drifts silently. This check extracts the routes from the router modules, extracts the
# paths declared in the spec and fails on anything that is mounted but undocumented.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTES_DIR="${ROOT}/crates/collector/src/routes"
SPEC="${ROUTES_DIR}/openapi.rs"

# Deliberately outside the REST surface described by the spec:
#   /badge/{kind}  the spec documents the two concrete badges; the handler answers 404 for any
#                  other kind, so a wildcard path would document something that does not exist
#   /reports       HTML page (every /reports/<name> data endpoint is checked)
ALLOWLIST=("/badge/{kind}" "/reports")

# Routes are flattened first: several are mounted as `.route(` with the path on the next line.
mounted="$(
  cat "${ROUTES_DIR}"/*.rs \
    | tr '\n' ' ' \
    | grep -oE '\.route\( *"/[^"]+"' \
    | sed -E 's/.*"([^"]+)"/\1/' \
    | grep -E '^/(api|reports|health|events|v1|badge)(/|$)' \
    | sort -u
)"

documented="$(
  grep -oE '"/[a-zA-Z0-9/{}._-]+":' "${SPEC}" \
    | sed -E 's/"([^"]+)":/\1/' \
    | sort -u
)"

missing=""
total=0
while IFS= read -r route; do
  [ -n "${route}" ] || continue
  keep=1
  for skip in "${ALLOWLIST[@]}"; do
    [ "${route}" = "${skip}" ] && keep=0
  done
  [ "${keep}" -eq 1 ] || continue
  total=$((total + 1))
  if ! printf '%s\n' "${documented}" | grep -qxF "${route}"; then
    missing="${missing}    ${route}"$'\n'
  fi
done <<< "${mounted}"

if [ -n "${missing}" ]; then
  count="$(printf '%s' "${missing}" | grep -c .)"
  echo "✗ OpenAPI spec is missing ${count} REST endpoint(s):"
  printf '%s' "${missing}"
  echo "  Describe them in build_spec(): crates/collector/src/routes/openapi.rs"
  exit 1
fi

echo "✓ OpenAPI spec covers all ${total} REST endpoints"
