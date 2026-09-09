#!/usr/bin/env python3
#
# COSMOSTRIX NEURAL PTY SMOKE (NIGHT-research-9)
#
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Drives the neural scene inside a PTY and reconstructs the
# screen at the genesis checkpoints, asserting the birth
# sequence's visible signature (pacing-robust: the interactive
# loop's sim cadence is adaptive, so the checks read SHAPE not
# wall-clock phase boundaries):
#   - signal: the machine's band is empty (the network has not
#             built — the data falls in the top band, above the
#             input layer's line, and nothing below it exists)
#   - steady: the machine thinks — the full band holds the
#             wiring + neurons (the loom), the input band holds
#             its neurons, and the output band holds its answer
#
# Local verification helper (not part of the shipped gatekeepers).

import os
import pty
import select
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ansi_screen import reconstruct_screen

COLS, ROWS = 80, 40

# The machine's geometry at 80x40 (NeuralGeom::for_viewport, four
# layers on 40 rows): the input band anchors at 0.26*40 = 10.4,
# the output band at 0.86*40 = 34.4; the x margin is 0.08*80 =
# 6.4. The signal phase's streamers absorb AT the input line —
# they never enter the machine's band below it.
MACHINE_BOX = (5, 75, 13, 37)  # cols 5-75, lines 13-37
INPUT_BOX = (5, 75, 8, 12)  # the input band +- the jitter
OUTPUT_BOX = (5, 75, 32, 36)  # the output band +- the jitter


def capture(seconds: float) -> str:
    """Run cosmostrix for `seconds` inside a PTY, return raw ANSI."""
    master, slave = pty.openpty()
    import fcntl
    import struct
    import termios

    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    proc = subprocess.Popen(
        [
            "./target/release/cosmostrix",
            "--scene",
            "neural",
            "--intro",
            "none",
            "--msg-mode",
            "false",
            "--duration",
            f"{seconds + 0.5:.1f}",
        ],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        close_fds=True,
        env=env,
    )
    os.close(slave)
    buf = b""
    deadline = time.time() + seconds + 2.0
    while time.time() < deadline and proc.poll() is None:
        r, _, _ = select.select([master], [], [], 0.1)
        if r:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                break
            if not chunk:
                break
            buf += chunk
    os.close(master)
    proc.terminate()
    proc.wait()
    return buf.decode("utf-8", errors="replace")


def screen_at(seconds: float):
    raw = capture(seconds)
    return reconstruct_screen(raw, COLS, ROWS)


def box_cells(grid, box):
    """Non-space cells inside a (col_a, col_b, line_a, line_b) box."""
    ca, cb, la, lb = box
    n = 0
    for y in range(la, lb + 1):
        row = grid[y] if y < len(grid) else ""
        for x in range(ca, cb + 1):
            if x < len(row) and row[x] != " ":
                n += 1
    return n


def run():
    ok = True

    # Signal (~0.5 s wall): the data falls, the machine is
    # unbuilt — the band below the input line is empty (no
    # neurons, no wiring; the streamers absorb AT the input
    # line, never past it).
    signal_grid = screen_at(0.5)
    signal_band = box_cells(signal_grid, MACHINE_BOX)
    print(f"signal machine-band: {signal_band:3d}")
    if signal_band > 0:
        print("FAIL: the machine drew before its genesis built it")
        ok = False

    # Steady (~20 s wall — the genesis is ~8.2 sim-s and the
    # interactive cadence is adaptive, so the window holds the
    # whole training run plus settling): the thinking machine —
    # the full band holds the loom (the wiring + the neurons),
    # the input band holds its neurons, the output band holds
    # its answer.
    steady_grid = screen_at(20.0)
    steady_band = box_cells(steady_grid, MACHINE_BOX)
    steady_input = box_cells(steady_grid, INPUT_BOX)
    steady_output = box_cells(steady_grid, OUTPUT_BOX)
    print(
        f"steady machine-band: {steady_band:3d}  input: {steady_input:3d}  "
        f"output: {steady_output:3d}"
    )
    if steady_band < 30:
        print("FAIL: the machine never wove its loom")
        ok = False
    if steady_input < 4:
        print("FAIL: the input band never materialized its neurons")
        ok = False
    if steady_output < 3:
        print("FAIL: the output band never held its answer")
        ok = False

    if ok:
        print("NEURAL SMOKE: PASS")
    else:
        print("NEURAL SMOKE: FAIL")
        sys.exit(1)


if __name__ == "__main__":
    run()
