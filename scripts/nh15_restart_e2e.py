#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-hunter-15: 'r' restart clear E2E (content-level).

Owner report (2026-09-06, post-e58f8b8): pressing 'r' on glyph rain did
not reset the field — the screen appeared stuck, then rain resumed on
top of the old residue ("bekas rain sebelumnya yang stuck"). Root cause
(unit-level, pinned in tests_restart_hunt15.rs): reset_with_bounds armed
semantic_invalidate only for structured styles, so the glyph force flag
was consumed by the HUNT-25 resync path (force_repaint — re-emit, no
clear). This harness verifies the fix at the REAL terminal boundary:
spawn the app in a PTY, send 'r' mid-run, render the ANSI stream through
the nh2 mini terminal emulator, and assert the screen actually blanks
(then refills from the top) — the pre-fix binary keeps the old glyphs.

Usage (repo root, release binary built):

  python3 scripts/nh15_restart_e2e.py            # 9 s run, 'r' at t=3
  BIN=/path/to/cosmostrix python3 scripts/nh15_restart_e2e.py
  SCENE=matrix python3 scripts/nh15_restart_e2e.py

Exit 0 = restart clears (fixed); exit 1 = residue retained (regressed).
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

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from nh2_shift_harness import Screen  # noqa: E402  (shared mini emulator)

TERM_COLS, TERM_ROWS = (int(x) for x in os.environ.get("SIZE", "200x56").split("x"))
RUN_SECS = float(os.environ.get("RUN_SECS", "9"))
RESTART_AT = float(os.environ.get("RESTART_AT", "3"))
SCENE = os.environ.get("SCENE", "")
BIN = os.environ.get("BIN", "target/release/cosmostrix")

BSU_ON = b"\x1b"
BSU_OFF = b"\x1b[?2026l"


def visible_glyphs(screen):
    n = 0
    for row in screen.grid:
        for cell in row:
            ch = cell[0]
            if ch not in (" ", ""):
                n += 1
    return n


def main() -> int:
    master_fd, slave_fd = pty.openpty()
    os.set_blocking(master_fd, False)
    fcntl.ioctl(slave_fd, termios.TIOCSWINSZ, struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0))

    env = dict(os.environ)
    for k in ("NO_COLOR", "CLICOLOR", "CLICOLOR_FORCE"):
        env.pop(k, None)
    env.update(
        {
            "TERM": "xterm-256color",
            "COLORTERM": "truecolor",
            "TERM_PROGRAM": "alacritty",
            "COLUMNS": str(TERM_COLS),
            "LINES": str(TERM_ROWS),
        }
    )

    argv = [BIN, "--intro", "none"]
    if SCENE:
        argv += ["--scene", SCENE]
    proc = subprocess.Popen(
        argv, stdin=slave_fd, stdout=slave_fd, stderr=slave_fd, env=env, close_fds=True
    )
    os.close(slave_fd)

    start = time.monotonic()
    deadline = start + RUN_SECS
    pending = bytearray()
    in_frame = False
    screen = Screen(TERM_COLS, TERM_ROWS)
    sent_r = False
    pre_count = 0
    post_frames = []  # (t, visible) for frames after the 'r'
    cleared = False
    rebuilt = False

    while True:
        now = time.monotonic()
        if now >= deadline or proc.poll() is not None:
            break
        if not sent_r and now - start >= RESTART_AT:
            try:
                os.write(master_fd, b"r")
                sent_r = True
                print(f"[key] t={now - start:5.1f}s sent 'r'", flush=True)
            except OSError:
                pass
        r, _, _ = select.select([master_fd], [], [], 0.002)
        if not r:
            continue
        try:
            chunk = os.read(master_fd, 1 << 20)
        except (BlockingIOError, OSError):
            continue
        if not chunk:
            break
        if screen.residual:
            pending[:0] = screen.residual
            screen.residual = bytearray()
        pending.extend(chunk)

        while True:
            if in_frame:
                off = pending.find(BSU_OFF)
                if off == -1:
                    screen.feed(pending)
                    pending.clear()
                    break
                body = bytes(pending[:off])
                screen.feed(body)
                pending = pending[off + len(BSU_OFF) :]
                t_end = time.monotonic() - start
                vis = visible_glyphs(screen)
                if not sent_r:
                    pre_count = max(pre_count, vis)
                else:
                    post_frames.append((t_end, vis))
                in_frame = False
            else:
                off = pending.find(BSU_ON)
                if off == -1:
                    screen.feed(pending)
                    pending.clear()
                    break
                pending = pending[off:]
                if pending[: len(BSU_OFF) + 1] == BSU_OFF + b"\x1b":
                    continue
                # begin-synchronized-update marker
                if pending.startswith(b"\x1b[?2026h"):
                    pending = pending[len(b"\x1b[?2026h") :]
                    in_frame = True
                    continue
                # some other escape starting with \x1b — feed one byte to
                # the parser so it can consume the sequence properly
                screen.feed(bytes(pending[:1]))
                pending = pending[1:]

    try:
        os.kill(proc.pid, signal.SIGTERM)
        proc.wait(timeout=3)
    except (OSError, subprocess.TimeoutExpired):
        proc.kill()

    # Analysis: frames after 'r'.
    # 1. An IMMEDIATE clear: within CLEAR_WINDOW (0.3 s) of the key the
    #    screen must blank to < 10% of the pre-'r' count. The fixed app
    #    clears on the FIRST frame after processing the key (tens of
    #    ms). The pre-fix binary only "recovers" late (0.4 s - 1.4 s,
    #    timing-dependent, via unrelated phosphor/thaw side effects) or
    #    not at all on slow-drain terminals — the owner's stuck-residue
    #    report. The immediate blank is the documented restart contract:
    #    "semua rain dikosongkan lalu turun dari atas".
    # 2. A rebuild: after the clear the count climbs back above 25%
    #    (rain refills from the top).
    if pre_count < 100:
        print(f"FAIL: pre-restart glyph count too low ({pre_count}) — harness issue")
        return 1
    clear_deadline = RESTART_AT + 0.3
    for t, vis in post_frames:
        if t > clear_deadline:
            break
        if vis < pre_count * 0.10:
            cleared = True
            print(f"[ok] t={t:5.2f}s screen cleared ({vis}/{pre_count} glyphs, +{t - RESTART_AT:.2f}s)")
            break
    if cleared:
        for t, vis in post_frames:
            if t > RESTART_AT + 1.0 and vis > pre_count * 0.25:
                rebuilt = True
                print(f"[ok] t={t:5.2f}s rain rebuilt ({vis} glyphs)")
                break

    print(f"summary: pre={pre_count} cleared={cleared} rebuilt={rebuilt}")
    if cleared and rebuilt:
        print("PASS: 'r' restart blanks the screen and refills from the top")
        return 0
    if not cleared:
        print(
            "FAIL: no clear frame after 'r' — pre-restart residue retained "
            "(the owner's 'bekas rain stuck' symptom)"
        )
        return 1
    print("WARN: cleared but rebuild not observed within the window (extend RUN_SECS)")
    return 1


if __name__ == "__main__":
    sys.exit(main())
