#!/usr/bin/env bash
#
# COSMOSTRIX eBPF PROFILER
#
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Attaches uprobes to cosmostrix hot functions and reports timing.
# Requires: bpftrace (pacman -S bpftrace / apt install bpftrace)
# Requires: root (eBPF needs CAP_BPF)
#
# Usage:
#   sudo ./scripts/ebpf-profile.sh /path/to/cosmostrix
#
# What it measures:
#   - frame::set() call count + duration histogram
#   - terminal::draw() call count + duration histogram
#   - flush_ansi() call count + duration histogram
#   - write() syscall count + bytes histogram
#
# Run cosmostrix in another terminal while this runs. Press Ctrl-C to
# stop and see the histograms.
#

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 /path/to/cosmostrix" >&2
    exit 1
fi

BIN="$1"

if [[ ! -x "$BIN" ]]; then
    echo "ERROR: binary not found: $BIN" >&2
    exit 1
fi

if ! command -v bpftrace &>/dev/null; then
    echo "ERROR: bpftrace not installed." >&2
    echo "  Arch:   sudo pacman -S bpftrace" >&2
    echo "  Debian: sudo apt install bpftrace" >&2
    exit 1
fi

if [[ $EUID -ne 0 ]]; then
    echo "ERROR: eBPF requires root. Run with sudo." >&2
    exit 1
fi

echo "Attaching uprobes to: $BIN" >&2
echo "Run cosmostrix in another terminal. Press Ctrl-C to stop." >&2
echo "" >&2

# Note: cosmostrix's Rust function symbols are mangled (e.g. _ZN10cosmostrix5frame5Frame3set...).
# We use wildcards to match. bpftrace supports glob patterns in uprobe names.
# If symbols are stripped (release build), use addresses from `nm` or `objdump`.

# Get symbol addresses for stripped binaries
SYMBOLS=$(nm "$BIN" 2>/dev/null | grep -iE 'frame.*set|terminal.*draw|flush_ansi|sgr_for_cell' || true)

if [[ -z "$SYMBOLS" ]]; then
    echo "WARNING: no matching symbols found in binary (may be stripped)." >&2
    echo "         Falling back to syscall tracing only." >&2
    echo "" >&2

    # Syscall-only tracing (always works)
    bpftrace -e '
        tracepoint:syscalls:sys_enter_write /comm == "cosmostrix"/ {
            @writes = count();
            @bytes = hist(args->count);
        }

        tracepoint:syscalls:sys_enter_write /comm == "cosmostrix"/ {
            $start = nsecs;
        }
        tracepoint:syscalls:sys_exit_write /comm == "cosmostrix"/ {
            @write_latency_ns = hist(nsecs - $start);
        }

        interval:s:5 {
            printf("\n--- cosmostrix write() stats (5s window) ---\n");
            print(@writes);
            print(@bytes);
            print(@write_latency_ns);
            printf("\n");
        }
    '
    exit 0
fi

echo "Found symbols:" >&2
echo "$SYMBOLS" >&2
echo "" >&2

# Full profiling with uprobes + syscalls
# The exact symbol names depend on the build. We try common patterns.
bpftrace -e "
    // --- frame::set() ---
    uprobe:\"$BIN\":*frame*set* {
        @set_start[tid] = nsecs;
        @set_count = count();
    }
    uretprobe:\"$BIN\":*frame*set* /@set_start[tid]/ {
        @set_duration_ns = hist(nsecs - @set_start[tid]);
        delete(@set_start[tid]);
    }

    // --- terminal::draw() ---
    uprobe:\"$BIN\":*terminal*draw* {
        @draw_start[tid] = nsecs;
        @draw_count = count();
    }
    uretprobe:\"$BIN\":*terminal*draw* /@draw_start[tid]/ {
        @draw_duration_ns = hist(nsecs - @draw_start[tid]);
        delete(@draw_start[tid]);
    }

    // --- flush_ansi() ---
    uprobe:\"$BIN\":*flush_ansi* {
        @flush_start[tid] = nsecs;
        @flush_count = count();
    }
    uretprobe:\"$BIN\":*flush_ansi* /@flush_start[tid]/ {
        @flush_duration_ns = hist(nsecs - @flush_start[tid]);
        delete(@flush_start[tid]);
    }

    // --- write() syscalls ---
    tracepoint:syscalls:sys_enter_write /comm == \"cosmostrix\"/ {
        @write_bytes = hist(args->count);
        @write_count = count();
    }

    // --- Periodic summary ---
    interval:s:5 {
        time(\"%H:%M:%S \");
        printf(\"--- cosmostrix hot path stats ---\n\");
        printf(\"frame::set() calls: \");  print(@set_count);
        printf(\"frame::set() duration: \"); print(@set_duration_ns);
        printf(\"terminal::draw() calls: \"); print(@draw_count);
        printf(\"terminal::draw() duration: \"); print(@draw_duration_ns);
        printf(\"flush_ansi() calls: \"); print(@flush_count);
        printf(\"flush_ansi() duration: \"); print(@flush_duration_ns);
        printf(\"write() calls: \"); print(@write_count);
        printf(\"write() bytes: \"); print(@write_bytes);
        printf(\"\n\");
    }
" 2>&1

echo "" >&2
echo "Profiling stopped. Histograms above show:" >&2
echo "  - Call counts (how often each function runs)" >&2
echo "  - Duration histograms (where time is spent)" >&2
echo "  - write() byte sizes (ANSI output per syscall)" >&2
echo "" >&2
echo "Look for:" >&2
echo "  - frame::set() should dominate call count (called per dirty cell)" >&2
echo "  - terminal::draw() duration should be <1ms (sub-frame-budget)" >&2
echo "  - write() bytes should be small (diff-based = few bytes per frame)" >&2
