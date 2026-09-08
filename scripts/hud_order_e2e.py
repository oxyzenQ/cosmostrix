#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""E2E HUD row-order verification for the v51 reorder (owner mandate).

Spawns cosmostrix in a PTY, skips the intro, toggles the HUD with 'i',
waits for the 1 Hz metric tick, and asserts the exact row order:
fps/tgt/max/p99/cpu/rss/ehs/prs/scn/chr/clr/sped/dsty/prdr/crdr/ambt/
glth/ctun/mnst/cid/up/screensize.

NIGHT-hunter-20 (2026-09-08) methodology fix: the original version
asserted the order of label occurrences in the RAW ANSI stream. That
proxy is invalid — the differential renderer may paint one HUD toggle
across MULTIPLE frame flushes (each bounded by a synchronized-output
`ESC[?2026l` marker), and the flush emission order follows the dirty
cell population, not the visual row layout. The observed failure mode:
the bottom HUD rows (cid/up/screensize + border) flushed in frame A
and the top rows (fps..) in frame B, so `cid`'s stream position sorted
FIRST while the on-screen layout was perfectly correct. The fix:
reconstruct the virtual terminal screen from the ANSI stream (shared
`ansi_screen.py` mini-emulator) and assert the label order by reading
the SCREEN ROWS — what the user actually sees. This ran red on the
baseline tree (b8efb15) with the old methodology and green with the
screen reconstruction on the same binary, confirming the script —
not the renderer — was the defect.
"""

import os
import pty
import re
import select
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ansi_screen import reconstruct_screen

BIN = "./target/release/cosmostrix"
COLS, ROWS = 100, 40


def main():
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(BIN, [BIN, "--fps", "60"])

    # Set the PTY window size BEFORE the app reads it (a fresh PTY is
    # 1x1; the resize escape arrives too late — cosmostrix measures at
    # startup and rejects the intro at < 10x5).
    import fcntl
    import struct
    import termios

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    stop = threading.Event()
    buf = bytearray()
    lock = threading.Lock()

    def reader():
        while not stop.is_set():
            try:
                r, _, _ = select.select([fd], [], [], 0.2)
                if not r:
                    continue
                chunk = os.read(fd, 65536)
                if not chunk:
                    break
                with lock:
                    buf.extend(chunk)
            except OSError:
                break

    t = threading.Thread(target=reader, daemon=True)
    t.start()

    # Skip the intro (6 s brand delay) then let it run a bit.
    time.sleep(7.0)
    os.write(fd, b"i")
    time.sleep(3.0)  # HUD on + at least one 1 Hz metric tick
    os.write(fd, b"q")
    time.sleep(1.0)
    stop.set()
    t.join(timeout=1.0)

    try:
        os.close(fd)
    except OSError:
        pass
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass

    with lock:
        text = bytes(buf).decode("utf-8", errors="replace")

    # Reconstruct the virtual terminal screen from the ANSI stream and
    # assert the row order by READING THE SCREEN — the raw stream's
    # emission order follows the dirty-cell flush population, not the
    # visual layout (see the module docstring for the failure history).
    screen = reconstruct_screen(text, COLS, ROWS)

    order_pats = [
        (r"^\s*fps:", "fps"),
        (r"^\s*tgt:", "tgt"),
        (r"^\s*max:", "max"),
        (r"^\s*p99:", "p99"),
        (r"^\s*cpu:", "cpu"),
        (r"^\s*rss:", "rss"),
        (r"^\s*ehs:", "ehs"),
        (r"^\s*prs:", "prs"),
        (r"^\s*scn:", "scn"),
        (r"^\s*chr:", "chr"),
        (r"^\s*clr:", "clr"),
        (r"^\s*sped:", "sped"),
        (r"^\s*dsty:", "dsty"),
        (r"^\s*prdr:", "prdr"),
        (r"^\s*crdr:", "crdr"),
        (r"^\s*ambt:", "ambt"),
        (r"^\s*glth:", "glth"),
        (r"^\s*ctun:", "ctun"),
        (r"^\s*mnst:", "mnst"),
        (r"^\s*rain:", "rain"),
        (r"^\s*dcel:", "dcel"),
        (r"^\s*tcel:", "tcel"),
        (r"^\s*cid:", "cid"),
        (r"^\s*up:", "up"),
        (r"^\s*\d+x\d+ (?:auto|fix)", "size"),
    ]
    # Map each screen row to the label it starts with (rows 0..25 — the
    # HUD block plus the border row). The screen reconstruction reflects
    # the FINAL painted state, so stale intermediate frames cannot skew
    # the result the way raw stream positions did.
    row_labels = {}
    for row_idx, row_text in enumerate(screen[:26]):
        for pat, name in order_pats:
            if re.match(pat, row_text):
                row_labels[name] = row_idx
                break
    names = [n for n, _ in sorted(row_labels.items(), key=lambda kv: kv[1])]
    present = set(row_labels)
    print("labels found:", len(present), "of 25 ->", sorted(present))
    print("screen order (by screen row):", names)
    expected = [
        "fps",
        "tgt",
        "max",
        "p99",
        "cpu",
        "rss",
        "ehs",
        "prs",
        "scn",
        "chr",
        "clr",
        "sped",
        "dsty",
        "prdr",
        "crdr",
        "ambt",
        "glth",
        "ctun",
        "mnst",
        "rain",
        "dcel",
        "tcel",
        "cid",
        "up",
        "size",
    ]
    ok = names == expected and len(present) == 25
    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
