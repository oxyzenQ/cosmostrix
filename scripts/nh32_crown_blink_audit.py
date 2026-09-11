#!/usr/bin/env python3
# Internal research: NIGHT-hunt-32 PTY capture — black hole crown blink audit.
#
# Two checks on the real binary via PTY byte-stream analysis:
#
# 1. GENESIS CHECK (the hunt-31 revert): the startup must carry the
#    full stellar-collapse formation after the intro handover — a
#    ~1.4 s near-empty seed-dot dwell, a ~0.5 s collapse cross, then
#    the horizon bloom. We look for the near-empty frames window
#    (frames with 1-5 visible cells) right after the intro content
#    ends.
#
# 2. BLINK CHECK (the hunt-32 fix): in steady state the three upper
#    crowns must never mass-blank. The stuck-cell sweep bug force-
#    cleared up to 256 live cells in one frame every 600 frames —
#    the signature is a frame whose emission contains a burst of
#    blank writes (space cells) concentrated in the upper band while
#    the scene is dense. With the fix, no such burst exists in a run
#    that crosses multiple 600-frame boundaries.

import fcntl
import os
import pty
import re
import select
import struct
import subprocess
import sys
import termios
import time

TERM_COLS, TERM_ROWS = (int(x) for x in os.environ.get("SIZE", "120x40").split("x"))
RUN_SECS = float(os.environ.get("RUN_SECS", "16"))
BIN = os.environ.get("BIN", "target/debug/cosmostrix")
SCENE = os.environ.get("SCENE", "sorgonemous_intrascals")
INTRO = os.environ.get("INTRO", "1")  # 1 = logo intro (default), 0 = no intro


def capture() -> tuple:
    master, slave = pty.openpty()
    fcntl.ioctl(
        master, termios.TIOCSWINSZ, struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0)
    )
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["TERM_PROGRAM"] = "alacritty"
    env["COLORTERM"] = "truecolor"
    args = [BIN, "--scene", SCENE, "--duration", f"{RUN_SECS}"]
    if INTRO == "0":
        args.append("--intro=none")
    if os.environ.get("MSG_MODE", "1") == "0":
        # The owner's read: no message banner. NOTE: with the default
        # banner active, the sweep's message gate never lets it run at
        # all — the blink only exists in msg-mode=false runs.
        args.append("--msg-mode=false")
    proc = subprocess.Popen(args, stdin=slave, stdout=slave, stderr=slave, env=env)
    os.close(slave)
    buf = bytearray()
    t0 = time.time()
    deadline = t0 + RUN_SECS + 3.0
    while time.time() < deadline:
        r, _, _ = select.select([master], [], [], 0.1)
        if r:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                break
            if not chunk:
                break
            buf.extend(chunk)
        if proc.poll() is not None:
            while True:
                r, _, _ = select.select([master], [], [], 0.2)
                if not r:
                    break
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                buf.extend(chunk)
            break
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
    os.close(master)
    return bytes(buf)


def split_frames(raw: bytes):
    frames = []
    off = 0
    n = len(raw)
    while off < n:
        end = raw.find(b"\x1b[?2026l", off)
        if end == -1:
            break
        frames.append(raw[off : end + 8])
        off = end + 8
    return frames


# A frame's "visible cells" heuristic: count SGR-prefixed non-space
# glyph runs. A blanked cell emits as SGR + space (possibly several).
# We count: (a) colored glyph cells (non-space after SGR), (b) blank
# writes (space runs after SGR with no other attribute meaning).
SGR = re.compile(rb"\x1b\[([0-9;]*)m")


def count_cells(frame: bytes):
    glyphs = 0
    blanks = 0
    # Walk SGR-then-content chunks.
    for m in SGR.finditer(frame):
        seg_start = m.end()
        next_m = SGR.search(frame, seg_start)
        seg_end = next_m.start() if next_m else min(len(frame), seg_start + 400)
        seg = frame[seg_start:seg_end]
        # Strip cursor-control sequences, keep plain text.
        text = re.sub(rb"\x1b\[[0-9;?]*[A-Za-z]", b"", seg)
        for ch in text:
            if ch == 0x20:
                blanks += 1
            elif 0x21 <= ch <= 0x7E:
                glyphs += 1
    return glyphs, blanks


def main():
    raw = capture()
    frames = split_frames(raw)
    print(f"captured {len(raw)} bytes, {len(frames)} frames in {RUN_SECS}s")

    # --- Genesis check: near-empty frames window after intro ---
    # Frame visible-glyph counts; the seed-dot dwell = consecutive
    # frames with 1..5 glyphs, appearing shortly after the intro's
    # dense frames.
    counts = []
    for i, fr in enumerate(frames):
        g, b = count_cells(fr)
        counts.append((i, g, b))

    # Find the last dense frame of the intro (glyph count > 300),
    # then look for the dwell window within the next 140 frames.
    # The dwell is mostly ZERO-emission frames (the diff renderer
    # skips identical content — a static seed dot emits nothing),
    # plus the occasional shimmer re-roll (1-5 glyphs).
    genesis_ok = False
    dwell_report = "not found"
    last_dense_intro = None
    for i, g, _ in counts:
        if g > 300:
            last_dense_intro = i
    if last_dense_intro is not None:
        window = counts[last_dense_intro + 1 : last_dense_intro + 140]
        run = 0
        best = 0
        for _, g, _ in window:
            if g <= 5:
                run += 1
                best = max(best, run)
            else:
                run = 0
        genesis_ok = best >= 20  # ~0.33 s minimum of dwell frames at 60fps
        dwell_report = f"dwell run of {best} near-empty/zero frames after intro frame {last_dense_intro}"
    print(f"GENESIS: {'PASS' if genesis_ok else 'FAIL'} ({dwell_report})")

    # --- Blink check: blank-count spike frames in steady state ---
    # Steady state starts after the bloom (~4 s in). The sweep blink
    # emits a frame carrying up to 256 blank writes ON TOP of the
    # normal motion glyphs (the sweep clears live cells drawn in the
    # same frame) — the signature is a blank-count SPIKE, not a
    # glyph-poor frame. Baseline blank counts in dense motion run
    # far below 100; a sweep frame lands near 256.
    steady_start = int(len(frames) * 0.35)
    bursts = []
    max_b = 0
    for i, g, b in counts[steady_start:]:
        max_b = max(max_b, b)
        if b >= 150:
            bursts.append((i, g, b))
    print(
        f"BLINK: {'PASS' if not bursts else 'FAIL'} "
        f"({len(bursts)} spike frames >=150 blanks in steady state, max blanks {max_b})"
    )
    for i, g, b in bursts[:5]:
        print(f"  burst at frame {i}: glyphs={g} blanks={b}")
    ok = genesis_ok and not bursts
    print(f"OVERALL: {'PASS' if ok else 'FAIL'}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
