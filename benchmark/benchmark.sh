#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="$ROOT_DIR/benchmark"

RELEASE_BIN="$ROOT_DIR/target/release/cosmostrix"
PRO_NATIVE_BIN="$ROOT_DIR/target/pro/cosmostrix"

DURATION_SECS="${DURATION_SECS:-30}"
HYPERFINE_RUNS="${HYPERFINE_RUNS:-3}"
BENCH_FPS="${BENCH_FPS:-60}"
BENCH_COLS="${BENCH_COLS:-120}"
BENCH_LINES="${BENCH_LINES:-40}"
BENCH_TARGET_SECS="${BENCH_TARGET_SECS:-$DURATION_SECS}"
CALIB_FRAMES="${CALIB_FRAMES:-10000}"
BENCH_FRAMES="${BENCH_FRAMES:-}"
MASSIF_FRAMES="${MASSIF_FRAMES:-}"

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

echo "=== Cosmostrix Benchmark ==="

echo "[1/5] Build (release + pro-native)"
JOBS="$(get_jobs)"
(cargo build --profile release --jobs "${JOBS}") >/dev/null
(cargo pro-native --jobs "${JOBS}") >/dev/null

if [[ -z "${BENCH_FRAMES}" ]]; then
	echo "[0/5] Calibrating BENCH_FRAMES (target ~${BENCH_TARGET_SECS}s)"
	CALIB_FPS=$(
		COSMOSTRIX_BENCH_COLS="$BENCH_COLS" COSMOSTRIX_BENCH_LINES="$BENCH_LINES" \
			"$RELEASE_BIN" --fps "$BENCH_FPS" --bench-frames "$CALIB_FRAMES" |
			awk '/frames_per_s:/ { print $2; exit }'
	)
	if [[ -z "${CALIB_FPS}" ]]; then
		echo "failed to calibrate BENCH_FRAMES (could not parse frames_per_s)" >&2
		exit 1
	fi

	BENCH_FRAMES=$(awk -v fps="$CALIB_FPS" -v secs="$BENCH_TARGET_SECS" 'BEGIN { printf "%d", (fps * secs) }')
	if [[ "${BENCH_FRAMES}" -lt 1000 ]]; then
		BENCH_FRAMES=1000
	fi
fi

if [[ -z "${MASSIF_FRAMES}" ]]; then
	MASSIF_FRAMES=$((BENCH_FRAMES / 10 + 1))
fi

echo "--- Settings: ${BENCH_FRAMES} frames @ ${BENCH_FPS} fps (cols=${BENCH_COLS} lines=${BENCH_LINES}) ---"

echo "[2/5] Hyperfine (if available)"
if have hyperfine; then
	HYPERFINE_ARGS=(
		--export-markdown "$BENCH_DIR/hyperfine.md"
		"COSMOSTRIX_BENCH_COLS=$BENCH_COLS COSMOSTRIX_BENCH_LINES=$BENCH_LINES $RELEASE_BIN --fps $BENCH_FPS --bench-frames $BENCH_FRAMES"
		"COSMOSTRIX_BENCH_COLS=$BENCH_COLS COSMOSTRIX_BENCH_LINES=$BENCH_LINES $PRO_NATIVE_BIN --fps $BENCH_FPS --bench-frames $BENCH_FRAMES"
	)

	if hyperfine --time-limit "1s" --runs 1 --warmup 0 "true" >/dev/null 2>&1; then
		hyperfine --warmup 1 --runs "${HYPERFINE_RUNS}" --time-limit "$DURATION_SECS" "${HYPERFINE_ARGS[@]}"
	else
		hyperfine --warmup 1 --runs "${HYPERFINE_RUNS}" "${HYPERFINE_ARGS[@]}"
	fi
else
	echo "- hyperfine not found; skipping."
fi

echo "[3/5] /usr/bin/time -v (if available)"
if [ -x /usr/bin/time ]; then
	COSMOSTRIX_BENCH_COLS="$BENCH_COLS" COSMOSTRIX_BENCH_LINES="$BENCH_LINES" \
		/usr/bin/time -v "$RELEASE_BIN" --fps "$BENCH_FPS" --bench-frames "$BENCH_FRAMES" >"$BENCH_DIR/time-release.txt" 2>&1 || true
	COSMOSTRIX_BENCH_COLS="$BENCH_COLS" COSMOSTRIX_BENCH_LINES="$BENCH_LINES" \
		/usr/bin/time -v "$PRO_NATIVE_BIN" --fps "$BENCH_FPS" --bench-frames "$BENCH_FRAMES" >"$BENCH_DIR/time-pro-native.txt" 2>&1 || true
else
	echo "- /usr/bin/time not found; skipping."
fi

echo "[4/5] perf stat (if available)"
if have perf; then
	COSMOSTRIX_BENCH_COLS="$BENCH_COLS" COSMOSTRIX_BENCH_LINES="$BENCH_LINES" \
		perf stat -d "$RELEASE_BIN" --fps "$BENCH_FPS" --bench-frames "$BENCH_FRAMES" 2>"$BENCH_DIR/perf-release.txt" || true
	COSMOSTRIX_BENCH_COLS="$BENCH_COLS" COSMOSTRIX_BENCH_LINES="$BENCH_LINES" \
		perf stat -d "$PRO_NATIVE_BIN" --fps "$BENCH_FPS" --bench-frames "$BENCH_FRAMES" 2>"$BENCH_DIR/perf-pro-native.txt" || true
else
	echo "- perf not found; skipping."
fi

echo "[5/5] Valgrind Massif (if available)"
if have valgrind; then
	COSMOSTRIX_BENCH_COLS="$BENCH_COLS" COSMOSTRIX_BENCH_LINES="$BENCH_LINES" \
		valgrind --tool=massif \
		--time-unit=ms \
		--max-snapshots=100 \
		--massif-out-file="$BENCH_DIR/massif-release-${MASSIF_FRAMES}f.out" \
		"$RELEASE_BIN" --fps "$BENCH_FPS" --bench-frames "$MASSIF_FRAMES" >/dev/null 2>&1 || true

	COSMOSTRIX_BENCH_COLS="$BENCH_COLS" COSMOSTRIX_BENCH_LINES="$BENCH_LINES" \
		valgrind --tool=massif \
		--time-unit=ms \
		--max-snapshots=100 \
		--massif-out-file="$BENCH_DIR/massif-pro-native-${MASSIF_FRAMES}f.out" \
		"$PRO_NATIVE_BIN" --fps "$BENCH_FPS" --bench-frames "$MASSIF_FRAMES" >/dev/null 2>&1 || true
else
	echo "- valgrind not found; skipping."
fi

echo "Done. Outputs written under: $BENCH_DIR"
