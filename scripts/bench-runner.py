#!/usr/bin/env python3
#
# COSMOSTRIX BENCH RUNNER — fair PTY measurement of a single tool
#
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Spawns a tool inside a pseudo-terminal (PTY) so terminal-aware tools
# (cosmostrix, neo-matrix, cxxmatrix) actually run their event loops
# instead of exiting early on non-tty stdout.
#
# Measures the tool DIRECTLY (not a wrapper shell):
#   - CPU time: resource.getrusage(RUSAGE_CHILDREN) delta (user + sys)
#   - Peak RSS: poll /proc/<pid>/status VmHWM at 10 Hz
#
# After DURATION seconds, sends SIGTERM. If the process doesn't exit
# within 5 seconds, sends SIGKILL.
#
# Output: single TSV line to stdout:
#   label<TAB>cpu_total_seconds<TAB>cpu_percent<TAB>peak_rss_kib
#
# Usage:
#   python3 bench-runner.py LABEL DURATION CMD [ARGS...]
#
# Example:
#   python3 bench-runner.py cosmostrix 30 ./target/release/cosmostrix
#   python3 bench-runner.py cmatrix 30 cmatrix -s
#

import os
import pty
import resource
import signal
import subprocess
import sys
import time


def main():
    if len(sys.argv) < 4:
        print("Usage: bench-runner.py LABEL DURATION CMD [ARGS...]", file=sys.stderr)
        sys.exit(1)

    label = sys.argv[1]
    duration = float(sys.argv[2])
    cmd = sys.argv[3]
    args = sys.argv[4:]

    # Snapshot cumulative children rusage BEFORE spawning.
    # resource.getrusage(RUSAGE_CHILDREN) returns totals for all children
    # that have been waited for. By snapshotting before+after, the delta
    # gives us this specific child's CPU time (user + system).
    rusage_before = resource.getrusage(resource.RUSAGE_CHILDREN)

    # Create a PTY pair. The slave end will be the tool's stdout/stderr/stdin.
    # This makes isatty() return True inside the tool, so terminal-aware
    # tools (cosmostrix, neo-matrix, cxxmatrix) run their event loops
    # instead of exiting early.
    master, slave = pty.openpty()

    # Spawn the tool with the PTY slave as its stdio.
    # TERM=xterm-256color so tools that check $TERM detect color support.
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"

    try:
        proc = subprocess.Popen(
            [cmd] + args,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            close_fds=True,
            env=env,
        )
    except FileNotFoundError:
        print(f"{label}\t—\t—\t—", flush=True)
        os.close(slave)
        os.close(master)
        return
    except OSError as e:
        print(f"{label}\t—\t—\t—", flush=True)
        print(f"spawn error: {e}", file=sys.stderr)
        os.close(slave)
        os.close(master)
        return

    # Close slave in parent — the child has its own copy now.
    os.close(slave)

    # Poll for peak RSS at 10 Hz. Also drain PTY output to prevent the
    # kernel PTY buffer (~64 KiB) from filling and blocking the tool's
    # write() calls.
    peak_rss_kib = 0
    start_time = time.monotonic()
    deadline = start_time + duration

    while True:
        # Check if process exited on its own
        result = proc.poll()
        if result is not None:
            break

        # Check deadline
        if time.monotonic() >= deadline:
            break

        # Sample RSS from /proc/<pid>/status
        try:
            with open(f"/proc/{proc.pid}/status", "r") as f:
                for line in f:
                    if line.startswith("VmHWM:"):
                        # Format: "VmHWM:\t    3908 kB\n"
                        parts = line.split()
                        if len(parts) >= 2:
                            rss = int(parts[1])
                            if rss > peak_rss_kib:
                                peak_rss_kib = rss
                        break
        except (IOError, ValueError, ProcessLookupError):
            # Process may have just exited — ignore
            pass

        # Drain PTY output (non-blocking-ish: read up to 8 KiB)
        try:
            os.read(master, 8192)
        except OSError:
            # EAGAIN or EOF — either way, continue
            pass

        time.sleep(0.1)

    # Kill the process: SIGTERM first, then SIGKILL after 5s if needed.
    if proc.poll() is None:
        try:
            proc.send_signal(signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            try:
                proc.kill()
            except ProcessLookupError:
                pass
            proc.wait()

    # Close master PTY
    try:
        os.close(master)
    except OSError:
        pass

    # Snapshot cumulative children rusage AFTER. The delta from before
    # gives this child's CPU time. This works because we waited for the
    # child above (proc.wait()), so its rusage is now accounted in
    # RUSAGE_CHILDREN.
    rusage_after = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu_user = rusage_after.ru_utime - rusage_before.ru_utime
    cpu_sys = rusage_after.ru_stime - rusage_before.ru_stime
    cpu_total = cpu_user + cpu_sys

    # Guard against clock skew (shouldn't happen, but be safe)
    if cpu_total < 0:
        cpu_total = 0.0

    cpu_pct = (cpu_total / duration) * 100.0 if duration > 0 else 0.0

    # Output TSV: label, cpu_total, cpu_pct, peak_rss_kib
    print(f"{label}\t{cpu_total:.2f}\t{cpu_pct:.1f}\t{peak_rss_kib}", flush=True)


if __name__ == "__main__":
    main()
