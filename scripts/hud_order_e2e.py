#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""E2E HUD row-order verification for the v51 reorder (owner mandate).

Spawns cosmostrix in a PTY, skips the intro, toggles the HUD with 'i',
waits for the 1 Hz metric tick, and asserts the exact row order:
fps/tgt/max/p99/cpu/rss/ehs/prs/scn/chr/clr/sped/dsty/prdr/crdr/ambt/
glth/ctun/mnst/cid/up/screensize.
"""

import os
import pty
import re
import select
import sys
import threading
import time

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

    # Strip ANSI escapes for row scanning
    clean = re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]", "", text)
    clean = re.sub(r"\x1b\][^\x07]*\x07", "", clean)

    order_pats = [
        (r"\bfps:", "fps"),
        (r"\btgt:", "tgt"),
        (r"\bmax:", "max"),
        (r"\bp99:", "p99"),
        (r"\bcpu:", "cpu"),
        (r"\brss:", "rss"),
        (r"\behs:", "ehs"),
        (r"\bprs:", "prs"),
        (r"\bscn:", "scn"),
        (r"\bchr:", "chr"),
        (r"\bclr:", "clr"),
        (r"\bsped:", "sped"),
        (r"\bdsty:", "dsty"),
        (r"\bprdr:", "prdr"),
        (r"\bcrdr:", "crdr"),
        (r"\bambt:", "ambt"),
        (r"\bglth:", "glth"),
        (r"\bctun:", "ctun"),
        (r"\bmnst:", "mnst"),
        (r"\bdcel:", "dcel"),
        (r"\btcel:", "tcel"),
        (r"\bcid:", "cid"),
        (r"\bup:", "up"),
        (r"\d+x\d+ (?:auto|fix)", "size"),
    ]
    # Use the LAST occurrence of each label (the HUD repaints every frame;
    # the final paint has the complete refreshed content).
    positions = []
    for pat, name in order_pats:
        matches = list(re.finditer(pat, clean))
        if matches:
            positions.append((matches[-1].start(), name))
        else:
            positions.append((10**9, name))
    names = [n for _, n in sorted(positions)]
    present = {n for p, n in positions if p < 10**9}
    print("labels found:", len(present), "of 24 ->", sorted(present))
    print("screen order (by last occurrence):", names)
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
        "dcel",
        "tcel",
        "cid",
        "up",
        "size",
    ]
    ok = names == expected and len(present) == 24
    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
