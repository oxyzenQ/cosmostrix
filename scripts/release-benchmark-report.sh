#!/usr/bin/env bash
# =============================================================================
# COSMOSTRIX RELEASE BENCHMARK REPORT
# =============================================================================
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# =============================================================================
# Runs N benchmark iterations, parses results, and prints a Markdown section
# suitable for pasting into benchmark/README.md.
#
# Usage:
#   ./scripts/release-benchmark-report.sh <VERSION> [OPTIONS]
#
# Options:
#   --runs N         Number of benchmark runs (default: 5)
#   --bin PATH       Path to binary (default: target/.../pro-linux-v3/cosmostrix)
#   --profile NAME   Build profile name (default: pro-linux-v3)
#   --no-build       Skip cargo build
#   --help           Show this help
#
# Output goes to stdout.  Review before copying into benchmark/README.md.
# =============================================================================

set -euo pipefail

# ── Defaults ────────────────────────────────────────────────────────────────

VERSION=""
RUNS=5
NO_BUILD=false
PROFILE="pro-linux-v3"
BIN="target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix"

# ── Argument parsing ────────────────────────────────────────────────────────

usage() {
    cat <<'EOF'
Usage: release-benchmark-report.sh <VERSION> [OPTIONS]

Generate a Markdown benchmark report section for benchmark/README.md.

Arguments:
  VERSION                Target version (e.g. 4.9.0)

Options:
  --runs N              Number of benchmark runs (default: 5)
  --bin PATH            Binary path (default: target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix)
  --profile NAME        Build profile (default: pro-linux-v3)
  --no-build            Skip cargo build step
  --help                Show this help

Output is printed to stdout.  Review before adding to benchmark/README.md.
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --runs)
            RUNS="$2"; shift 2 ;;
        --bin)
            BIN="$2"; shift 2 ;;
        --profile)
            PROFILE="$2"; shift 2 ;;
        --no-build)
            NO_BUILD=true; shift ;;
        --help|-h)
            usage ;;
        -*)
            echo "ERROR: Unknown option: $1" >&2; exit 1 ;;
        *)
            if [[ -z "$VERSION" ]]; then
                VERSION="$1"; shift
            else
                echo "ERROR: Unexpected argument: $1" >&2; exit 1
            fi
            ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo "ERROR: Version argument is required. Use --help for usage." >&2
    exit 1
fi

# ── Build ───────────────────────────────────────────────────────────────────

if [[ "$NO_BUILD" == "false" ]]; then
    echo "Building $PROFILE binary..." >&2
    if ! command -v cargo &>/dev/null; then
        echo "ERROR: cargo not found in PATH" >&2
        exit 1
    fi
    cargo "$PROFILE" >&2
fi

# ── Verify binary exists and version matches ────────────────────────────────

if [[ ! -x "$BIN" ]]; then
    echo "ERROR: Binary not found or not executable: $BIN" >&2
    exit 1
fi

BINARY_VERSION_OUTPUT=$("$BIN" -V 2>&1 || true)
if ! echo "$BINARY_VERSION_OUTPUT" | grep -q "v${VERSION}"; then
    echo "ERROR: Binary version does not contain v${VERSION}" >&2
    echo "  Binary output: $BINARY_VERSION_OUTPUT" >&2
    exit 1
fi

# Extract commit hash from version output
COMMIT_HASH=$(echo "$BINARY_VERSION_OUTPUT" | grep -oP '\(\K[a-f0-9]{7,}' || echo "unknown")

# ── Parse a single benchmark run ────────────────────────────────────────────

# Extracts a field value from benchmark output.
# Usage: extract_field "output text" "field_name"
extract_field() {
    local output="$1"
    local field="$2"
    echo "$output" | grep -oP "${field}: \K[^\s]+" | head -1
}

# ── Collect benchmark runs ─────────────────────────────────────────────────

declare -a AVG_FPS MEDIAN_FPS P95_FT P99_FT STABILITY DIRTY_RATIO STREAMS
declare -a ACT_EXEC TERM_WRITER COMPUTE_PAR

for ((i = 1; i <= RUNS; i++)); do
    echo "Run $i/$RUNS..." >&2
    RUN_OUTPUT=$("$BIN" --benchmark 2>&1)

    a_fps=$(extract_field "$RUN_OUTPUT" "avg_fps")
    m_fps=$(extract_field "$RUN_OUTPUT" "median_fps")
    p95=$(extract_field "$RUN_OUTPUT" "p95_frame_time" | sed 's/ms$//')
    p99=$(extract_field "$RUN_OUTPUT" "p99_frame_time" | sed 's/ms$//')
    stab=$(extract_field "$RUN_OUTPUT" "frame_time_stability")
    dirty=$(extract_field "$RUN_OUTPUT" "avg_dirty_cell_ratio_percent" | sed 's/%$//')
    streams=$(extract_field "$RUN_OUTPUT" "active_streams_avg")
    a_exec=$(extract_field "$RUN_OUTPUT" "actual_execution")
    t_writer=$(extract_field "$RUN_OUTPUT" "terminal_writer")
    c_par=$(extract_field "$RUN_OUTPUT" "compute_parallelism")

    # Validate required fields
    for label in "avg_fps=$a_fps" "median_fps=$m_fps" "p95=$p95" "p99=$p99" \
                 "stability=$stab" "dirty=$dirty" "streams=$streams" \
                 "actual_execution=$a_exec" "terminal_writer=$t_writer" \
                 "compute_parallelism=$c_par"; do
        val="${label#*=}"
        if [[ -z "$val" ]]; then
            echo "ERROR: Missing required field in run $i: ${label%%=*}" >&2
            exit 1
        fi
    done

    # Validate invariants
    if [[ "$a_exec" != "single-threaded-renderer" ]]; then
        echo "ERROR: Run $i: actual_execution is '$a_exec', expected 'single-threaded-renderer'" >&2
        exit 1
    fi
    if [[ "$t_writer" != "single-owner" ]]; then
        echo "ERROR: Run $i: terminal_writer is '$t_writer', expected 'single-owner'" >&2
        exit 1
    fi
    if [[ "$c_par" != "disabled" ]]; then
        echo "ERROR: Run $i: compute_parallelism is '$c_par', expected 'disabled'" >&2
        exit 1
    fi

    # Warn if stability is not excellent
    if [[ "$stab" != "excellent" ]]; then
        echo "WARNING: Run $i: frame_time_stability is '$stab', not 'excellent'" >&2
    fi

    AVG_FPS+=("$a_fps")
    MEDIAN_FPS+=("$m_fps")
    P95_FT+=("$p95")
    P99_FT+=("$p99")
    STABILITY+=("$stab")
    DIRTY_RATIO+=("$dirty")
    STREAMS+=("$streams")
    ACT_EXEC+=("$a_exec")
    TERM_WRITER+=("$t_writer")
    COMPUTE_PAR+=("$c_par")

    # Brief pause between runs
    if ((i < RUNS)); then
        sleep 3
    fi
done

# ── Compute summary statistics ─────────────────────────────────────────────

compute_mean() {
    local sum=0
    local count=$#
    for val in "$@"; do
        # Use awk for floating-point arithmetic
        sum=$(awk "BEGIN {print $sum + $val}")
    done
    awk "BEGIN {printf \"%.1f\", $sum / $count}"
}

find_min() {
    local min="$1"
    for val in "$@"; do
        if awk "BEGIN {exit !($val < $min)}"; then
            min="$val"
        fi
    done
    echo "$min"
}

find_max() {
    local max="$1"
    for val in "$@"; do
        if awk "BEGIN {exit !($val > $max)}"; then
            max="$val"
        fi
    done
    echo "$max"
}

MEAN_FPS=$(compute_mean "${AVG_FPS[@]}")
P95_MIN=$(find_min "${P95_FT[@]}")
P99_MIN=$(find_min "${P99_FT[@]}")
P99_MAX=$(find_max "${P99_FT[@]}")

# ── Get today's date ────────────────────────────────────────────────────────

TODAY=$(date -u +%Y-%m-%d)

# ── Print Markdown report section ──────────────────────────────────────────

echo "## v${VERSION} — <RELEASE TITLE>"
echo ""
echo "Release benchmark from \`${PROFILE}\` binary"
echo "(commit \`${COMMIT_HASH}\`, ${TODAY}). Default 120x40 terminal size."
echo ""
echo "- Binary version: \`${BINARY_VERSION_OUTPUT%%$'\n'*}\`"
echo "- Commit: \`${COMMIT_HASH}\`"
echo "- Profile: \`${PROFILE}\` (linux-amd64-v3)"
echo "- Run count: ${RUNS}"
echo ""
echo "### ${RUNS}-Run Table"
echo ""
echo "| Run | Avg FPS | Median FPS | P95 frame time | P99 frame time | Stability | Dirty ratio | Active streams |"
echo "|-----|--------:|-----------:|---------------:|---------------:|-----------|------------:|---------------:|"

for ((i = 0; i < RUNS; i++)); do
    run_num=$((i + 1))
    echo "| ${run_num} | ${AVG_FPS[$i]} | ${MEDIAN_FPS[$i]} | ${P95_FT[$i]} ms | ${P99_FT[$i]} ms | ${STABILITY[$i]} | ${DIRTY_RATIO[$i]}% | ${STREAMS[$i]} |"
done

echo ""
echo "- **Mean avg_fps**: ${MEAN_FPS}"
echo "- **P95 range**: ${P95_MIN}–$(find_max "${P95_FT[@]}") ms"
echo "- **P99 range**: ${P99_MIN}–${P99_MAX} ms"
echo ""
echo "### Invariants"
echo ""
echo "| Field | Value |"
echo "|-------|-------|"
echo "| \`actual_execution\` | \`${ACT_EXEC[0]}\` |"
echo "| \`terminal_writer\` | \`${TERM_WRITER[0]}\` |"
echo "| \`compute_parallelism\` | \`${COMPUTE_PAR[0]}\` |"
echo "| \`frame_time_stability\` | \`${STABILITY[0]}\` (all ${RUNS} runs) |"
echo "| \`avg_dirty_cell_ratio\` | ${DIRTY_RATIO[0]}% (all ${RUNS} runs) |"
echo "| \`active_streams_avg\` | ${STREAMS[0]} (all ${RUNS} runs) |"
echo ""
echo "### Notes"
echo ""
echo "- This benchmark measures the **default renderer workload** (cosmic rain"
echo "  animation at 120x40).  Heavy message or matrix-mode workloads are not"
echo "  comparable to the default benchmark and will yield different FPS numbers."
echo "- The 50k FPS lab target was **not reached** and is **not promised**."
echo "- \`terminal_writer\` remains \`single-owner\`: terminal writes are never"
echo "  parallelized."
echo "- \`compute_parallelism\` remains \`disabled\`: no parallel frame computation."
echo "- \`actual_execution\` remains \`single-threaded-renderer\`: the renderer executes"
echo "  on a single thread in benchmark mode."
echo ""
echo "These numbers are local measurements on a single machine, not a portable"
echo "promise.  Benchmark FPS is **synthetic uncapped throughput** — it measures how"
echo "many frames the renderer can compute per second in a tight loop, not the FPS"
echo "the user will see at runtime.  Treat stability, p95, and p99 as far more"
echo "important than raw FPS."