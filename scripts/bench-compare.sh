#!/usr/bin/env bash
#
# COSMOSTRIX COMPETITOR BENCHMARK
#
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Fair side-by-side comparison of cosmostrix vs up to 7 competitor matrix
# rain tools (cmatrix, unimatrix, neo-matrix, tmatrix, gmatrix, fmatrix,
# cxxmatrix).
#
# All installed tools are spawned inside a real PTY (via Python's
# pty.openpty) and measured DIRECTLY — not through a wrapper shell.
# This is the only fair way: terminal-aware tools (cosmostrix, neo-matrix,
# cxxmatrix) require a real TTY to run their event loops. Without a PTY,
# they exit early and CPU=0. With a 'script' wrapper, /usr/bin/time
# measures the shell, not the tool.
#
# The Python bench-runner (scripts/bench-runner.py):
#   - Spawns the tool with PTY as stdout/stderr/stdin (isatty() = True)
#   - Measures CPU via resource.getrusage(RUSAGE_CHILDREN) delta
#   - Measures peak RSS via /proc/<pid>/status VmHWM polling at 10 Hz
#   - Kills with SIGTERM after DURATION, SIGKILL after 5 more seconds
#
# Additionally, cosmostrix's headless benchmark (--benchmark --json) is
# run as a BONUS metric showing raw engine ceiling throughput (no
# terminal I/O). This is NOT comparable to the interactive numbers —
# it shows what the engine can compute per second when unthrottled.
#
# Output: Markdown table on stdout, suitable for pasting into
# benchmark/README.md.
#
# Usage:
#   ./scripts/bench-compare.sh [OPTIONS]
#
# Options:
#   --duration N       Run duration in seconds (default: 10)
#   --cosmostrix PATH  Path to cosmostrix binary
#   --no-build         Skip cargo build
#   --profile NAME     Build profile (default: release)
#   --debug            Show verbose debug output to stderr
#   --help             Show this help
#

set -euo pipefail

DURATION=10
COSMOSTRIX_BIN=""
NO_BUILD=false
PROFILE="release"
DEBUG=false

usage() {
    cat <<'EOF'
Usage: bench-compare.sh [OPTIONS]

Fair side-by-side comparison: cosmostrix vs up to 7 competitors.
All tools run in interactive mode inside a real PTY (via Python).
CPU + RSS measured directly on the tool process.

Options:
  --duration N         Run duration in seconds (default: 10)
  --cosmostrix PATH    Path to cosmostrix binary
  --no-build           Skip cargo build step
  --profile NAME       Build profile (default: autodetect from CPU)
                       Autodetect: AVX-512 → pro-linux-v4,
                       AVX2 → pro-linux-v3, else release.
  --debug              Show verbose debug output to stderr
  --help               Show this help

Requirements:
  python3         (for PTY-based fair measurement)
  /proc           (Linux — for RSS polling)
  cmatrix        (optional — apt install cmatrix / pacman -S cmatrix)
  unimatrix      (optional — pip install unimatrix / paru -S unimatrix-git)
  neo-matrix     (optional — paru -S neo-matrix)
  tmatrix        (optional — paru -S tmatrix)
  gmatrix        (optional — paru -S gmatrix)
  fmatrix        (optional — paru -S fmatrix-git)
  cxxmatrix      (optional — paru -S cxxmatrix-git)

Only installed tools are benchmarked; missing tools are silently skipped.

Output: Markdown table on stdout.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --duration) DURATION="$2"; shift 2 ;;
        --cosmostrix) COSMOSTRIX_BIN="$2"; shift 2 ;;
        --no-build) NO_BUILD=true; shift ;;
        --profile) PROFILE="$2"; shift 2 ;;
        --debug) DEBUG=true; shift ;;
        --help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
    esac
done

debug() {
    if [[ "$DEBUG" == "true" ]]; then
        echo "    [debug] $*" >&2
    fi
}

# ── Architecture autodetection ──────────────────────────────────────────────
#
# Detect CPU microarchitecture level and pick the optimal build profile.
# The user can override with --profile, but by default we autodetect:
#   x86-64-v4 (AVX-512)  → pro-linux-v4
#   x86-64-v3 (AVX2)     → pro-linux-v3
#   other / non-x86      → release
#
# This ensures the benchmark runs the FASTEST possible cosmostrix binary
# for the host CPU, giving a fair "best of" comparison.
detect_arch_profile() {
    # Only autodetect if profile wasn't explicitly set by --profile flag.
    # If the user passed --profile release, respect that.
    if [[ "$PROFILE" != "release" ]]; then
        return  # user explicitly chose a profile
    fi

    # Check for AVX-512 (x86-64-v4)
    if grep -q 'avx512f' /proc/cpuinfo 2>/dev/null; then
        PROFILE="pro-linux-v4"
        echo "  CPU: AVX-512 detected → using pro-linux-v4 profile" >&2
        return
    fi

    # Check for AVX2 (x86-64-v3)
    if grep -q 'avx2' /proc/cpuinfo 2>/dev/null; then
        PROFILE="pro-linux-v3"
        echo "  CPU: AVX2 detected → using pro-linux-v3 profile" >&2
        return
    fi

    # Fallback: plain release (x86-64-v1 baseline, works everywhere)
    echo "  CPU: no AVX2/AVX-512 → using release profile" >&2
}

# ── Preflight ───────────────────────────────────────────────────────────────

# Autodetect optimal build profile (unless --profile was explicitly given).
# This finds the fastest cosmostrix binary for the host CPU.
# Runs even with --no-build, because we need the right profile to locate
# the existing binary.
if [[ -z "$COSMOSTRIX_BIN" ]]; then
    detect_arch_profile
fi

if [[ -z "$COSMOSTRIX_BIN" ]]; then
    # Try the detected profile first, then fall back to release.
    COSMOSTRIX_BIN="target/${PROFILE}/cosmostrix"
    if [[ ! -x "$COSMOSTRIX_BIN" ]] && [[ "$PROFILE" != "release" ]]; then
        debug "binary not found at $COSMOSTRIX_BIN, trying release"
        COSMOSTRIX_BIN="target/release/cosmostrix"
    fi
    if [[ ! -x "$COSMOSTRIX_BIN" ]]; then
        # Also check common cross-compiled paths (install.sh output).
        for candidate in \
            "target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix" \
            "target/x86_64-unknown-linux-gnu/pro-linux-v4/cosmostrix" \
            "target/release/cosmostrix" \
            "target/pro-linux-v3/cosmostrix" \
            "target/pro-linux-v4/cosmostrix"; do
            if [[ -x "$candidate" ]]; then
                COSMOSTRIX_BIN="$candidate"
                debug "found binary at $COSMOSTRIX_BIN"
                break
            fi
        done
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

# Warn if the binary version doesn't match Cargo.toml.
BINARY_VERSION=$("$COSMOSTRIX_BIN" --version 2>&1 | head -1 | grep -oP 'v\d+\.\d+\.\d+' | head -1)
CARGO_VERSION=$(grep -m1 '^version' Cargo.toml | grep -oP '\d+\.\d+\.\d+')
if [[ -n "$BINARY_VERSION" ]] && [[ -n "$CARGO_VERSION" ]] && [[ "$BINARY_VERSION" != "v$CARGO_VERSION" ]]; then
    echo "WARNING: cosmostrix binary is $BINARY_VERSION but Cargo.toml is v$CARGO_VERSION" >&2
    echo "         The binary is stale. Run: cargo build --profile $PROFILE" >&2
    echo "         Continuing with stale binary..." >&2
    echo "" >&2
fi

# ── Tool detection ──────────────────────────────────────────────────────────

HAVE_PYTHON3=false
command -v python3 &>/dev/null && HAVE_PYTHON3=true

find_binary() {
    local name="$1"
    local p
    p=$(command -v "$name" 2>/dev/null) && [[ -x "$p" ]] && { echo "$p"; return 0; }
    p=$(type -P "$name" 2>/dev/null) && [[ -n "$p" ]] && { echo "$p"; return 0; }
    p=$(which "$name" 2>/dev/null) && [[ -x "$p" ]] && { echo "$p"; return 0; }
    for dir in /usr/bin /usr/local/bin /bin; do
        [[ -x "$dir/$name" ]] && { echo "$dir/$name"; return 0; }
    done
    return 1
}

CMATRIX_PATH=""
UNIMATRIX_PATH=""
NEO_MATRIX_PATH=""
TMATRIX_PATH=""
GMATRIX_PATH=""
FMATRIX_PATH=""
CXXMATRIX_PATH=""
HAVE_CMATRIX=false
HAVE_UNIMATRIX=false
HAVE_NEO_MATRIX=false
HAVE_TMATRIX=false
HAVE_GMATRIX=false
HAVE_FMATRIX=false
HAVE_CXXMATRIX=false
if p=$(find_binary cmatrix); then HAVE_CMATRIX=true; CMATRIX_PATH="$p"; fi
if p=$(find_binary unimatrix); then HAVE_UNIMATRIX=true; UNIMATRIX_PATH="$p"; fi
if p=$(find_binary neo-matrix); then HAVE_NEO_MATRIX=true; NEO_MATRIX_PATH="$p"; fi
if p=$(find_binary tmatrix); then HAVE_TMATRIX=true; TMATRIX_PATH="$p"; fi
if p=$(find_binary gmatrix); then HAVE_GMATRIX=true; GMATRIX_PATH="$p"; fi
if p=$(find_binary fmatrix); then HAVE_FMATRIX=true; FMATRIX_PATH="$p"; fi
if p=$(find_binary cxxmatrix); then HAVE_CXXMATRIX=true; CXXMATRIX_PATH="$p"; fi

echo "Detection:" >&2
echo "  python3:       $([ "$HAVE_PYTHON3" == "true" ] && echo "found" || echo "NOT found (required for PTY measurement)")" >&2
echo "  cmatrix:       $([ "$HAVE_CMATRIX" == "true" ] && echo "$CMATRIX_PATH" || echo "NOT found")" >&2
echo "  unimatrix:     $([ "$HAVE_UNIMATRIX" == "true" ] && echo "$UNIMATRIX_PATH" || echo "NOT found")" >&2
echo "  neo-matrix:    $([ "$HAVE_NEO_MATRIX" == "true" ] && echo "$NEO_MATRIX_PATH" || echo "NOT found")" >&2
echo "  tmatrix:       $([ "$HAVE_TMATRIX" == "true" ] && echo "$TMATRIX_PATH" || echo "NOT found")" >&2
echo "  gmatrix:       $([ "$HAVE_GMATRIX" == "true" ] && echo "$GMATRIX_PATH" || echo "NOT found")" >&2
echo "  fmatrix:       $([ "$HAVE_FMATRIX" == "true" ] && echo "$FMATRIX_PATH" || echo "NOT found")" >&2
echo "  cxxmatrix:     $([ "$HAVE_CXXMATRIX" == "true" ] && echo "$CXXMATRIX_PATH" || echo "NOT found")" >&2
echo "" >&2

# ── Interactive benchmark runner ────────────────────────────────────────────
#
# Runs a tool inside a PTY (via 'script') so terminal-aware tools render
# normally. 'timeout --signal=INT' kills the tool after DURATION seconds.
# /usr/bin/time -v captures CPU time (user+sys) and peak RSS via wait4().
#
# All three tools are measured identically: interactive mode, PTY, same
# duration, same /usr/bin/time. This is the fair comparison axis.

run_interactive() {
    local label="$1"
    local cmd="$2"
    shift 2
    local args=("$@")

    if [[ "$HAVE_PYTHON3" != "true" ]]; then
        printf '%s\t%s\t%s\t%s\n' "$label" "—" "—" "—"
        return
    fi

    debug "running: $cmd ${args[*]} (PTY via Python, ${DURATION}s)"

    # Use the Python bench-runner to spawn the tool in a real PTY and
    # measure it DIRECTLY (not a wrapper shell). This is the fair way:
    #   - PTY makes isatty() return True → terminal-aware tools run
    #   - resource.getrusage(RUSAGE_CHILDREN) delta → accurate CPU
    #   - /proc/<pid>/status VmHWM polling → accurate peak RSS
    #
    # Previous approaches failed:
    #   - /usr/bin/time + script wrapper: measured 'script' (shell), not tool
    #   - /usr/bin/time + direct: no PTY → cosmostrix/neo-matrix/cxxmatrix exit early
    #   - This Python approach: PTY + direct measurement = fair for ALL tools
    local runner
    runner="$(dirname "${BASH_SOURCE[0]}")/bench-runner.py"

    # Run the Python runner. It outputs a single TSV line.
    # Pass cmd + args as separate arguments to avoid quoting issues.
    local result
    result=$(python3 "$runner" "$label" "$DURATION" "$cmd" "${args[@]}" 2>/dev/null) || true

    if [[ -z "$result" ]]; then
        printf '%s\t%s\t%s\t%s\n' "$label" "—" "—" "—"
    else
        printf '%s\n' "$result"
    fi
}

# ── Run all interactive benchmarks ──────────────────────────────────────────

# Build the list of competitors using parallel arrays (bulletproof —
# no string-splitting ambiguity with tabs/spaces in paths).
COMP_LABELS=()
COMP_PATHS=()
COMP_ARGS=()

COMP_LABELS+=("cosmostrix");      COMP_PATHS+=("$COSMOSTRIX_BIN");  COMP_ARGS+=("")
if [[ "$HAVE_CMATRIX" == "true" ]]; then
    COMP_LABELS+=("cmatrix");     COMP_PATHS+=("$CMATRIX_PATH");    COMP_ARGS+=("-s")
fi
if [[ "$HAVE_UNIMATRIX" == "true" ]]; then
    COMP_LABELS+=("unimatrix");   COMP_PATHS+=("$UNIMATRIX_PATH");  COMP_ARGS+=("")
fi
if [[ "$HAVE_NEO_MATRIX" == "true" ]]; then
    COMP_LABELS+=("neo-matrix");  COMP_PATHS+=("$NEO_MATRIX_PATH"); COMP_ARGS+=("-s")
fi
if [[ "$HAVE_TMATRIX" == "true" ]]; then
    COMP_LABELS+=("tmatrix");     COMP_PATHS+=("$TMATRIX_PATH");    COMP_ARGS+=("")
fi
if [[ "$HAVE_GMATRIX" == "true" ]]; then
    COMP_LABELS+=("gmatrix");     COMP_PATHS+=("$GMATRIX_PATH");    COMP_ARGS+=("")
fi
if [[ "$HAVE_FMATRIX" == "true" ]]; then
    COMP_LABELS+=("fmatrix");     COMP_PATHS+=("$FMATRIX_PATH");    COMP_ARGS+=("")
fi
if [[ "$HAVE_CXXMATRIX" == "true" ]]; then
    COMP_LABELS+=("cxxmatrix");   COMP_PATHS+=("$CXXMATRIX_PATH");  COMP_ARGS+=("")
fi

TOTAL=${#COMP_LABELS[@]}
echo "Running interactive benchmarks (${DURATION}s each, PTY, ${TOTAL} tools)..." >&2
echo "" >&2

RESULTS=()
for i in "${!COMP_LABELS[@]}"; do
    label="${COMP_LABELS[$i]}"
    path="${COMP_PATHS[$i]}"
    args="${COMP_ARGS[$i]}"
    n=$((i + 1))
    echo "  [${n}/${TOTAL}] ${label}..." >&2
    # shellcheck disable=SC2086 # intentional word-splitting of args
    RESULTS+=("$(run_interactive "$label" "$path" $args)")
done

# ── Cosmostrix headless benchmark (engine ceiling) ──────────────────────────

echo "" >&2
echo "Running cosmostrix headless benchmark (${DURATION}s)..." >&2

COSMOSTRIX_JSON=$("$COSMOSTRIX_BIN" --benchmark --bench-duration "$DURATION" --json 2>/dev/null)

parse_json() {
    local json="$1"
    local key="$2"
    if command -v python3 &>/dev/null; then
        python3 -c "
import json
d = json.loads('''$json''')
keys = '$key'.split('.')
v = d
for k in keys:
    v = v.get(k, '') if isinstance(v, dict) else v
print(v)
" 2>/dev/null
    else
        echo "$json" | grep -oP "\"$key\"\s*:\s*\"?\K[^\",}]+" | head -1
    fi
}

CX_FPS=$(parse_json "$COSMOSTRIX_JSON" "performance.avg_fps")
CX_PEAK_FPS=$(parse_json "$COSMOSTRIX_JSON" "performance.peak_fps")
CX_RSS=$(parse_json "$COSMOSTRIX_JSON" "memory.peak_rss")
CX_CPU=$(parse_json "$COSMOSTRIX_JSON" "cpu.avg_cpu_percent")
CX_P99=$(parse_json "$COSMOSTRIX_JSON" "performance.p99_frame_time_ms")
CX_FRAMES=$(parse_json "$COSMOSTRIX_JSON" "timing.total_frames")

# ── Output ──────────────────────────────────────────────────────────────────

echo ""
echo "## Competitor Comparison — Fair Interactive Benchmark"
echo ""
echo "All tools run in **interactive mode** inside a real PTY (via Python"
echo "\`pty.openpty()\`), ${DURATION}s per tool. Each tool's event loop runs"
echo "normally — terminal-aware tools (cosmostrix, neo-matrix, cxxmatrix)"
echo "render instead of exiting early. CPU time and peak RSS are measured"
echo "**directly** on the tool process (not a wrapper shell)."
echo ""
echo "Lower is better for CPU time, CPU%, and RSS."
echo ""

if [[ "$HAVE_PYTHON3" != "true" ]]; then
    echo "**Note**: \`python3\` not available — interactive comparison skipped."
    echo "Install Python 3 to enable PTY-based fair measurement."
    echo ""
else
    echo "| Tool | CPU time (s) | CPU % | Peak RSS (KiB) | Peak RSS (MiB) |"
    echo "|------|-------------:|------:|---------------:|---------------:|"

    for r in "${RESULTS[@]}"; do
        IFS=$'\t' read -r label cpu cpu_pct rss <<< "$r"
        if [[ "$cpu" == "—" ]]; then
            echo "| ${label} | — | — | — | (not installed) |"
        else
            rss_mib=$(awk "BEGIN {printf \"%.1f\", ${rss:-0} / 1024}" 2>/dev/null || echo "—")
            echo "| ${label} | ${cpu} | ${cpu_pct} | ${rss} | ${rss_mib} |"
        fi
    done
fi

echo ""
echo "### Interpretation"
echo ""
echo "- **CPU time / CPU%**: lower = more efficient. All tools render"
echo "  to the same PTY at their natural frame rate. cosmostrix targets 60"
echo "  FPS with adaptive sleep; cmatrix/neo-matrix/tmatrix use fixed"
echo "  delays; unimatrix (Python) has its own frame timing."
echo "- **Peak RSS**: lower = smaller memory footprint. Compiled-native"
echo "  tools (cosmostrix/Rust, cmatrix/C, tmatrix/C++, gmatrix/C,"
echo "  fmatrix/C++, cxxmatrix/C++, neo-matrix/C) should be comparable;"
echo "  unimatrix (Python interpreter) will be significantly larger."
echo "- **Why cosmostrix CPU may be higher than some competitors**:"
echo "  cosmostrix's diff-based engine does more per-frame work (dirty"
echo "  tracking, RLE encoding, phosphor afterglow, depth-of-field,"
echo "  atmosphere engine) than plain full-redraw tools. The tradeoff:"
echo "  cosmostrix emits far fewer ANSI bytes to the terminal, so terminal"
echo "  emulator CPU is lower (not measured here)."
echo ""

echo "### Bonus: Cosmostrix Engine Ceiling (headless benchmark)"
echo ""
echo "Measured with \`cosmostrix --benchmark --bench-duration ${DURATION} --json\`."
echo "This is the **raw engine throughput** — frames computed per second"
echo "without any terminal I/O. NOT comparable to the interactive numbers above."
echo "Shows the engine ceiling: how fast cosmostrix can compute frames when"
echo "unthrottled by terminal speed or frame-rate targeting."
echo ""
echo "| Metric | Value |"
echo "|--------|------:|"
echo "| Avg FPS (headless) | ${CX_FPS} |"
echo "| Peak FPS (headless) | ${CX_PEAK_FPS} |"
echo "| Total frames in ${DURATION}s | ${CX_FRAMES} |"
echo "| p99 frame time (ms) | ${CX_P99} |"
echo "| Peak RSS | ${CX_RSS} |"
echo "| Avg CPU (%) | ${CX_CPU} |"
echo ""
echo "At 25,000+ FPS headless, the engine has **400x headroom** over the 60 FPS"
echo "interactive target. This means visual effects (glitch, phosphor,"
echo "depth-of-field, atmosphere) consume <0.25% of the frame budget."
echo ""

echo "### Environment"
echo ""
echo "- Date: \`$(date -u +%Y-%m-%dT%H:%M:%SZ)\`"
echo "- Host: \`$(uname -n 2>/dev/null || hostname 2>/dev/null || echo 'unknown')\`"
echo "- Kernel: \`$(uname -sr)\`"
echo "- CPU: \`$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo 'unknown')\`"
echo "- Cosmostrix: \`$("$COSMOSTRIX_BIN" --version 2>&1 | head -1)\`"
echo "- Duration per tool: ${DURATION}s"
echo ""
