#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="$ROOT_DIR/benchmark"

DEBUG_BIN="$ROOT_DIR/target/debug/cosmostrix"
RELEASE_BIN="$ROOT_DIR/target/release/cosmostrix"

DURATION_SECS="${DURATION_SECS:-30}"

have() { command -v "$1" >/dev/null 2>&1; }

get_jobs() {
  local jobs
  jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || true)"
  if [[ -z "${jobs}" ]]; then
    jobs="$(nproc 2>/dev/null || true)"
  fi
  if [[ -z "${jobs}" ]]; then
    jobs="$(sysctl -n hw.ncpu 2>/dev/null || true)"
  fi
  if [[ -z "${jobs}" ]]; then
    jobs=2
  fi
  echo "${jobs}"
}

echo "=== Cosmostrix Benchmark (${DURATION_SECS}s limit) ==="

echo "[1/5] Build (debug + release)"
JOBS="$(get_jobs)"
(cargo build --profile dev --jobs "${JOBS}") >/dev/null
(cargo build --profile release --jobs "${JOBS}") >/dev/null

echo "[2/5] Hyperfine (if available)"
if have hyperfine; then
  if hyperfine --help 2>/dev/null | grep -q -- "--time-limit"; then
    hyperfine \
      --warmup 3 \
      --runs 10 \
      --time-limit "$DURATION_SECS" \
      "$DEBUG_BIN --duration $DURATION_SECS" \
      "$RELEASE_BIN --duration $DURATION_SECS" \
      --export-markdown "$BENCH_DIR/hyperfine.md"
  else
    echo "- hyperfine does not support --time-limit; falling back to fewer runs."
    hyperfine \
      --warmup 1 \
      --runs 3 \
      "$DEBUG_BIN --duration $DURATION_SECS" \
      "$RELEASE_BIN --duration $DURATION_SECS" \
      --export-markdown "$BENCH_DIR/hyperfine.md"
  fi
else
  echo "- hyperfine not found; skipping."
fi

echo "[3/5] /usr/bin/time -v (if available)"
if [ -x /usr/bin/time ]; then
  /usr/bin/time -v "$DEBUG_BIN" --duration "$DURATION_SECS" >"$BENCH_DIR/time-debug.txt" 2>&1 || true
  /usr/bin/time -v "$RELEASE_BIN" --duration "$DURATION_SECS" >"$BENCH_DIR/time-release.txt" 2>&1 || true
else
  echo "- /usr/bin/time not found; skipping."
fi

echo "[4/5] perf stat (if available)"
if have perf; then
  perf stat -d "$RELEASE_BIN" --duration "$DURATION_SECS" 2>"$BENCH_DIR/perf-release.txt" || true
  perf stat -d "$DEBUG_BIN" --duration "$DURATION_SECS" 2>"$BENCH_DIR/perf-debug.txt" || true
else
  echo "- perf not found; skipping."
fi

echo "[5/5] Valgrind Massif (if available)"
if have valgrind; then
  valgrind --tool=massif \
    --time-unit=ms \
    --max-snapshots=100 \
    --massif-out-file="$BENCH_DIR/massif-30s.out" \
    "$RELEASE_BIN" --duration "$DURATION_SECS" >/dev/null 2>&1 || true
else
  echo "- valgrind not found; skipping."
fi

echo "Done. Outputs written under: $BENCH_DIR"
