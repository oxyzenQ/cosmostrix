#!/usr/bin/env python3
#
# COSMOSTRIX QUASAR PTY SMOKE (NIGHT-research-8)
#
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Drives the quasar scene inside a PTY and reconstructs the
# screen at the ignition checkpoints, asserting the birth
# sequence's visible signature (pacing-robust: the interactive
# loop's sim cadence is adaptive, so the checks read SHAPE not
# wall-clock phase boundaries):
#   - dark:  the core's home is empty (the engine has not lit;
#            the cold rain falls at the rim, far from the center)
#   - steady: the engine burns — the core's home holds the
#             engine's cells (core + glow + inner disk) and the
#             beam band lights most of its rows (the polar jets
#             firing through the disk's plane)
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

# The engine's geometry at 80x40 (QuasGeom::for_viewport): the
# core sits at (39.5, 17.2); the beams fire the central column
# band; the cold rain spawns at the rim (|x - 39.5| >= 34).
CORE_BOX = (36, 43, 15, 20)   # cols 36-43, lines 15-20
BEAM_COLS = (36, 43)          # the precessing beams +- the helix


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
        ["./target/release/cosmostrix", "--scene", "quasar",
         "--intro", "none", "--msg-mode", "false",
         "--duration", f"{seconds + 0.5:.1f}"],
        stdin=slave, stdout=slave, stderr=slave,
        close_fds=True, env=env,
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
    proc.terminate()
    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()
    os.close(master)
    return buf.decode("utf-8", "replace")


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


def beam_rows(grid):
    """Rows with at least one cell in the beam column band — the
    jets' vertical signature (every row the beams cross)."""
    ca, cb = BEAM_COLS
    rows = 0
    for y in range(1, ROWS - 2):
        row = grid[y] if y < len(grid) else ""
        for x in range(ca, cb + 1):
            if x < len(row) and row[x] != " ":
                rows += 1
                break
    return rows


def run():
    ok = True

    # Dark (~0.5 s wall): the cold cloud falls, the engine's home
    # is empty (no core, no disk — the infall and the halo orbit
    # outside the center box, the captures have not landed).
    dark_grid = screen_at(0.5)
    dark_core = box_cells(dark_grid, CORE_BOX)
    dark_band = beam_rows(dark_grid)
    print(f"dark   core-box: {dark_core:3d}  beam rows: {dark_band:3d}")
    if dark_core > 0:
        print("FAIL: the engine drew its core during the dark cloud")
        ok = False

    # Steady (~20 s wall — the ignition is ~8 sim-s and the
    # interactive cadence is adaptive, so the window holds the
    # whole birth plus settling): the burning engine — the core's
    # home holds the engine's cells and the beams light most of
    # their rows.
    steady_grid = screen_at(20.0)
    steady_core = box_cells(steady_grid, CORE_BOX)
    steady_band = beam_rows(steady_grid)
    print(f"steady core-box: {steady_core:3d}  beam rows: {steady_band:3d}")
    if steady_core < 4:
        print("FAIL: the engine never drew its core + glow")
        ok = False
    if steady_band < 22:
        print("FAIL: the beams never fired through the band")
        ok = False

    if ok:
        print("QUASAR SMOKE: PASS")
    else:
        print("QUASAR SMOKE: FAIL")
        sys.exit(1)


if __name__ == "__main__":
    run()
