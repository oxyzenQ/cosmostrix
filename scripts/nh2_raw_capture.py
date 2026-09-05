#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-hunter-2 round 2, raw fast-drain capture + offline analysis.

The shift harness parses inline, which makes the Python reader the
bottleneck and pushes cosmostrix into its marginal-drain regime (prs
0.8+, dsty floored) — NOT the owner's fast-Alacritty regime (prs ~0).
This tool spools the PTY byte stream to disk at full drain speed (the
app never blocks on the reader), then replays it offline through the
same mini terminal emulator for per-frame churn/composition analysis.

Usage (repo root, release binary built):

  KEYS_AT=8:i,70:s RUN_SECS=90 python3 scripts/nh2_raw_capture.py
  python3 scripts/nh2_raw_capture.py analyze            # replay + report

Knobs (env): RUN_SECS, KEYS_AT ("t:ch" or bare "t" = 's'), SIZE,
SCENE, BIN, RAW (default /tmp/cosmostrix-nh2/raw.bin).
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import fcntl
import pty
import re
import select
import signal
import struct
import subprocess
import termios
import time

from nh2_shift_harness import (  # noqa: E402
    BSU_OFF,
    BSU_ON,
    Screen,
    analyze,
    cell_kind,
    classify,
    composition,
    row_occupancy,
)

TERM_COLS, TERM_ROWS = (int(x) for x in os.environ.get("SIZE", "200x56").split("x"))
RUN_SECS = float(os.environ.get("RUN_SECS", "75"))
KEYS_RAW = os.environ.get("KEYS_AT", "")
SCENE = os.environ.get("SCENE", "")
BIN = os.environ.get("BIN", "target/release/cosmostrix")
RAW = os.environ.get("RAW", "/tmp/cosmostrix-nh2/raw.bin")
TERM_PROGRAM = os.environ.get("TERM_PROGRAM", "alacritty")


def capture() -> int:
    keys = []
    for entry in KEYS_RAW.split(","):
        entry = entry.strip()
        if not entry:
            continue
        if ":" in entry:
            t_s, ch = entry.split(":", 1)
        else:
            t_s, ch = entry, "s"
        keys.append((float(t_s), ch[0]))

    master_fd, slave_fd = pty.openpty()
    os.set_blocking(master_fd, False)
    fcntl.ioctl(
        slave_fd, termios.TIOCSWINSZ, struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0)
    )
    env = dict(os.environ)
    for k in ("NO_COLOR", "CLICOLOR", "CLICOLOR_FORCE"):
        env.pop(k, None)
    env.update(
        {
            "TERM": "xterm-256color",
            "COLORTERM": "truecolor",
            "TERM_PROGRAM": TERM_PROGRAM,
            "COLUMNS": str(TERM_COLS),
            "LINES": str(TERM_ROWS),
        }
    )
    argv = [BIN]
    if SCENE:
        argv += ["--scene", SCENE]
    proc = subprocess.Popen(
        argv, stdin=slave_fd, stdout=slave_fd, stderr=slave_fd, env=env, close_fds=True
    )
    os.close(slave_fd)

    start = time.monotonic()
    deadline = start + RUN_SECS
    next_key = 0
    os.makedirs(os.path.dirname(RAW), exist_ok=True)
    chunks = []  # (t_rel, bytes)
    total = 0

    with open(RAW, "wb") as f:
        while True:
            now = time.monotonic()
            if now >= deadline or proc.poll() is not None:
                break
            while next_key < len(keys) and now - start >= keys[next_key][0]:
                t, ch = keys[next_key]
                try:
                    os.write(master_fd, ch.encode())
                    print(f"[key] t={now - start:6.1f}s sent {ch!r}", flush=True)
                except OSError:
                    pass
                next_key += 1
            r, _, _ = select.select([master_fd], [], [], 0.001)
            if not r:
                continue
            try:
                chunk = os.read(master_fd, 1 << 22)
            except (BlockingIOError, OSError):
                continue
            if not chunk:
                break
            chunks.append((time.monotonic() - start, len(chunk)))
            f.write(chunk)
            total += len(chunk)

    try:
        proc.send_signal(signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()

    with open(RAW + ".tsv", "w") as f:
        f.write("# t_rel\tbytes\n")
        f.writelines(f"{t:.6f}\t{b}\n" for t, b in chunks)
    dur = time.monotonic() - start
    print(f"captured {total/1e6:.1f} MB over {dur:.0f}s -> {RAW} ({len(chunks)} chunks)")
    return 0


def analyze_stream() -> int:
    data = open(RAW, "rb").read()
    ts = []
    for line in open(RAW + ".tsv"):
        if line.startswith("#"):
            continue
        t_s, b_s = line.split("\t")
        ts.append((float(t_s), int(b_s)))

    screen = Screen(TERM_COLS, TERM_ROWS)
    prev_snap = None
    frame_idx = 0
    # chunk-time interpolation: frames are timestamped by the chunk they end in
    events = []
    hud_rows = []
    last_hud_t = -1.0
    i = 0
    n = len(data)
    chunk_i = 0
    bytes_seen = 0
    t_now = 0.0

    def advance_chunk(pos):
        nonlocal chunk_i, bytes_seen, t_now
        while chunk_i < len(ts) - 1 and bytes_seen + ts[chunk_i][1] <= pos:
            bytes_seen += ts[chunk_i][1]
            chunk_i += 1
            t_now = ts[chunk_i][0]

    pending = bytearray()
    in_frame = False
    while i < n:
        # feed a bounded slice at a time and advance chunk timestamps
        step = 1 << 20
        slice_end = min(i + step, n)
        pending.extend(data[i:slice_end])
        i = slice_end
        advance_chunk(slice_end)
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
                frame_idx += 1
                snap = screen.snapshot()
                if prev_snap is not None:
                    shifted, churn = analyze(prev_snap, snap)
                    mass = {lag: ys for lag, ys in shifted.items() if len(ys) >= 5}
                    t_end = t_now
                    if churn > 300 or mass:
                        cc = classify(prev_snap, snap)
                        cc_s = " ".join(
                            f"{k}={v}" for k, v in sorted(cc.items(), key=lambda kv: -kv[1])
                        )
                        g, c, b = composition(snap)
                        events.append(
                            f"t={t_end:7.2f}s f#{frame_idx:7d} churn={churn:5d} "
                            f"G={g:5d} C={c:5d} B={b:5d} {cc_s}"
                        )
                        for lag, ys in sorted(mass.items()):
                            events.append(
                                f"    MASS-LAG {lag:+3d} rows={len(ys):3d} "
                                f"({','.join(map(str, ys[:10]))})"
                            )
                    if t_now - last_hud_t >= 0.5:
                        last_hud_t = t_now
                        text = "\n".join("".join(c[0] for c in row) for row in screen.grid)
                        m_p = re.search(r"prs:\s*([0-9.]+)", text)
                        m_d = re.search(r"dsty:\s*([0-9.]+)", text)
                        m_f = re.search(r"fps:\s*([0-9.]+)", text)
                        m_e = re.search(r"ehs:\s*([0-9.]+)", text)
                        g, c, b = composition(snap)
                        hud_rows.append(
                            (t_now, m_p, m_d, m_f, m_e, g, c, b)
                        )
                prev_snap = snap
                in_frame = False
            else:
                on = pending.find(BSU_ON)
                if on == -1:
                    screen.feed(pending)
                    pending.clear()
                    break
                pre = bytes(pending[:on])
                if pre:
                    screen.feed(pre)
                pending = pending[on + len(BSU_ON) :]
                in_frame = True

    print(f"frames={frame_idx}")
    print("\nEVENTS (churn>300 or mass-lag):")
    for e in events:
        print(" ", e)
    print("\nHUD samples (t, prs, dsty, fps, ehs, G, C, B) 1/2s:")
    for t, m_p, m_d, m_f, m_e, g, c, b in hud_rows:
        print(
            f"  t={t:7.2f}s prs={m_p.group(1) if m_p else '-':>5} "
            f"dsty={m_d.group(1) if m_d else '-':>5} fps={m_f.group(1) if m_f else '-':>6} "
            f"ehs={m_e.group(1) if m_e else '-':>4} G={g:5d} C={c:5d} B={b:5d}"
        )
    return 0


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "analyze":
        sys.exit(analyze_stream())
    sys.exit(capture())
