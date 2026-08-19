#!/usr/bin/env bash

# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
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

# ── Size sweep mode ───────────────────────────────────────────────────────
# Usage: ./benchmark/benchmark.sh sweep [BIN_PATH]
#        ./benchmark/benchmark.sh sweep --build pro-linux-v4
#
# Benchmarks cosmostrix across a geometric progression of terminal sizes
# from the engine minimum (4x4) up to 8K UHD (7680x4320). Each size tier
# uses adaptive duration (shorter for larger cell grids) to keep total
# wall time reasonable.
#
# Environment overrides:
#   SWEEP_DURATION_SMALL  - seconds for sizes < 5K cells   (default: 5)
#   SWEEP_DURATION_MEDIUM - seconds for sizes 5K-500K cells (default: 3)
#   SWEEP_DURATION_LARGE  - seconds for sizes > 500K cells  (default: 2)
#   SWEEP_SCENE           - scene name (default: monolith)
#   SWEEP_CHARSET         - charset    (default: zen)
#   SWEEP_SCENES          - space-separated list of scenes to sweep (overrides SWEEP_SCENE)
#   SWEEP_OUTPUT_DIR      - output directory (default: benchmark/cloud-xeon)

run_sweep() {
        local bin="${1:-}"
        local needs_build=false

        if [[ "$bin" == "--build" ]]; then
                needs_build=true
                bin="${2:-}"
        fi

        # Resolve binary: explicit path > env > default pro-linux-v4 > release
        if [[ -z "$bin" ]]; then
                bin="${SWEEP_BIN:-}"
        fi
        if [[ -z "$bin" ]]; then
                local v4_bin="$ROOT_DIR/target/x86_64-unknown-linux-gnu/pro-linux-v4/cosmostrix"
                if [[ -x "$v4_bin" ]]; then
                        bin="$v4_bin"
                else
                        bin="$RELEASE_BIN"
                fi
        fi

        if [[ "$needs_build" == true ]]; then
                local profile="${2:-pro-linux-v4}"
                echo "[sweep] Building $profile..."
                local jobs
                jobs="$(get_jobs)"
                case "$profile" in
                        pro-linux-v4)  CARGO_BUILD_JOBS="$jobs" cargo pro-linux-v4  ;;
                        pro-linux-v3)  CARGO_BUILD_JOBS="$jobs" cargo pro-linux-v3  ;;
                        pro)          CARGO_BUILD_JOBS="$jobs" cargo pro          ;;
                        pro-native)   CARGO_BUILD_JOBS="$jobs" cargo pro-native   ;;
                        release)      cargo build --profile release --jobs "$jobs" ;;
                        *)            echo "[sweep] Unknown profile: $profile" >&2; exit 1 ;;
                esac
                # Re-resolve binary after build
                local v4_bin="$ROOT_DIR/target/x86_64-unknown-linux-gnu/pro-linux-v4/cosmostrix"
                [[ -x "$v4_bin" ]] && bin="$v4_bin"
        fi

        if [[ ! -x "$bin" ]]; then
                echo "[sweep] Binary not found or not executable: $bin" >&2
                echo "[sweep] Build first: cargo pro-linux-v4" >&2
                exit 1
        fi

        local dur_s="${SWEEP_DURATION_SMALL:-5}"
        local dur_m="${SWEEP_DURATION_MEDIUM:-3}"
        local dur_l="${SWEEP_DURATION_LARGE:-2}"
        local scene="${SWEEP_SCENE:-monolith}"
        local charset="${SWEEP_CHARSET:-zen}"
        local out_dir="${SWEEP_OUTPUT_DIR:-$BENCH_DIR/cloud-xeon}"
        mkdir -p "$out_dir"

        # Size tiers: "cols lines" pairs from 4x4 (min) to 7680x4320 (8K UHD).
        # Chosen as geometric progression covering practical + stress sizes.
        local -a sizes=(
                "4 4"        # 16 cells     - absolute minimum
                "20 6"       # 120 cells    - tiny
                "80 24"      # 1920 cells   - classic terminal
                "120 40"     # 4800 cells   - default benchmark
                "200 80"     # 16000 cells  - large terminal
                "480 160"    # 76800 cells  - XL
                "960 270"    # 259200 cells - 2K class
                "1920 540"   # 1036800 cells - 4K class
                "3840 1080"  # 4147200 cells - 5K class
                "7680 4320"  # 33177600 cells - 8K UHD
        )

        # Determine scenes to sweep
        local -a scenes=()
        if [[ -n "${SWEEP_SCENES:-}" ]]; then
                read -ra scenes <<< "$SWEEP_SCENES"
        else
                scenes=("$scene")
        fi

        local ts
        ts=$(date +%Y%m%d_%H%M%S)
        local summary_file="$out_dir/sweep_${ts}.md"
        local csv_file="$out_dir/sweep_${ts}.csv"

        echo "# Cosmostrix Size Sweep" > "$summary_file"
        echo "" >> "$summary_file"
        echo "Binary: \`$(basename "$bin")\`" >> "$summary_file"
        echo "Date: \`$(date -Iseconds)\`" >> "$summary_file"
        echo "Profile: \`${SWEEP_BUILD_PROFILE:-pro-linux-v4}\`" >> "$summary_file"
        echo "" >> "$summary_file"

        # CSV header
        echo "size_label,cols,lines,cells,scene,avg_fps,peak_fps,p99_frame_ms,avg_dirty_cells,dirty_ratio_pct,peak_rss_mib,heap_retained_kib,fps_drift_pct,frame_time_stability,avg_frame_ms" > "$csv_file"

        local total_sizes=${#sizes[@]}
        local size_idx=0

        for size_entry in "${sizes[@]}"; do
                size_idx=$((size_idx + 1))
                read -r cols lines <<< "$size_entry"
                local cells=$((cols * lines))
                local label="${cols}x${lines}"
                local cells_fmt
                cells_fmt=$(printf "%'d" "$cells")

                # Adaptive duration based on cell count
                local dur
                if (( cells < 5000 )); then
                        dur="$dur_s"
                elif (( cells < 500000 )); then
                        dur="$dur_m"
                else
                        dur="$dur_l"
                fi

                echo ""
                echo "[$size_idx/$total_sizes] $label ($cells_fmt cells) — ${dur}s each scene"

                for sc in "${scenes[@]}"; do
                        local raw_log="$out_dir/sweep_${label}_${sc}_${ts}.txt"
                        local safe_label="${cols}x${lines}"

                        echo "  scene=$sc ..." >&2
                        COSMOSTRIX_BENCH_COLS="$cols" COSMOSTRIX_BENCH_LINES="$lines" \
                                "$bin" --benchmark --bench-scene lean --scene "$sc" --charset "$charset" \
                                --bench-duration "$dur" > "$raw_log" 2>&1 || true

                        # Parse key metrics from the benchmark output
                        local avg_fps peak_fps p99_ft avg_dirty dirty_ratio peak_rss heap_retained drift stability avg_ft
                        avg_fps=$(awk '/^  avg_fps:/ { print $2; exit }' "$raw_log" || echo "N/A")
                        peak_fps=$(awk '/^  peak_fps:/ { print $2; exit }' "$raw_log" || echo "N/A")
                        p99_ft=$(awk '/^  p99_frame_time:/ { sub(/ms$/,""); print $2; exit }' "$raw_log" || echo "N/A")
                        avg_ft=$(awk '/^  avg_frame_time:/ { sub(/ms$/,""); print $2; exit }' "$raw_log" || echo "N/A")
                        avg_dirty=$(awk '/^  avg_dirty_cells_per_frame:/ { print $2; exit }' "$raw_log" || echo "N/A")
                        dirty_ratio=$(awk '/^  avg_dirty_cell_ratio_percent:/ { sub(/%$/,""); print $2; exit }' "$raw_log" || echo "N/A")
                        peak_rss=$(awk '/^  peak_rss:/ { sub(/MiB$/,""); print $2; exit }' "$raw_log" || echo "N/A")
                        heap_retained=$(awk '/^  heap_retained:/ { sub(/K$/,"KiB"); print $2; exit }' "$raw_log" || echo "N/A")
                        drift=$(awk '/^  fps_drift_percent:/ { print $2; exit }' "$raw_log" || echo "N/A")
                        stability=$(awk '/^  frame_time_stability:/ { print $2; exit }' "$raw_log" || echo "N/A")

                        # Append CSV row
                        echo "${safe_label},${cols},${lines},${cells},${sc},${avg_fps},${peak_fps},${p99_ft},${avg_dirty},${dirty_ratio},${peak_rss},${heap_retained},${drift},${stability},${avg_ft}" >> "$csv_file"

                        # Console progress
                        echo "    avg_fps=$avg_fps  p99=${p99_ft}ms  rss=${peak_rss}MiB  dirty=${avg_dirty}cells"
                done
        done

        echo ""
        echo "[$size_idx/$total_sizes] Done."
        echo ""
        echo "Generating summary table..."

        # Build markdown table from CSV
        echo "## Results" >> "$summary_file"
        echo "" >> "$summary_file"
        echo "| Size | Cells | Scene | Avg FPS | Peak FPS | p99 (ms) | Dirty cells/frame | RSS (MiB) | Stability |" >> "$summary_file"
        echo "|------|------:|-------|--------:|---------:|---------:|------------------:|-----------:|-----------|" >> "$summary_file"

        tail -n +2 "$csv_file" | while IFS=, read -r label cols lines cells scene avg_fps peak_fps p99_ft avg_dirty dirty_ratio peak_rss heap_retained drift stability avg_ft; do
                printf "| \`%s\` | %'d | %s | %s | %s | %s | %s | %s | %s |\n" \
                        "$label" "$cells" "$scene" "$avg_fps" "$peak_fps" "$p99_ft" "$avg_dirty" "$peak_rss" "$stability" >> "$summary_file"
        done

        echo "" >> "$summary_file"
        echo "Raw logs: \`sweep_*_${ts}.txt\` in this directory" >> "$summary_file"
        echo "CSV data: \`sweep_${ts}.csv\`" >> "$summary_file"

        echo ""
        echo "Summary: $summary_file"
        echo "CSV:     $csv_file"
        echo "Logs:    $out_dir/sweep_*_${ts}.txt"
}

# ── Dispatch ───────────────────────────────────────────────────────────────

case "${1:-}" in
        sweep)
                shift
                run_sweep "$@"
                ;;
        "")
                # Default: original single-size benchmark flow
                ;;
        *)
                echo "Usage: $0 [sweep [BIN_PATH | --build PROFILE]]" >&2
                echo "" >&2
                echo "  (no args)  Run the original single-size benchmark" >&2
                echo "  sweep      Run multi-size sweep from 4x4 to 8K" >&2
                echo "  sweep --build pro-linux-v4  Build then sweep" >&2
                exit 1
                ;;
esac

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
