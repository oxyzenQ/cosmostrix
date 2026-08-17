#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Minimal full-redraw Matrix rain in Python.
# Writes every cell every frame via ANSI escape sequences to stdout.
# Usage: BENCH_COLS=120 BENCH_LINES=40 BENCH_FRAMES=100 python3 matrix_python.py
import sys, os, random

COLS = int(os.environ.get("BENCH_COLS", "120"))
LINES = int(os.environ.get("BENCH_LINES", "40"))
FRAMES = int(os.environ.get("BENCH_FRAMES", "100"))

import time
start = time.time()
buf = [[" "]*COLS for _ in range(LINES)]
heads = [random.randint(0, LINES-1) for _ in range(COLS)]

for _ in range(FRAMES):
    for c in range(COLS):
        heads[c] = (heads[c]+1) % LINES
        buf[heads[c]][c] = random.choice("01")
        if heads[c] > 0: buf[heads[c]-1][c] = " "
    out = ["\x1b[H"]
    for r in range(LINES):
        for c in range(COLS):
            ch = buf[r][c]
            out.append("\x1b[92m"+ch if ch != " " and r == heads[c] else "\x1b[32m"+ch if ch != " " else " ")
        out.append("\n")
    sys.stdout.write("".join(out))
    sys.stdout.flush()

elapsed = time.time() - start
print(f"PYTHON: frames={FRAMES} elapsed={elapsed:.3f} fps={FRAMES/elapsed:.1f} bytes={sys.stdout.buffer.tell() if hasattr(sys.stdout,'buffer') else 'N/A'}", file=sys.stderr)
