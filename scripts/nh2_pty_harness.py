#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-hunter-2 drain-loop reproduction harness (owner hunt 2026-09-04).

Reproduces the "glitch rain shift" mechanism on demand: spawns cosmostrix
in a PTY whose reader drains at a configurable rate (a real terminal's
sustainable ANSI throughput), then reports the output-cadence trajectory
the drain backoff produced.

Why it exists: HUNT-23 wired write-latency overshoot into perf_pressure
and the drain backoff. On a marginally-draining terminal the loop's
pressure strobes 0.0 -> 1.0 with a ~1-2 s period, and every VISUAL
consumer of the raw signal (phosphor decay skip hysteresis, spawn-scale
bands, glitch gate, sim-delta cap) flapped with it — the owner-visible
periodic glitch. NIGHT-hunter-2 fed those consumers a 2.5 s EMA instead;
this harness is the before/after evidence machine.

Usage (from the repo root, release binary built):

  python3 scripts/nh2_pty_harness.py                    # 60 s @ 12 MB/s
  DRAIN_BPS=5000000 RUN_SECS=75 python3 scripts/nh2_pty_harness.py

Knobs (env):
  DRAIN_BPS   reader drain rate, bytes/sec. 12 MB/s = marginal
              saturation (the owner-regime reproduction: 144 FPS x
              ~95 KB frames demands ~14 MB/s); 5 MB/s = chronic
              saturation stress; 0 = unlimited (fast-reader control).
  RUN_SECS    wall seconds to run (default 60).
  SIZE        "cols x rows" (default 200x56).
  BIN         binary path (default target/release/cosmostrix).
  TERM_PROGRAM  terminal identity for the high-perf classification
              (default alacritty -> 144 FPS dynamic default).

Output: per-frame TSV (t_end_sec, frame_bytes) at
  /tmp/cosmostrix-nh2/frames.tsv
plus a stdout summary: per-second FPS buckets, frame-size distribution,
inter-frame gap percentiles, big-frame (dirty_all repaint) counts.
The visual-strobe signature of the pre-fix build: frames ballooning
past 150 KB (2-6x normal) in bursts + gap spikes > 50 ms recurring
every few seconds. Post-fix marginal-drain runs show none.
"""

import fcntl
import os
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time

TERM_COLS, TERM_ROWS = (int(x) for x in os.environ.get("SIZE", "200x56").split("x"))
DRAIN_BPS = float(os.environ.get("DRAIN_BPS", "12_000_000"))
RUN_SECS = float(os.environ.get("RUN_SECS", "60"))
EVT_LOG = os.environ.get(
    "EVT_LOG", "/tmp/cosmostrix-nh2/frames.tsv"
)
BIN = os.environ.get("BIN", "target/release/cosmostrix")
TERM_PROGRAM = os.environ.get("TERM_PROGRAM", "alacritty")

BSU_ON = b"\x1b[?2026h"
BSU_OFF = b"\x1b[?2026l"
UNLIMITED = DRAIN_BPS <= 0
CREDIT_CAP = 262144  # 256 KB burst tolerance, no long-term banking


def main() -> int:
    master_fd, slave_fd = pty.openpty()
    os.set_blocking(master_fd, False)
    fcntl.ioctl(
        slave_fd, termios.TIOCSWINSZ, struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0)
    )

    env = dict(os.environ)
    for k in ("NO_COLOR", "CLICOLOR", "CLICOLOR_FORCE"):
        env.pop(k, None)
    env.update(
        {
            "TERM": "xterm-256color",
            "COLORTERM": "truecolor",
            "TERM_PROGRAM": TERM_PROGRAM,
            "COLUMNS": str(TERM_COLS),
            "LINES": str(TERM_ROWS),
        }
    )

    proc = subprocess.Popen(
        [BIN], stdin=slave_fd, stdout=slave_fd, stderr=slave_fd, env=env, close_fds=True
    )
    os.close(slave_fd)

    start = time.monotonic()
    deadline = start + RUN_SECS

    frames = []  # (t_end, bytes inside one BSU pair)
    pending = bytearray()
    in_frame = False
    frame_acc = bytearray()
    credit = CREDIT_CAP
    last_credit_t = start
    total = 0

    while True:
        now = time.monotonic()
        if now >= deadline or proc.poll() is not None:
            break
        if not UNLIMITED:
            credit += (now - last_credit_t) * DRAIN_BPS
            if credit > CREDIT_CAP:
                credit = float(CREDIT_CAP)
            last_credit_t = now
            if credit <= 0:
                time.sleep(0.0005)
                continue
        r, _, _ = select.select([master_fd], [], [], 0.002)
        if not r:
            continue
        want = CREDIT_CAP if UNLIMITED else max(1, min(int(credit), CREDIT_CAP))
        try:
            chunk = os.read(master_fd, want)
        except (BlockingIOError, OSError):
            continue
        if not chunk:
            break
        credit -= len(chunk) if not UNLIMITED else 0
        total += len(chunk)
        pending.extend(chunk)

        while True:
            if in_frame:
                off = pending.find(BSU_OFF)
                if off == -1:
                    frame_acc.extend(pending)
                    pending.clear()
                    break
                frame_acc.extend(pending[:off])
                pending = pending[off + len(BSU_OFF) :]
                frames.append((time.monotonic() - start, len(frame_acc)))
                frame_acc = bytearray()
                in_frame = False
            else:
                on = pending.find(BSU_ON)
                if on == -1:
                    if len(pending) > 65536:
                        pending.clear()
                    break
                pending = pending[on + len(BSU_ON) :]
                in_frame = True

    try:
        proc.send_signal(signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()

    os.makedirs(os.path.dirname(EVT_LOG), exist_ok=True)
    with open(EVT_LOG, "w") as f:
        f.write("# t_end_sec\tframe_bytes\n")
        for t, b in frames:
            f.write(f"{t:.6f}\t{b}\n")

    dur = time.monotonic() - start
    print(f"frames={len(frames)}  span={dur:.0f}s  avg {len(frames)/max(dur,1e-9):.1f} fps  "
          f"{total/max(dur,1e-9)/1e6:.2f} MB/s  tsv -> {EVT_LOG}")

    # Summary: the signatures NIGHT-hunter-2 locked.
    if len(frames) > 60:
        gaps = [
            (frames[i][0] - frames[i - 1][0]) * 1000
            for i in range(1, len(frames))
            if frames[i][0] > 5.0  # skip the intro cadence
        ]
        sizes = sorted(b for t, b in frames if t > 5.0)
        big = sum(1 for b in sizes if b > 150 * 1024)
        gs = sorted(gaps)
        print(f"frame bytes: p50={sizes[len(sizes)//2]/1024:.0f}KB "
              f"max={sizes[-1]/1024:.0f}KB  frames>150KB={big}")
        print(f"gaps ms: p50={gs[len(gs)//2]:.1f} p99={gs[int(len(gs)*0.99)]:.1f} "
              f"max={max(gs):.0f}  gaps>50ms={sum(1 for g in gaps if g > 50)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
