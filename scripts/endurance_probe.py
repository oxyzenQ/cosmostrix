#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""COSMOSTRIX NIGHT-lts-4 endurance probe.

Spawns cosmostrix in a PTY (drain-rate reader like a real terminal),
then samples /proc/<pid> every SAMPLE_SECS for RUN_SECS:
  - CPU% (utime+stime delta / wall delta) per interval
  - VmRSS trajectory (leak detection: slope after warmup)
  - minor/major page faults (should flatline after warmup)
  - voluntary/involuntary context switches

Exit code 0 = PASS (bounded CPU, RSS slope under leak threshold).
Exit code 1 = findings (printed).

Env knobs: BIN, RUN_SECS (default 120), SAMPLE_SECS (default 5),
SCENE (default cinematic), EXTRA_ARGS, DRAIN_BPS (default 12_000_000),
SIZE (default 200x56), RSS_SLOPE_LIMIT_MB_PER_MIN (default 1.0),
CPU_LIMIT_PCT (default 40).
"""

import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time
from itertools import pairwise

BIN = os.environ.get("BIN", "target/release/cosmostrix")
RUN_SECS = float(os.environ.get("RUN_SECS", "120"))
SAMPLE_SECS = float(os.environ.get("SAMPLE_SECS", "5"))
SCENE = os.environ.get("SCENE", "cinematic")
EXTRA_ARGS = os.environ.get("EXTRA_ARGS", "").split()
DRAIN_BPS = float(os.environ.get("DRAIN_BPS", "12_000_000"))
COLS, ROWS = (int(x) for x in os.environ.get("SIZE", "200x56").split("x"))
RSS_SLOPE_LIMIT = float(os.environ.get("RSS_SLOPE_LIMIT_MB_PER_MIN", "1.0"))
CPU_LIMIT = float(os.environ.get("CPU_LIMIT_PCT", "40"))
WARMUP_SECS = 15.0

HZ = os.sysconf("SC_CLK_TCK")


def read_proc(pid):
    with open(f"/proc/{pid}/stat") as f:
        parts = f.read().rsplit(") ", 1)[1].split()
    # parts[i] = stat field (i+3). utime=14 -> parts[11], stime=15 ->
    # parts[12], minflt=10 -> parts[7], majflt=12 -> parts[8].
    utime, stime = int(parts[11]), int(parts[12])
    minflt, majflt = int(parts[7]), int(parts[8])
    vcs, ivcs = 0, 0
    with open(f"/proc/{pid}/status") as f:
        status = f.read()
    rss_kb = 0
    for line in status.splitlines():
        if line.startswith("VmRSS:"):
            rss_kb = int(line.split()[1])
        elif line.startswith("voluntary_ctxt_switches:"):
            vcs = int(line.split()[1])
        elif line.startswith("nonvoluntary_ctxt_switches:"):
            ivcs = int(line.split()[1])
    return {
        "cpu_ticks": utime + stime,
        "minflt": minflt,
        "majflt": majflt,
        "vcs": vcs,
        "ivcs": ivcs,
        "rss_kb": rss_kb,
    }


def main():
    master, slave = pty.openpty()
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["TERM_PROGRAM"] = "alacritty"
    argv = [BIN, "--scene", SCENE] + EXTRA_ARGS
    pid = os.fork()
    if pid == 0:
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        os.dup2(slave, 0)
        os.dup2(slave, 1)
        os.dup2(slave, 2)
        os.close(master)
        os.close(slave)
        os.execve(argv[0], argv, env)
        os._exit(127)
    os.close(slave)

    start = time.time()
    samples = []
    byte_budget = 0
    last_drain = time.time()
    try:
        next_sample = start + SAMPLE_SECS
        while time.time() - start < RUN_SECS:
            # drain the PTY at DRAIN_BPS
            now = time.time()
            byte_budget += (now - last_drain) * DRAIN_BPS
            last_drain = now
            timeout = max(0.0, next_sample - now)
            r, _, _ = select.select([master], [], [], min(0.2, timeout))
            if r:
                try:
                    data = os.read(master, 262144)
                except OSError:
                    break
                byte_budget = max(0, byte_budget - len(data))
                if byte_budget <= 0:
                    time.sleep(0.05)
            now = time.time()
            if now >= next_sample:
                try:
                    s = read_proc(pid)
                except (FileNotFoundError, ProcessLookupError):
                    break
                s["t"] = now - start
                samples.append(s)
                next_sample = now + SAMPLE_SECS
    finally:
        os.kill(pid, signal.SIGTERM)
        try:
            # Grace window for the child's own signal handlers; then force.
            deadline = time.time() + 3.0
            while time.time() < deadline:
                r = os.waitpid(pid, os.WNOHANG)
                if r[0] == pid:
                    break
                time.sleep(0.05)
            else:
                os.kill(pid, signal.SIGKILL)
                try:
                    os.waitpid(pid, 0)
                except ChildProcessError:
                    pass
        except ChildProcessError:
            pass
        os.close(master)

    if len(samples) < 4:
        print("FAIL: not enough samples")
        return 1

    # CPU per interval
    findings = []
    cpu_pcts = []
    for a, b in pairwise(samples):
        wall = b["t"] - a["t"]
        cpu_pct = (b["cpu_ticks"] - a["cpu_ticks"]) / HZ / wall * 100.0
        cpu_pcts.append(cpu_pct)
    steady_cpu = [c for c, s in zip(cpu_pcts, samples[1:]) if s["t"] >= WARMUP_SECS]
    max_cpu = max(cpu_pcts)
    avg_steady = sum(steady_cpu) / len(steady_cpu) if steady_cpu else 0.0

    # RSS slope after warmup (linear regression on steady samples)
    steady = [s for s in samples if s["t"] >= WARMUP_SECS]
    n = len(steady)
    if n >= 3:
        xs = [s["t"] for s in steady]
        ys = [s["rss_kb"] for s in steady]
        mx = sum(xs) / n
        my = sum(ys) / n
        slope_kb_s = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum(
            (x - mx) ** 2 for x in xs
        )
        rss_slope_mb_min = slope_kb_s * 60.0 / 1024.0
    else:
        rss_slope_mb_min = float("inf")

    rss_start = samples[0]["rss_kb"] / 1024.0
    rss_end = samples[-1]["rss_kb"] / 1024.0
    rss_max = max(s["rss_kb"] for s in samples) / 1024.0

    # page faults after warmup (per second)
    flt_rate = 0.0
    if len(steady) >= 2:
        a, b = steady[0], steady[-1]
        flt_rate = (b["minflt"] - a["minflt"]) / (b["t"] - a["t"])

    vcs_end = samples[-1]["vcs"]
    ivcs_end = samples[-1]["ivcs"]

    print(f"samples: {len(samples)}  interval: {SAMPLE_SECS}s  run: {RUN_SECS:.0f}s")
    print(f"scene: {SCENE}  size: {COLS}x{ROWS}  drain: {DRAIN_BPS / 1e6:.0f} MB/s")
    print(f"cpu max: {max_cpu:.1f}%  cpu steady avg: {avg_steady:.1f}%")
    print(f"rss: start {rss_start:.1f} MB  end {rss_end:.1f} MB  max {rss_max:.1f} MB")
    print(f"rss slope after warmup: {rss_slope_mb_min:+.2f} MB/min")
    print(f"minor faults after warmup: {flt_rate:.1f}/s")
    print(f"ctx switches: vol {vcs_end}  invol {ivcs_end}")

    if avg_steady > CPU_LIMIT:
        findings.append(f"steady CPU {avg_steady:.1f}% exceeds {CPU_LIMIT}% budget")
    if rss_slope_mb_min > RSS_SLOPE_LIMIT:
        findings.append(
            f"RSS slope {rss_slope_mb_min:+.2f} MB/min exceeds "
            f"{RSS_SLOPE_LIMIT} MB/min — possible leak"
        )
    if flt_rate > 500:
        findings.append(f"steady minor-fault rate {flt_rate:.0f}/s — page churn")

    if findings:
        print("FINDINGS:")
        for f in findings:
            print(f"  - {f}")
        return 1
    print("PASS: bounded CPU, flat RSS, no fault churn")
    return 0


if __name__ == "__main__":
    sys.exit(main())
