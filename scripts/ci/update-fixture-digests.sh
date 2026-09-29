#!/usr/bin/env bash
# update-fixture-digests.sh — refresh sha256 fields in fixtures/manifest.json
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MANIFEST="$ROOT/crates/collector/tests/fixtures/manifest.json"
FIXDIR="$ROOT/crates/collector/tests/fixtures"

python3 - "$MANIFEST" "$FIXDIR" <<'PY'
import hashlib, json, sys
from pathlib import Path

manifest_path, fixdir = Path(sys.argv[1]), Path(sys.argv[2])
m = json.loads(manifest_path.read_text())
changed = 0
for fx in m.get("fixtures") or []:
    path = fixdir / fx["file"]
    if not path.is_file():
        raise SystemExit(f"missing fixture file: {path}")
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if fx.get("sha256") != digest:
        print(f"  {fx['id']}: {fx.get('sha256', '?')[:12]}… → {digest[:12]}…")
        fx["sha256"] = digest
        changed += 1
    else:
        print(f"  {fx['id']}: ok")
manifest_path.write_text(json.dumps(m, indent=2) + "\n")
print(f"[update-fixture-digests] updated={changed} total={len(m.get('fixtures') or [])}")
PY
