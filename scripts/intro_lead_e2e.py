#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""E2E verification for the v52 message-intro-lead fix (owner bug report).

Owner repro: `cosmostrix --mfs engrave -mb test` — with the default logo
intro the message lands shortly after the cinematic (tuned, good). But
`--intro none` and the Space runtime restart re-armed the 6 s lead with
nothing hiding the message, so the overlay dead-aired for 6 s.

Detection strategy: run with `-C digits` so the rain can never emit the
message glyph X (also absent from the logo art and the cosmic burst
chars) — every X in the output stream is a message-overlay cell write.

Scenarios:
  A  --intro none          : first X fast (<= 4 s); full reveal by 5 s.
                             (Pre-fix behavior: nothing until >= 6 s.)
  B  --intro logo (default): NO X before the intro finishes (<= 3.8 s),
                             full reveal by 10 s — locks the tuned feel.
                             (Broken-skip behavior: X within ~1 s.)
  C  --intro none + Space  : press Space at t=3 s (message fully typed),
                             the fresh replay re-reveals within 1.3 s.
                             (Pre-fix: single-char dead air for 6 s.)
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
MSG = "XXXXXXXXXXXX"  # 12 glyphs; X never appears in rain/logo/burst


def strip_ansi(buf: bytes) -> str:
    text = buf.decode("utf-8", errors="replace")
    text = re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]", "", text)
    text = re.sub(r"\x1b\][^\x07]*\x07", "", text)
    text = re.sub(r"\x1b[()][0-9A-B]", "", text)
    return text


def run_scenario(args, actions=None, duration=6.0):
    """Spawn the binary in a PTY; record (t, x_count) timeline of X writes.

    actions: list of (delay_seconds, bytes) to write to the PTY.
    Returns a list of (elapsed_seconds, cumulative_x_count) samples.
    """
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(BIN, [BIN, "-C", "digits", "-mb", MSG] + args)

    # A fresh PTY is 1x1 — set the winsize BEFORE the app measures it
    # (hud_order_e2e.py lesson: else the intro is skipped as too-small).
    import fcntl
    import struct
    import termios

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    stop = threading.Event()
    samples = []  # (t, cumulative X count)
    lock = threading.Lock()
    t0 = time.monotonic()

    def reader():
        total = 0
        while not stop.is_set():
            try:
                r, _, _ = select.select([fd], [], [], 0.05)
                if not r:
                    continue
                chunk = os.read(fd, 65536)
                if not chunk:
                    break
                total += strip_ansi(chunk).count("X")
                with lock:
                    samples.append((time.monotonic() - t0, total))
            except OSError:
                break

    t = threading.Thread(target=reader, daemon=True)
    t.start()

    for delay, payload in actions or []:
        wait = delay - (time.monotonic() - t0)
        if wait > 0:
            time.sleep(wait)
        os.write(fd, payload)

    end = time.monotonic() + duration - (time.monotonic() - t0)
    while time.monotonic() < end:
        time.sleep(0.1)
    os.write(fd, b"q")
    time.sleep(0.8)
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
    return samples


def count_at(samples, t):
    """Cumulative X count at or before wall-time t."""
    return max((c for ts, c in samples if ts <= t), default=0)


def first_time(samples):
    """Wall time of the first X write (None if never)."""
    for ts, c in samples:
        if c > 0:
            return ts
    return None


def main():
    failures = []

    # ── Scenario A: --intro none reveals immediately ─────────────────────
    a = run_scenario(["--intro", "none"], duration=5.0)
    a_first = first_time(a)
    a_by_5 = count_at(a, 5.0)
    print(f"A  --intro none      : first X at {a_first}s, X writes by 5s = {a_by_5}")
    if a_first is None or a_first > 4.0:
        failures.append(
            "A: first message glyph must appear within 4 s (pre-fix: >= 6 s)"
        )
    if a_by_5 < 8:
        failures.append("A: full reveal (>= 8 X writes) must complete by 5 s")

    # ── Scenario B: default logo intro keeps the tuned lead ──────────────
    b = run_scenario([], duration=10.0)  # default intro = logo
    b_before_38 = count_at(b, 3.8)
    b_by_10 = count_at(b, 10.0)
    b_first = first_time(b)
    print(
        f"B  --intro logo      : X writes before 3.8s = {b_before_38}, "
        f"first X at {b_first}s, X writes by 10s = {b_by_10}"
    )
    if b_before_38 != 0:
        failures.append(
            "B: no message glyph may appear before the intro finishes (~4.5 s)"
        )
    if b_first is None or b_first < 3.8:
        failures.append(
            "B: the intro lead must hide the message until after the cinematic"
        )
    if b_by_10 < 8:
        failures.append("B: full reveal must complete by 10 s (lead expires ~6.2 s)")

    # ── Scenario C: Space restart replays immediately ────────────────────
    c = run_scenario(
        ["--intro", "none"],
        actions=[(3.0, b" ")],  # Space at t=3 s: reset + message replay
        duration=5.0,
    )
    c_pre = count_at(c, 3.0)
    c_window = count_at(c, 4.5) - count_at(c, 3.2)
    print(
        f"C  --intro none +Space: X writes before Space = {c_pre}, "
        f"fresh replay X writes in [3.2s, 4.5s] = {c_window}"
    )
    if c_pre < 8:
        failures.append("C precondition: initial reveal must be complete before Space")
    if c_window < 8:
        failures.append(
            "C: Space restart must re-reveal within ~1.3 s (pre-fix: 6 s dead air)"
        )

    print()
    if failures:
        for f in failures:
            print("FAIL:", f)
        print("RESULT: FAIL")
        return 1
    print("RESULT: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
