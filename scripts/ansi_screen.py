#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""Mini ANSI screen reconstructor — proves the HUD's on-screen layout.

NIGHT-hunter-20 verification helper: applies an ANSI stream to a
virtual terminal grid (CUP moves + printable chars, ignoring SGR),
then reads the screen rows so HUD assertions can be made against the
RECONSTRUCTED SCREEN instead of the raw byte stream (whose order
depends on the differential renderer's multi-flush emission pattern,
not the visual layout).
"""

import re


def reconstruct_screen(raw: str, cols: int, rows: int) -> list:
    grid = [[" "] * cols for _ in range(rows)]
    y = x = 0
    i = 0
    n = len(raw)
    while i < n:
        ch = raw[i]
        if ch == "\x1b":
            # CSI sequence: ESC [ params letter
            m = re.match(r"\x1b\[([0-9;?]*)([a-zA-Z])", raw[i:])
            if m:
                params, final = m.group(1), m.group(2)
                if final == "H":
                    parts = params.split(";") if params else [""]
                    r_ = int(parts[0]) if parts[0] else 1
                    c_ = int(parts[1]) if len(parts) > 1 and parts[1] else 1
                    y, x = r_ - 1, c_ - 1
                i += m.end()
                continue
            # OSC sequence: ESC ] ... BEL
            m = re.match(r"\x1b\][^\x07]*\x07", raw[i:])
            if m:
                i += m.end()
                continue
            # Other two-char escapes (ESC + single char)
            i += 2
            continue
        if ch == "\n":
            y += 1
            i += 1
            continue
        if ch == "\r":
            x = 0
            i += 1
            continue
        if 0 <= y < rows and 0 <= x < cols and ch.isprintable():
            grid[y][x] = ch
        x += 1
        i += 1
    return ["".join(r).rstrip() for r in grid]
