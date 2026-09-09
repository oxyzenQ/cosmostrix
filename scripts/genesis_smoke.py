#!/usr/bin/env python3
#
# COSMOSTRIX DNA GENESIS PTY SMOKE (NIGHT-research-7 part 3)
#
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Drives the dna_helix scene inside a PTY and reconstructs the
# screen at the genesis checkpoints, asserting the birth
# sequence's visible signature (pacing-robust: the interactive
# loop's sim cadence is adaptive, so the checks read SHAPE not
# wall-clock phase boundaries):
#   - soup:   no dominant column (the sky is scattered rain —
#             the molecule has not drawn)
#   - ladder: two straight full-height strand columns (the flat
#             face-on ladder, symmetric around the axis)
#   - steady: the molecule spread into the helix band (the
#             columns dissolve into the winding ladder; the drawn
#             population grows past the flat ladder's)
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
        ["./target/release/cosmostrix", "--scene", "dna_helix",
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


def band_cells(grid):
    """Non-space cells in the central helix band (cols 23-56,
    lines 1-38) — the molecule's home plus the soup passing
    through it."""
    n = 0
    for y in range(1, ROWS - 2):
        row = grid[y] if y < len(grid) else ""
        for x in range(23, 57):
            if x < len(row) and row[x] != " ":
                n += 1
    return n


def column_dominance(grid):
    """The most rows any single column holds in the helix band —
    the straight flat ladder's signature is two columns each
    holding nearly every row; scattered rain and the wound helix
    both spread their occupancy."""
    best = 0
    for x in range(23, 57):
        rows = 0
        for y in range(1, ROWS - 2):
            row = grid[y] if y < len(grid) else ""
            if x < len(row) and row[x] != " ":
                rows += 1
        best = max(best, rows)
    return best


def run():
    ok = True

    # Soup (~0.35 s wall): the primordial dwell — scattered rain,
    # no dominant column (a formed molecule's strands hold 30+
    # rows on their columns; scattered soup holds far less).
    soup_grid = screen_at(0.35)
    soup_dom = column_dominance(soup_grid)
    soup_cells = band_cells(soup_grid)
    print(f"soup   cells: {soup_cells:3d}  column dominance: {soup_dom}")
    if soup_dom >= 20:
        print("FAIL: a dominant column stood during the primordial soup")
        ok = False

    # Ladder (~0.8 s wall): the flat face-on ladder — two straight
    # strand columns, each holding most rows.
    ladder_grid = screen_at(0.8)
    ladder_dom = column_dominance(ladder_grid)
    ladder_cells = band_cells(ladder_grid)
    print(f"ladder cells: {ladder_cells:3d}  column dominance: {ladder_dom}")
    if ladder_dom < 25:
        print("FAIL: the flat ladder's strand columns never stood")
        ok = False
    if ladder_cells <= soup_cells:
        print("FAIL: the ladder drew less than the soup sky")
        ok = False

    # Steady (~8 s wall): the wound helix — the straight columns
    # dissolve (the twist spreads the strands across the radius
    # band) while the drawn population grows past the flat
    # ladder's.
    steady_grid = screen_at(8.0)
    steady_dom = column_dominance(steady_grid)
    steady_cells = band_cells(steady_grid)
    print(f"steady cells: {steady_cells:3d}  column dominance: {steady_dom}")
    if steady_cells <= ladder_cells:
        print("FAIL: the steady molecule is not fuller than the flat ladder")
        ok = False
    if steady_dom >= 35:
        print("FAIL: the columns never wound into the helix")
        ok = False

    print("GENESIS SMOKE: " + ("PASS" if ok else "FAIL"))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    run()
