#!/usr/bin/env bash
# =============================================================================
# COSMOSTRIX COMPETITOR BENCHMARK
# =============================================================================
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# =============================================================================
# Side-by-side comparison of cosmostrix vs cmatrix (and unimatrix if
# available). Measures CPU time, peak RSS, and elapsed wall time for
# a fixed-duration run. Output is a Markdown table suitable for
# pasting into benchmark/README.md.
#
# Usage:
#   ./scripts/bench-compare.sh [OPTIONS]
#
# Options:
#   --duration N     Run duration in seconds (default: 10)
#   --cosmostrix PATH  Path to cosmostrix binary (default: target/.../cosmostrix)
#   --no-build       Skip cargo build
#   --help           Show this help
#
# Requirements:
#   - /usr/bin/time (GNU time, for RSS measurement)
#   - cmatrix installed (apt install cmatrix / pacman -S cmatrix)
#   - unimatrix installed (pip install unimatrix) — optional
#
# Note: terminal-bound tools (cmatrix, unimatrix, cosmostrix interactive)
# cannot be benchmarked for FPS via subprocess — the FPS is determined by
# the terminal emulator, not the process. This script measures RESOURCE
# USAGE (CPU + RSS) under identical terminal conditions, which is the
# defensible comparison axis.
# =============================================================================

set -euo pipefail

# ── Defaults ────────────────────────────────────────────────────────────────

DURATION=10
COSMOSTRIX_BIN=""
NO_BUILD=false
PROFILE="pro-linux-v3"

# ── Argument parsing ────────────────────────────────────────────────────────

usage() {
    cat <<'EOF'
Usage: bench-compare.sh [OPTIONS]

Side-by-side resource comparison: cosmostrix vs cmatrix vs unimatrix.

Options:
  --duration N         Run duration in seconds (default: 10)
  --cosmostrix PATH    Path to cosmostrix binary
  --no-build           Skip cargo build step
  --profile NAME       Build profile (default: pro-linux-v3)
  --help               Show this help

Requirements:
  /usr/bin/time  (GNU time, for RSS — usually /usr/bin/time on Linux)
  cmatrix        (apt install cmatrix / pacman -S cmatrix)
  unimatrix      (pip install unimatrix) — optional, skipped if absent

Output: Markdown table on stdout.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --duration) DURATION="$2"; shift 2 ;;
        --cosmostrix) COSMOSTRIX_BIN="$2"; shift 2 ;;
        --no-build) NO_BUILD=true; shift ;;
        --profile) PROFILE="$2"; shift 2 ;;
        --help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
    esac
done

# ── Preflight checks ────────────────────────────────────────────────────────

if ! command -v /usr/bin/time &>/dev/null; then
    echo "ERROR: /usr/bin/time (GNU time) not found." >&2
    echo "Install: apt install time  (Debian/Ubuntu)" >&2
    echo "         pacman -S time     (Arch)" >&2
    exit 1
fi

if [[ -z "$COSMOSTRIX_BIN" ]]; then
    COSMOSTRIX_BIN="target/x86_64-unknown-linux-gnu/${PROFILE}/cosmostrix"
fi

if [[ ! -x "$COSMOSTRIX_BIN" ]]; then
    if [[ "$NO_BUILD" == "true" ]]; then
        echo "ERROR: cosmostrix binary not found at: $COSMOSTRIX_BIN" >&2
        exit 1
    fi
    echo "Building cosmostrix ($PROFILE profile)..." >&2
    if ! cargo build --profile "$PROFILE" 2>&1 | tail -5; then
        echo "ERROR: build failed" >&2
        exit 1
    fi
fi

# Verify the binary works
if ! "$COSMOSTRIX_BIN" --version &>/dev/null; then
    echo "ERROR: cosmostrix binary at $COSMOSTRIX_BIN is not executable or broken" >&2
    exit 1
fi

HAVE_CMATRIX=false
HAVE_UNIMATRIX=false

if command -v cmatrix &>/dev/null; then
    HAVE_CMATRIX=true
else
    echo "NOTE: cmatrix not found — install with 'apt install cmatrix' or 'pacman -S cmatrix' to include in comparison" >&2
fi

if command -v unimatrix &>/dev/null; then
    HAVE_UNIMATRIX=true
else
    echo "NOTE: unimatrix not found — install with 'pip install unimatrix' to include in comparison" >&2
fi

# ── Helper: run a tool for N seconds, capture CPU + RSS ─────────────────────

# We use a pseudo-terminal (script) so terminal-aware tools actually
# render (cmatrix and unimatrix detect non-tty stdout and exit early).
# /usr/bin/time -v captures peak RSS via wait4().

run_bench() {
    local label="$1"
    shift
    local cmd="$*"
    local outfile
    outfile="$(mktemp)"

    # Use 'script' to allocate a PTY so terminal-aware tools render.
    # Send 'q' after DURATION seconds to exit cleanly.
    # /usr/bin/time -v wraps the whole thing for RSS.
    local time_log
    time_log="$(mktemp)"

    # Build the command: run inside script, kill after DURATION+2s
    # timeout as a safety net.
    (
        /usr/bin/time -v -o "$time_log" \
            timeout $((DURATION + 2)) \
            script -qec "$cmd; sleep $DURATION" /dev/null \
            >"$outfile" 2>&1 &
        local pid=$!
        sleep "$DURATION"
        kill -INT "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    ) || true

    # Parse /usr/bin/time -v output
    local cpu_sys cpu_user cpu_total rss_kb wall_time
    cpu_user=$(grep "User time (seconds)" "$time_log" 2>/dev/null | awk '{print $NF}' || echo "0")
    cpu_sys=$(grep "System time (seconds)" "$time_log" 2>/dev/null | awk '{print $NF}' || echo "0")
    cpu_total=$(awk "BEGIN {print $cpu_user + $cpu_sys}")
    rss_kb=$(grep "Maximum resident set size" "$time_log" 2>/dev/null | awk '{print $NF}' || echo "0")
    wall_time=$(grep "Elapsed (wall clock) time" "$time_log" 2>/dev/null | \
        sed 's/Elapsed (wall clock) time (.*): //' || echo "0s")

    # Clean up
    rm -f "$outfile" "$time_log"

    # Return as TSV
    printf '%s\t%s\t%s\t%s\t%s\n' "$label" "$cpu_total" "$rss_kb" "$wall_time" "$cmd"
}

# ── Run benchmarks ──────────────────────────────────────────────────────────

echo "" >&2
echo "Running benchmarks (${DURATION}s each)..." >&2
echo "This will open each tool in a PTY for ${DURATION}s. Do not switch terminals." >&2
echo "" >&2

RESULTS=()

# Cosmostrix (interactive mode, default settings)
echo "  [1/3] cosmostrix..." >&2
RESULTS+=("$(run_bench "cosmostrix" "$COSMOSTRIX_BIN")")
sleep 1

# cmatrix (if available)
if [[ "$HAVE_CMATRIX" == "true" ]]; then
    echo "  [2/3] cmatrix..." >&2
    RESULTS+=("$(run_bench "cmatrix" "cmatrix -s")")
    sleep 1
else
    RESULTS+=("cmatrix	—	—	—	(not installed)")
fi

# unimatrix (if available)
if [[ "$HAVE_UNIMATRIX" == "true" ]]; then
    echo "  [3/3] unimatrix..." >&2
    RESULTS+=("$(run_bench "unimatrix" "unimatrix")")
else
    RESULTS+=("unimatrix	—	—	—	(not installed)")
fi

# ── Output Markdown table ───────────────────────────────────────────────────

echo ""
echo "## Competitor Comparison — Resource Usage"
echo ""
echo "Measured with \`/usr/bin/time -v\` inside a PTY (\`script\`), ${DURATION}s per tool."
echo "Lower is better for all columns. CPU time = user + system. RSS = peak resident set."
echo ""
echo "**Important**: terminal-bound renderers cannot be compared for FPS via subprocess —"
echo "FPS is determined by the terminal emulator, not the process. This table measures"
echo "**resource efficiency** (CPU + memory) under identical terminal conditions, which"
echo "is the defensible comparison axis for diff-based vs full-redraw engines."
echo ""
echo "| Tool | CPU time (s) | Peak RSS (KiB) | Peak RSS (MiB) | Wall time |"
echo "|------|-------------:|---------------:|---------------:|----------:|"

for r in "${RESULTS[@]}"; do
    IFS=$'\t' read -r label cpu rss wall cmd <<< "$r"
    if [[ "$rss" == "—" ]]; then
        echo "| $label | — | — | — | — |"
    else
        rss_mib=$(awk "BEGIN {printf \"%.1f\", $rss / 1024}")
        echo "| $label | $cpu | $rss | $rss_mib | $wall |"
    fi
done

echo ""
echo "### Interpretation"
echo ""
echo "- **CPU time**: lower = more efficient rendering engine. Cosmostrix's"
echo "  diff-based engine should use significantly less CPU than cmatrix's"
echo "  full-redraw approach, especially at higher terminal sizes."
echo "- **Peak RSS**: lower = smaller memory footprint. Includes the"
echo "  terminal's PTY buffer, the process's own heap, and any shared"
echo "  libraries."
echo "- **Wall time**: should be ~${DURATION}s for all tools. Significant"
echo "  deviation indicates the tool exited early or hung."
echo ""
echo "### Environment"
echo ""
echo "- Date: \`$(date -u +%Y-%m-%dT%H:%M:%SZ)\`"
echo "- Host: \`$(hostname)\`"
echo "- Kernel: \`$(uname -sr)\`"
echo "- CPU: \`$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo 'unknown')\`"
echo "- Terminal: \`echo \$\{TERM:-unknown\}\`"
echo ""
