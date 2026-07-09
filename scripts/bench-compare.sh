#!/usr/bin/env bash
# =============================================================================
# COSMOSTRIX COMPETITOR BENCHMARK
# =============================================================================
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# =============================================================================
# Side-by-side comparison of cosmostrix vs cmatrix (and unimatrix if
# available).
#
# Cosmostrix has a built-in headless benchmark (--benchmark --json) that
# measures raw engine throughput without a terminal. This script runs it
# and parses the JSON for FPS, RSS, CPU%, and dirty-cell ratio.
#
# cmatrix and unimatrix do NOT have headless benchmark modes — they are
# purely interactive terminal applications. For those, this script runs
# them under a PTY with /usr/bin/time (if available) to measure CPU +
# RSS during a fixed-duration interactive run. If /usr/bin/time is not
# available, only cosmostrix data is reported.
#
# Output: Markdown table on stdout, suitable for pasting into
# benchmark/README.md.
#
# Usage:
#   ./scripts/bench-compare.sh [OPTIONS]
#
# Options:
#   --duration N       Benchmark duration in seconds (default: 5)
#   --cosmostrix PATH  Path to cosmostrix binary
#   --no-build         Skip cargo build
#   --profile NAME     Build profile (default: release)
#   --help             Show this help
# =============================================================================

set -euo pipefail

DURATION=5
COSMOSTRIX_BIN=""
NO_BUILD=false
PROFILE="release"

usage() {
    cat <<'EOF'
Usage: bench-compare.sh [OPTIONS]

Side-by-side resource comparison: cosmostrix (headless benchmark) vs
cmatrix/unimatrix (interactive, if /usr/bin/time available).

Options:
  --duration N         Benchmark duration in seconds (default: 5)
  --cosmostrix PATH    Path to cosmostrix binary
  --no-build           Skip cargo build step
  --profile NAME       Build profile (default: release)
  --help               Show this help

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

# ── Preflight ───────────────────────────────────────────────────────────────

if [[ -z "$COSMOSTRIX_BIN" ]]; then
    COSMOSTRIX_BIN="target/${PROFILE}/cosmostrix"
    if [[ ! -x "$COSMOSTRIX_BIN" ]] && [[ "$PROFILE" == "release" ]]; then
        # Default cargo release path
        COSMOSTRIX_BIN="target/release/cosmostrix"
    fi
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

if ! "$COSMOSTRIX_BIN" --version &>/dev/null; then
    echo "ERROR: cosmostrix binary broken" >&2
    exit 1
fi

# Warn if the binary version doesn't match Cargo.toml — likely a stale build.
BINARY_VERSION=$("$COSMOSTRIX_BIN" --version 2>&1 | head -1 | grep -oP 'v\d+\.\d+\.\d+' | head -1)
CARGO_VERSION=$(grep -m1 '^version' Cargo.toml | grep -oP '\d+\.\d+\.\d+')
if [[ -n "$BINARY_VERSION" ]] && [[ -n "$CARGO_VERSION" ]] && [[ "$BINARY_VERSION" != "v$CARGO_VERSION" ]]; then
    echo "WARNING: cosmostrix binary is $BINARY_VERSION but Cargo.toml is v$CARGO_VERSION" >&2
    echo "         The binary is stale. Run: cargo build --profile $PROFILE" >&2
    echo "         Continuing with stale binary..." >&2
    echo "" >&2
fi

HAVE_TIME=false
if [[ -x /usr/bin/time ]]; then
    HAVE_TIME=true
fi

# Robust binary detection: try multiple methods.
# 'command -v' can fail in some bash contexts (e.g. when the binary is
# a shell function in the parent zsh but not in bash). We fall back to
# 'type -P' (bash builtin, external commands only), 'which', and direct
# path checks.
find_binary() {
    local name="$1"
    # Method 1: command -v (POSIX)
    local p
    p=$(command -v "$name" 2>/dev/null) && [[ -x "$p" ]] && { echo "$p"; return 0; }
    # Method 2: type -P (bash builtin, finds external only)
    p=$(type -P "$name" 2>/dev/null) && [[ -n "$p" ]] && { echo "$p"; return 0; }
    # Method 3: which
    p=$(which "$name" 2>/dev/null) && [[ -x "$p" ]] && { echo "$p"; return 0; }
    # Method 4: direct path checks
    for dir in /usr/bin /usr/local/bin /bin /opt/homebrew/bin /home/linuxbrew/.linuxbrew/bin; do
        if [[ -x "$dir/$name" ]]; then
            echo "$dir/$name"
            return 0
        fi
    done
    return 1
}

CMATRIX_PATH=""
UNIMATRIX_PATH=""
if p=$(find_binary cmatrix); then
    HAVE_CMATRIX=true
    CMATRIX_PATH="$p"
else
    HAVE_CMATRIX=false
fi
if p=$(find_binary unimatrix); then
    HAVE_UNIMATRIX=true
    UNIMATRIX_PATH="$p"
else
    HAVE_UNIMATRIX=false
fi

# Debug: show what was detected
echo "Detection:" >&2
echo "  /usr/bin/time: $([ "$HAVE_TIME" == "true" ] && echo "found" || echo "NOT found")" >&2
echo "  cmatrix: $([ "$HAVE_CMATRIX" == "true" ] && echo "$CMATRIX_PATH" || echo "NOT found")" >&2
echo "  unimatrix: $([ "$HAVE_UNIMATRIX" == "true" ] && echo "$UNIMATRIX_PATH" || echo "NOT found")" >&2
echo "  PATH=$PATH" >&2
echo "" >&2

# ── 1. Cosmostrix headless benchmark ────────────────────────────────────────

echo "" >&2
echo "Running cosmostrix headless benchmark (${DURATION}s)..." >&2

COSMOSTRIX_JSON=$("$COSMOSTRIX_BIN" --benchmark --bench-duration "$DURATION" --json 2>/dev/null)

# Parse JSON fields (use python3 if available, else grep/sed)
parse_json() {
    local json="$1"
    local key="$2"
    if command -v python3 &>/dev/null; then
        python3 -c "
import json, sys
d = json.loads('''$json''')
keys = '$key'.split('.')
v = d
for k in keys:
    v = v.get(k, '') if isinstance(v, dict) else v
print(v)
" 2>/dev/null
    else
        # Fallback: regex extract
        echo "$json" | grep -oP "\"$key\"\s*:\s*\"?\K[^\",}]+" | head -1
    fi
}

COSMOSTRIX_FPS=$(parse_json "$COSMOSTRIX_JSON" "performance.avg_fps")
COSMOSTRIX_PEAK_FPS=$(parse_json "$COSMOSTRIX_JSON" "performance.peak_fps")
COSMOSTRIX_RSS=$(parse_json "$COSMOSTRIX_JSON" "memory.peak_rss")
COSMOSTRIX_CPU=$(parse_json "$COSMOSTRIX_JSON" "cpu.avg_cpu_percent")
COSMOSTRIX_P99=$(parse_json "$COSMOSTRIX_JSON" "performance.p99_frame_time_ms")
COSMOSTRIX_DIRTY=$(parse_json "$COSMOSTRIX_JSON" "performance.active_frame_ratio_percent")
COSMOSTRIX_FRAMES=$(parse_json "$COSMOSTRIX_JSON" "timing.total_frames")

# ── 2. cmatrix / unimatrix interactive (if /usr/bin/time available) ─────────

run_interactive() {
    local label="$1"
    shift
    local cmd="$1"
    shift
    local args=("$@")

    if [[ "$HAVE_TIME" != "true" ]]; then
        echo "${label}  —       —       —       (/usr/bin/time not installed)"
        return
    fi

    local time_log
    time_log="$(mktemp)"

    # Run inside a PTY (via 'script') so terminal-aware tools actually render.
    # Use 'timeout' INSIDE the PTY to kill the tool after DURATION seconds.
    # Previous approach ('cmd; sleep N') was broken: if the tool exited early,
    # the sleep consumed the remaining wall time with ~0 CPU, making it look
    # like the tool used no CPU. Now timeout sends SIGINT directly to the tool.
    # Set TERM + COLUMNS + LINES so the tool can initialize its screen.
    if command -v script &>/dev/null; then
        TERM=xterm-256color COLUMNS=120 LINES=40 \
        /usr/bin/time -v -o "$time_log" \
            script -qec "TERM=xterm-256color COLUMNS=120 LINES=40 timeout --signal=INT ${DURATION} ${cmd} ${args[*]}" /dev/null \
            >/dev/null 2>&1 || true
    else
        TERM=xterm-256color COLUMNS=120 LINES=40 \
        /usr/bin/time -v -o "$time_log" \
            timeout --signal=INT $((DURATION + 2)) \
            "$cmd" "${args[@]}" \
            >/dev/null 2>&1 || true
    fi

    local cpu_user cpu_sys cpu_total rss_kb
    cpu_user=$(grep "User time (seconds)" "$time_log" 2>/dev/null | awk '{print $NF}' || echo "0")
    cpu_sys=$(grep "System time (seconds)" "$time_log" 2>/dev/null | awk '{print $NF}' || echo "0")
    cpu_total=$(awk "BEGIN {printf \"%.2f\", $cpu_user + $cpu_sys}" 2>/dev/null || echo "n/a")
    rss_kb=$(grep "Maximum resident set size" "$time_log" 2>/dev/null | awk '{print $NF}' || echo "0")
    rm -f "$time_log"

    echo "${label}      ${cpu_total}    ${rss_kb}"
}

echo "Checking cmatrix..." >&2
CMATRIX_RESULT="(not installed)"
if [[ "$HAVE_CMATRIX" == "true" ]]; then
    CMATRIX_RESULT=$(run_interactive "cmatrix" "$CMATRIX_PATH" "-s")
fi

echo "Checking unimatrix..." >&2
UNIMATRIX_RESULT="(not installed)"
if [[ "$HAVE_UNIMATRIX" == "true" ]]; then
    UNIMATRIX_RESULT=$(run_interactive "unimatrix" "$UNIMATRIX_PATH")
fi

# ── Output ──────────────────────────────────────────────────────────────────

echo ""
echo "## Competitor Comparison — Engine Throughput & Resource Usage"
echo ""
echo "### Cosmostrix (headless benchmark — raw engine throughput)"
echo ""
echo "Measured with \`cosmostrix --benchmark --bench-duration ${DURATION} --json\`."
echo "This is the **engine** throughput — frames computed per second without"
echo "terminal I/O. Interactive FPS is bounded by the terminal emulator."
echo ""
echo "| Metric | Value |"
echo "|--------|------:|"
echo "| Avg FPS (headless) | ${COSMOSTRIX_FPS} |"
echo "| Peak FPS (headless) | ${COSMOSTRIX_PEAK_FPS} |"
echo "| Total frames in ${DURATION}s | ${COSMOSTRIX_FRAMES} |"
echo "| p99 frame time (ms) | ${COSMOSTRIX_P99} |"
echo "| Active frame ratio (%) | ${COSMOSTRIX_DIRTY} |"
echo "| Peak RSS | ${COSMOSTRIX_RSS} |"
echo "| Avg CPU (%) | ${COSMOSTRIX_CPU} |"
echo ""
echo "### Competitors (interactive, /usr/bin/time)"
echo ""
if [[ "$HAVE_TIME" != "true" ]]; then
    echo "**Note**: \`/usr/bin/time\` not installed — interactive comparison skipped."
    echo "Install with: \`apt install time\` (Debian/Ubuntu) or \`pacman -S time\` (Arch)."
    echo ""
else
    echo "Measured under PTY (\`script\`) with \`/usr/bin/time -v\`, ${DURATION}s per tool."
    echo "Lower is better for CPU time and RSS. cmatrix/unimatrix do not have"
    echo "headless benchmark modes, so only resource usage is comparable."
    echo ""
    echo "| Tool | CPU time (s) | Peak RSS (KiB) | Peak RSS (MiB) |"
    echo "|------|--------------:|---------------:|---------------:|"

    IFS=$'\t' read -r label cpu rss <<< "$CMATRIX_RESULT"
    if [[ "$label" == "cmatrix" ]]; then
        rss_mib=$(awk "BEGIN {printf \"%.1f\", $rss / 1024}" 2>/dev/null || echo "—")
        echo "| cmatrix | $cpu | $rss | $rss_mib |"
    else
        echo "| cmatrix | — | — | (not installed) |"
    fi

    IFS=$'\t' read -r label cpu rss <<< "$UNIMATRIX_RESULT"
    if [[ "$label" == "unimatrix" ]]; then
        rss_mib=$(awk "BEGIN {printf \"%.1f\", $rss / 1024}" 2>/dev/null || echo "—")
        echo "| unimatrix | $cpu | $rss | $rss_mib |"
    else
        echo "| unimatrix | — | — | (not installed) |"
    fi
fi
echo ""
echo "### Interpretation"
echo ""
echo "- **Cosmostrix headless FPS**: raw engine throughput. This is what the"
echo "  diff-based rendering pipeline can compute per second. Real interactive"
echo "  FPS is bounded by terminal I/O, but a higher headless FPS means more"
echo "  headroom for visual effects (glitch, phosphor, depth-of-field)."
echo "- **cmatrix/unimatrix**: full-redraw engines. They do not expose a"
echo "  headless benchmark, so only resource usage (CPU + RSS) is directly"
echo "  comparable. In interactive mode, their CPU usage will be higher"
echo "  because they re-emit every cell every frame."
echo "- **Active frame ratio**: percentage of frames where cosmostrix's"
echo "  diff-based engine actually had work to do. Lower = more efficient"
echo "  (more frames where the diff was empty or trivial)."
echo ""
echo "### Environment"
echo ""
echo "- Date: \`$(date -u +%Y-%m-%dT%H:%M:%SZ)\`"
echo "- Host: \`$(uname -n 2>/dev/null || hostname 2>/dev/null || echo 'unknown')\`"
echo "- Kernel: \`$(uname -sr)\`"
echo "- CPU: \`$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo 'unknown')\`"
echo "- Cosmostrix: \`$("$COSMOSTRIX_BIN" --version 2>&1 | head -1)\`"
echo ""
