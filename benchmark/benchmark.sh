#!/usr/bin/env bash

# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Uses nproc, command -v, bash arrays. Will not run on Windows.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="$ROOT_DIR/benchmark"
BINARY_NAME="${BINARY_NAME:-cosmostrix}"

# ── CPU feature detection ──────────────────────────────────────────────────
# Returns the best matching cargo build alias for the current host CPU.
# Checks /proc/cpuinfo (Linux), sysctl (macOS/BSD), or falls back to
# generic profiles.
#
# Detection order:
#   AVX-512 (avx512f) → pro-linux-v4
#   AVX2              → pro-linux-v3
#   ARM NEON          → pro-native  (aarch64)
#   Fallback          → pro

auto_detect_build_profile() {
        local arch
        arch=$(uname -m 2>/dev/null || echo "unknown")

        # ARM / AArch64 — use pro-native for -march=native
        if [[ "$arch" == aarch64 || "$arch" == arm64 ]]; then
                echo "pro-native"
                return 0
        fi

        # x86_64 — probe CPU flags
        if [[ "$arch" == x86_64 ]]; then
                local flags=""
                if [[ -f /proc/cpuinfo ]]; then
                        flags=$(awk '/^flags[[:space:]]*:/ { print; exit }' /proc/cpuinfo 2>/dev/null || true)
                elif command -v sysctl >/dev/null 2>&1; then
                        # macOS / BSD fallback
                        flags=$(sysctl -n machdep.cpu.features 2>/dev/null || true)
                fi

                if [[ "$flags" == *avx512f* ]]; then
                        echo "pro-linux-v4"
                        return 0
                elif [[ "$flags" == *avx2* ]]; then
                        echo "pro-linux-v3"
                        return 0
                fi
        fi

        # Generic fallback
        echo "pro"
}

# Print detected CPU info for logging.
print_cpu_info() {
        local arch model flags_line
        arch=$(uname -m 2>/dev/null || echo "unknown")
        model=$(awk '/^model name[[:space:]]*:/ { sub(/^model name[[:space:]]*: */, ""); print; exit }' /proc/cpuinfo 2>/dev/null || true)
        if [[ -z "$model" ]]; then
                model=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "$arch")
        fi

        local feats=""
        if [[ -f /proc/cpuinfo ]]; then
                flags_line=$(awk '/^flags[[:space:]]*:/ { print; exit }' /proc/cpuinfo 2>/dev/null || true)
                # Extract key SIMD features
                feats=$(echo "$flags_line" | grep -oE 'avx512f|avx2|sse4_2|neon' 2>/dev/null | tr '\n' ' ' || true)
        fi

        echo "[auto] Arch: $arch"
        echo "[auto] CPU:  $model"
        [[ -n "$feats" ]] && echo "[auto] SIMD: $feats"
}

# ── Binary resolution ─────────────────────────────────────────────────────
# All binary paths are discovered dynamically. No hardcoded profiles.
#
# Precedence (sweep mode):
#   1. SWEEP_BIN env var
#   2. Positional argument (explicit path)
#   3. probe_bin() — auto-discover under target/
#
# Precedence (default mode):
#   1. BENCH_BIN / BENCH_BIN2 env vars
#   2. probe_bin() — auto-discover under target/

# Detect host target triple (same pattern as scripts/build.sh).
default_target() {
        if command -v rustc >/dev/null 2>&1; then
                local host
                host=$(rustc -vV 2>/dev/null | sed -n 's/^host: //p' || true)
                if [ -n "${host}" ]; then
                        echo "${host}"
                        return 0
                fi
        fi
        echo "x86_64-unknown-linux-gnu"
}

readonly HOST_TARGET="${COSMOSTRIX_TARGET:-$(default_target)}"

# Probe target/ for any built cosmostrix binary.
# Priority: optimized profiles first, then debug.
# Searches both target/<triple>/<profile>/ and target/<profile>/.
probe_bin() {
        local target_root="$ROOT_DIR/target"

        # Profile priority: most optimized first
        local -a profiles=(
                pro-linux-v4 pro-linux-v3 pro-linux-musl
                pro-macos-aarch64-native pro-win-amd64 pro-win-aarch64
                pro-freebsd-amd64 pro-android-aarch64
                pro-native pro
                release
                pgo-use pgo-instrument
                debug
        )

        # Search target/<triple>/<profile>/ first (target-specific builds)
        for profile in "${profiles[@]}"; do
                local candidate="$target_root/$HOST_TARGET/$profile/$BINARY_NAME"
                if [[ -x "$candidate" ]]; then
                        echo "$candidate"
                        return 0
                fi
        done

        # Search target/<profile>/ (default target builds)
        for profile in "${profiles[@]}"; do
                local candidate="$target_root/$profile/$BINARY_NAME"
                if [[ -x "$candidate" ]]; then
                        echo "$candidate"
                        return 0
                fi
        done

        # Last resort: find any executable matching the name under target/
        local found
                found=$(find "$target_root" -type f -name "$BINARY_NAME" -perm -u+x 2>/dev/null | head -1 || true)
        if [[ -n "$found" ]]; then
                echo "$found"
                return 0
        fi

        return 1
}

# Extract profile label from binary path for reporting.
# e.g. target/x86_64-unknown-linux-gnu/pro-linux-v4/cosmostrix → pro-linux-v4
#      /abs/path/target/release/cosmostrix → release
bin_profile_label() {
        local bin="$1"
        local rel
        # Try absolute ROOT_DIR/target/ prefix first
        rel="${bin#"$ROOT_DIR"/target/}"
        # If unchanged, strip leading "target/" or any path up to "/target/"
        if [[ "$rel" == "$bin" ]]; then
                rel="${bin#target/}"
                # Also handle cases like ./target/ or /any/path/target/
                if [[ "$rel" == "$bin" ]]; then
                        rel="${bin##*/target/}"
                fi
        fi
        # Strip leading <triple>/ if present (3+ path components)
        if [[ "$rel" == */*/* ]]; then
                rel="${rel#*/}"
        fi
        echo "${rel%%/*}"
}

# ── Configurable defaults ────────────────────────────────────────────────

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
#        ./benchmark/benchmark.sh sweep --build <cargo-alias>
#
# Benchmarks cosmostrix across a geometric progression of terminal sizes
# from the engine minimum (4x4) up to 8K UHD (7680x4320). Each size tier
# uses adaptive duration (shorter for larger cell grids) to keep total
# wall time reasonable.
#
# Environment overrides:
#   SWEEP_BIN            - explicit binary path (skips auto-detect)
#   SWEEP_BUILD_PROFILE  - profile label for report header (auto-detected if unset)
#   SWEEP_DURATION_SMALL  - seconds for sizes < 5K cells   (default: 5)
#   SWEEP_DURATION_MEDIUM - seconds for sizes 5K-500K cells (default: 3)
#   SWEEP_DURATION_LARGE  - seconds for sizes > 500K cells  (default: 2)
#   SWEEP_SCENE           - scene name (default: monolith)
#   SWEEP_CHARSET         - charset    (default: zen)
#   SWEEP_SCENES          - space-separated scene list (overrides SWEEP_SCENE)
#   SWEEP_OUTPUT_DIR      - output directory (default: benchmark/bench-labs)

run_sweep() {
        local bin="${1:-}"
        local needs_build=false
        local build_alias=""
        local auto_detect=false

        if [[ "$bin" == "--auto" ]]; then
                auto_detect=true
                needs_build=true
                build_alias=$(auto_detect_build_profile)
                bin="${2:-}"
        elif [[ "$bin" == "--build" ]]; then
                needs_build=true
                build_alias="${2:-}"
                if [[ -z "$build_alias" ]]; then
                        echo "[sweep] --build requires a cargo alias (e.g. pro-linux-v4, pro, release)" >&2
                        exit 1
                fi
                bin="${3:-}"
        fi

        # Resolve binary: explicit arg > env > auto-probe
        if [[ -z "$bin" ]]; then
                bin="${SWEEP_BIN:-}"
        fi
        if [[ -z "$bin" ]]; then
                bin=$(probe_bin) || true
        fi

        if [[ "$needs_build" == true ]]; then
                if [[ "$auto_detect" == true ]]; then
                        print_cpu_info
                fi
                echo "[sweep] Building: cargo $build_alias..."
                local jobs
                jobs="$(get_jobs)"
                CARGO_BUILD_JOBS="$jobs" cargo "$build_alias" >/dev/null
                # Re-probe after build
                bin=$(probe_bin) || true
        fi

        if [[ -z "$bin" || ! -x "$bin" ]]; then
                echo "[sweep] No usable binary found." >&2
                echo "[sweep] Build first with any profile, e.g.:" >&2
                echo "[sweep]   cargo pro-linux-v4   # or pro-linux-v3, pro, pro-native, release" >&2
                echo "[sweep]   ./benchmark/benchmark.sh sweep --build pro-linux-v4" >&2
                echo "[sweep]   SWEEP_BIN=./my-custom-binary ./benchmark/benchmark.sh sweep" >&2
                exit 1
        fi

        # Detect profile label for report
        local profile_label="${SWEEP_BUILD_PROFILE:-}"
        if [[ -z "$profile_label" ]]; then
                profile_label=$(bin_profile_label "$bin")
        fi

        echo "[sweep] Binary:  $bin"
        echo "[sweep] Profile: $profile_label"
        if [[ "$auto_detect" == true ]]; then
                echo "[sweep] Auto:    $build_alias (CPU-detected)"
        fi

        local dur_s="${SWEEP_DURATION_SMALL:-5}"
        local dur_m="${SWEEP_DURATION_MEDIUM:-3}"
        local dur_l="${SWEEP_DURATION_LARGE:-2}"
        local scene="${SWEEP_SCENE:-monolith}"
        local charset="${SWEEP_CHARSET:-zen}"
        local out_dir="${SWEEP_OUTPUT_DIR:-$BENCH_DIR/bench-labs}"
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

        local cpu_model=""
        cpu_model=$(awk '/^model name[[:space:]]*:/ { sub(/^model name[[:space:]]*: */, ""); print; exit }' /proc/cpuinfo 2>/dev/null || sysctl -n machdep.cpu.brand_string 2>/dev/null || true)

        {
            echo "<!-- SPDX-License-Identifier: GPL-3.0-only -->"
            echo ""
            echo "# cosmostrix Size Sweep"
            echo ""
            echo "Binary: \`$(basename "$bin")\`"
            echo "Date: \`$(date -Iseconds)\`"
            echo "Profile: \`${profile_label}\`"
            echo "Target: \`${HOST_TARGET}\`"
            [[ -n "$cpu_model" ]] && echo "CPU: \`${cpu_model}\`"
            echo ""
        } > "$summary_file"

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

                        echo "  scene=$sc ..." >&2
                        COSMOSTRIX_BENCH_COLS="$cols" COSMOSTRIX_BENCH_LINES="$lines" \
                                "$bin" --benchmark --bench-scene lean --scene "$sc" --charset "$charset" \
                                --bench-duration "$dur" > "$raw_log" 2>&1 || true

                        # Parse key metrics from the benchmark output
                        local avg_fps peak_fps p99_ft avg_ft avg_dirty dirty_ratio
                        local peak_rss heap_retained drift stability
                        avg_fps=$(awk '/^  avg_fps:/ { print $2; exit }' "$raw_log" || echo "N/A")
                        peak_fps=$(awk '/^  peak_fps:/ { print $2; exit }' "$raw_log" || echo "N/A")
                        p99_ft=$(awk '/^  p99_frame_time:/ { sub(/ms$/,""); print $2; exit }' "$raw_log" || echo "N/A")
                        avg_ft=$(awk '/^  avg_frame_time:/ { sub(/ms$/,""); print $2; exit }' "$raw_log" || echo "N/A")
                        avg_dirty=$(awk '/^  avg_dirty_cells_per_frame:/ { print $2; exit }' "$raw_log" || echo "N/A")
                        dirty_ratio=$(awk '/^  avg_dirty_cell_ratio_percent:/ { sub(/%$/,""); print $2; exit }' "$raw_log" || echo "N/A")
                        # peak_rss: Rust outputs N.NN MiB or N.NN GiB via format_rss_kb().
                        # Normalize both to MiB so CSV column is consistent.
                        peak_rss=$(awk '/^  peak_rss:/ {
                                val=$2; unit=$3;
                                if (unit == "GiB") val = val * 1024;
                                printf "%.0f", val; exit
                        }' "$raw_log" || echo "N/A")
                        # heap_retained: Rust outputs humanized bytes (e.g. 86K, 1.16M, 564.10M).
                        # Normalize to KiB (1K=1000 bytes from humanize, 1KiB=1024 bytes).
                        heap_retained=$(awk '/^  heap_retained:/ {
                                val=$2;
                                if (val ~ /K$/) { sub(/K$/,""); printf "%.0f", val * 1000 / 1024; }
                                else if (val ~ /M$/) { sub(/M$/,""); printf "%.0f", val * 1000000 / 1024; }
                                else if (val ~ /B$/) { sub(/B$/,""); printf "%.0f", val / 1024; }
                                else { printf "%.0f", val / 1024; }
                                exit
                        }' "$raw_log" || echo "N/A")
                        drift=$(awk '/^  fps_drift_percent:/ { sub(/%$/,""); print $2; exit }' "$raw_log" || echo "N/A")
                        stability=$(awk '/^  frame_time_stability:/ { print $2; exit }' "$raw_log" || echo "N/A")

                        local safe_label="${cols}x${lines}"
                        echo "${safe_label},${cols},${lines},${cells},${sc},${avg_fps},${peak_fps},${p99_ft},${avg_dirty},${dirty_ratio},${peak_rss},${heap_retained},${drift},${stability},${avg_ft}" >> "$csv_file"

                        echo "    avg_fps=$avg_fps  p99=${p99_ft}ms  rss=${peak_rss}MiB  heap=${heap_retained}KiB  dirty=${avg_dirty}cells"
                done
        done

        echo ""
        echo "[$size_idx/$total_sizes] Done."
        echo ""
        echo "Generating summary table..."

        # Build markdown table from CSV
        {
            echo "## Results"
            echo ""
            echo "| Size | Cells | Scene | Avg FPS | Peak FPS | p99 (ms) | Dirty cells/frame | RSS (MiB) | Stability |"
            echo "|------|------:|-------|--------:|---------:|---------:|------------------:|-----------:|-----------|"
        } >> "$summary_file"

        tail -n +2 "$csv_file" | while IFS=, read -r label cols lines cells scene avg_fps peak_fps p99_ft avg_dirty dirty_ratio peak_rss heap_retained drift stability avg_ft; do
                printf "| \`%s\` | %'d | %s | %s | %s | %s | %s | %s | %s |\n" \
                        "$label" "$cells" "$scene" "$avg_fps" "$peak_fps" "$p99_ft" "$avg_dirty" "$peak_rss" "$stability" >> "$summary_file"
        done

        {
            echo ""
            echo "Raw logs: \`sweep_*_${ts}.txt\` in this directory"
            echo "CSV data: \`sweep_${ts}.csv\`"
        } >> "$summary_file"

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
                exit 0
                ;;
        "")
                # Default: original single-size benchmark flow
                ;;
        *)
                echo "Usage: $0 [sweep [BIN_PATH | --build <cargo-alias> | --auto]]" >&2
                echo "" >&2
                echo "  (no args)                   Run the original single-size benchmark" >&2
                echo "  sweep                       Sweep 4x4 to 8K with auto-detected binary" >&2
                echo "  sweep --auto                Auto-detect CPU, build optimal profile, sweep" >&2
                echo "  sweep --build pro-linux-v4  Build then sweep" >&2
                echo "  sweep --build pro          Build pro then sweep" >&2
                echo "  sweep --build release      Build release then sweep" >&2
                echo "  sweep ./target/pro/cosmostrix  Sweep with explicit binary" >&2
                echo "" >&2
                echo "  --auto detection order:" >&2
                echo "    AVX-512 (avx512f)  -> pro-linux-v4" >&2
                echo "    AVX2              -> pro-linux-v3" >&2
                echo "    ARM NEON          -> pro-native" >&2
                echo "    Fallback          -> pro" >&2
                echo "" >&2
                echo "Environment:" >&2
                echo "  SWEEP_BIN   Override binary path" >&2
                echo "  BENCH_BIN   Override binary (default mode)" >&2
                echo "  COSMOSTRIX_TARGET  Override host target triple" >&2
                exit 1
                ;;
esac

# ── Default mode: original single-size benchmark ─────────────────────────

# Resolve binaries: env override > auto-probe > legacy hardcoded fallback
if [[ -n "${BENCH_BIN:-}" ]]; then
        RELEASE_BIN="$BENCH_BIN"
else
        RELEASE_BIN="${ROOT_DIR}/target/release/$BINARY_NAME"
        if [[ ! -x "$RELEASE_BIN" ]]; then
                RELEASE_BIN=$(probe_bin) || true
        fi
fi

if [[ -n "${BENCH_BIN2:-}" ]]; then
        PRO_NATIVE_BIN="$BENCH_BIN2"
else
        PRO_NATIVE_BIN="${ROOT_DIR}/target/pro/$BINARY_NAME"
        if [[ ! -x "$PRO_NATIVE_BIN" ]]; then
                # Try to find a second distinct binary; if not, use same as first
                PRO_NATIVE_BIN=$(probe_bin) || echo "$RELEASE_BIN"
        fi
fi

echo "=== cosmostrix Benchmark ==="

if [[ "$RELEASE_BIN" == "$PRO_NATIVE_BIN" ]]; then
        echo "[1/5] Build (release)"
        JOBS="$(get_jobs)"
        (cargo build --profile release --jobs "${JOBS}") >/dev/null
        RELEASE_BIN="${ROOT_DIR}/target/release/$BINARY_NAME"
        PRO_NATIVE_BIN="$RELEASE_BIN"
else
        echo "[1/5] Build (release + pro-native)"
        JOBS="$(get_jobs)"
        (cargo build --profile release --jobs "${JOBS}") >/dev/null
        (cargo pro-native --jobs "${JOBS}") >/dev/null
fi

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
