#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""
Naive full-screen redraw Matrix rain renderer.
Simulates cmatrix's approach: write every cell every frame via ANSI escapes.
Used as a baseline competitor for cell-write comparison.
"""
import sys
import os
import random
import time

COLS = int(os.environ.get("BENCH_COLS", "120"))
LINES = int(os.environ.get("BENCH_LINES", "40"))
DURATION = float(os.environ.get("BENCH_DURATION", "5"))

CHARS = "01"
buffer = [[" " for _ in range(COLS)] for _ in range(LINES)]
heads = [random.randint(0, LINES - 1) for _ in range(COLS)]

# ANSI: \x1b[H = cursor home, \x1b[32m = green, \x1b[0m = reset
HOME = "\x1b[H"
GREEN = "\x1b[32m"
GREEN_BRIGHT = "\x1b[92m"
RESET = "\x1b[0m"

start = time.time()
frame_count = 0

while time.time() - start < DURATION:
    # Advance heads
    for col in range(COLS):
        heads[col] = (heads[col] + 1) % LINES
        buf_row = heads[col]
        buffer[buf_row][col] = random.choice(CHARS)
        # Clear trail behind
        if buf_row > 0:
            buffer[buf_row - 1][col] = " "

    # Full-screen redraw: write every cell every frame
    out = [HOME]
    for row in range(LINES):
        for col in range(COLS):
            ch = buffer[row][col]
            if ch != " ":
                out.append(GREEN_BRIGHT if row == heads[col] else GREEN)
                out.append(ch)
            else:
                out.append(" ")
        out.append("\n")
    sys.stdout.write("".join(out))
    sys.stdout.flush()
    frame_count += 1

elapsed = time.time() - start
print(f"\nNAIVE: frames={frame_count}, elapsed={elapsed:.3f}s, fps={frame_count/elapsed:.1f}, "
      f"bytes_written={sys.stdout.buffer.tell() if hasattr(sys.stdout, 'buffer') else 'N/A'}",
      file=sys.stderr)
