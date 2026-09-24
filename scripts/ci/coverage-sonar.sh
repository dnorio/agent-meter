#!/usr/bin/env bash
# Generate LCOV for Sonar (official sonar.rust.lcov.reportPaths).
# SQLite unit/lib tests — no Postgres required.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

COVERAGE_MIN_LINES="${COVERAGE_MIN_LINES:-0}"
OUT_DIR="${ROOT}/target/coverage"
LCOV_RAW="${LCOV_RAW:-/tmp/agent-meter-oss.lcov}"
LCOV_OUT="${OUT_DIR}/lcov.info"

mkdir -p "$OUT_DIR"

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
	echo "[coverage-sonar] installing cargo-llvm-cov + llvm-tools"
	rustup component add llvm-tools-preview
	cargo install cargo-llvm-cov --locked
fi

echo "[coverage-sonar] running llvm-cov (collector + db)"
# Ignore entrypoints / unused backends so Sonar LCOV matches sonar.coverage.exclusions
# (Rust analyzer does not reliably honor coverage.exclusions for imported LCOV).
cargo llvm-cov -p agent-meter-collector -p agent-meter-db --lib --tests \
	--ignore-filename-regex 'tests/postgres|/bin/|/ui/|postgres\.rs$|demo\.rs$|/main\.rs$|telemetry\.rs$|collector/src/lib\.rs$' \
	--lcov --output-path "$LCOV_RAW" \
	-- --test-threads="${RUST_TEST_THREADS:-1}" --skip postgres

python3 <<PY
import re
src = ${LCOV_RAW@Q}
dst = ${LCOV_OUT@Q}
text = open(src).read()

def fix_sf(m):
    path = m.group(1)
    marker = "/agent-meter/"
    if marker in path:
        tail = path.split(marker, 1)[1]
        if tail.startswith("crates/") or tail.startswith("target/"):
            path = tail
    else:
        idx = path.find("crates/")
        if idx >= 0:
            path = path[idx:]
    return "SF:" + path

text = re.sub(r"^SF:(.+)$", fix_sf, text, flags=re.M)

def drop_excluded(text: str) -> str:
    """Remove entire SF...end_of_record blocks for coverage-excluded paths."""
    out = []
    cur = []
    drop = False
    for line in text.splitlines(True):
        if line.startswith("SF:"):
            if cur and not drop:
                out.extend(cur)
            cur = [line]
            path = line[3:].strip()
            drop = (
                "/ui/" in path
                or path.endswith(".js")
                or path.endswith(".css")
                or "/tests/" in path
                or "_tests.rs" in path
                or "/bin/" in path
                or path.endswith("/main.rs")
                or path.endswith("/postgres.rs")
                or path.endswith("/demo.rs")
                or path.endswith("/telemetry.rs")
                or path.endswith("/collector/src/lib.rs")
            )
            continue
        cur.append(line)
        if line.startswith("end_of_record"):
            if not drop:
                out.extend(cur)
            cur = []
            drop = False
    if cur and not drop:
        out.extend(cur)
    return "".join(out)

text = drop_excluded(text)
open(dst, "w").write(text)
print("[coverage-sonar] remapped+filtered SF ->", dst)

min_pct = float(${COVERAGE_MIN_LINES@Q})
cur = None
hit = miss = 0
with open(dst) as f:
    for line in f:
        if line.startswith("SF:"):
            cur = line[3:].strip()
            continue
        m = re.match(r"DA:(\d+),(\d+)", line)
        if not m or not cur:
            continue
        if "crates/" not in cur:
            continue
        cnt = int(m.group(2))
        if cnt == 0:
            miss += 1
        else:
            hit += 1
total = hit + miss
pct = (100.0 * hit / total) if total else 0.0
print("[coverage-sonar] LCOV lines %.4f%% (hit=%d miss=%d total=%d)" % (pct, hit, miss, total))
if min_pct > 0 and pct < min_pct:
    raise SystemExit("[coverage-sonar] FAIL %.4f%% < %.2f%%" % (pct, min_pct))
print("[coverage-sonar] OK ->", dst, "(min gate=%.2f%%; Sonar QG owns new-code bar)" % min_pct)
PY
