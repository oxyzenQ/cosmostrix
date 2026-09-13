#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-hunter-34: color-bg default-background residue E2E (content-level).

Owner report (2026-09-12, v100.0.0-beta.1): with
`color-bg = "default-background"` the screen keeps physical residue in
four scenarios that are all clean under `color-bg = "black"`:

  S1  intro residue   — after the logo intro, intro-rain glyphs stay
                        stuck on screen; 'r' restart does not clean them.
  S2  'x'/'X' residue — scene switch leaves the old scene's glyphs /
                        charset / colors on screen.
  S2b 'r' residue     — restart shortkey leaves glyph residue.
  S3  bg reload       — live-reload black -> default-background leaves
                        black cells behind the moving rain.
  S4  custom palette  — same as S3 with a colors-custom bg active.

Root cause (unit level, see the fix commit): LastFrame::reuse_or_new
resets the shadow buffer to Cell::blank_with_bg(None) on every
semantic-gen / dimension reset. The HUNT-27 full-redraw cell skip then
treats "frame blank == shadow blank" as "no emit needed" — but the
physical screen still holds the old content. When the frame blank bg is
Some(black) the reset cell differs (accidental full repaint); when it is
None (default-background) the skip fires and the residue survives.

This harness verifies the four scenarios at the REAL terminal boundary:
spawn the app in a PTY, drive it (keys + config live rewrites), render
the ANSI stream through the nh2 mini terminal emulator, and assert the
final screen is clean.

Usage (repo root, release binary built):

  python3 scripts/night_cbg34_e2e.py              # all scenarios
  python3 scripts/night_cbg34_e2e.py S1 S3        # subset
  BIN=/path/to/cosmostrix python3 scripts/night_cbg34_e2e.py

Exit 0 = all requested scenarios clean; exit 1 = residue detected.
"""

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

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from nh2_shift_harness import Screen

# Stray-ESC cleanup: a lone ESC not introducing a known sequence class
# (CSI/OSC/charset/DCS/SOS/PM/APC). The app wraps every frame in 2026
# sync markers (bare ESC); the base Screen would pair such an ESC with
# the NEXT escape (eating a byte) or with a glyph (shifting the row).
LONE_ESC_RE = re.compile(rb"\x1b(?![\[\]()\+\-PX^_])")


class SyncScreen(Screen):
    """Screen that tolerates the 2026-sync (BSU) framing the app emits.

    The app writes `ESC` (sync start) immediately before each frame's
    ANSI body and `ESC[?2026l` (sync end) after it. The base Screen
    pairs `ESC ESC` into one skip (i += 2), leaking sequence bodies into
    the grid as text and eating a glyph byte when the body starts with
    a printable char. This subclass merges/strips the framing first and
    holds back a trailing lone ESC until the next chunk decides it.
    """

    def __init__(self, cols, rows):
        super().__init__(cols, rows)
        self.pre = bytearray()

    def feed(self, data):
        buf = bytes(self.pre) + bytes(data)
        self.pre = bytearray()
        buf = buf.replace(b"\x1b\x1b", b"\x1b")
        buf = LONE_ESC_RE.sub(b"", buf)
        if buf.endswith(b"\x1b"):
            self.pre = bytearray(buf[-1:])
            buf = buf[:-1]
        super().feed(buf)
        # Fold the parent's residual (incomplete sequence) plus our
        # held-back ESC into one prepend for the next feed call.
        if self.residual or self.pre:
            self.pre = bytearray(self.residual) + self.pre
            self.residual = bytearray()


TERM_COLS, TERM_ROWS = (int(x) for x in os.environ.get("SIZE", "140x46").split("x"))
# HUD occupies the bottom ~24 rows; analyze the rain region only.
HUD_ROWS = 26
ANALYSIS_ROWS = TERM_ROWS - HUD_ROWS
BIN = os.environ.get("BIN", "target/release/cosmostrix")

CFG_DEFAULT_BG = 'color-bg = "default-background"\n'
CFG_BLACK_BG = 'color-bg = "black"\n'
CFG_CUSTOM = (
    'color-bg = "default-background"\n\n'
    "[colors-custom.test]\n"
    'bg = "#02031f"\n'
    'rain = ["#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#e6b3ff", "#f2ccff", "#ffffff"]\n'
)

DEFAULT_BG_MARKER = (-1, -1, -1)  # Screen default (SGR 39/49/reset)


class Run:
    """One PTY run: spawn, feed the emulator, schedule actions.

    NIGHT-hunt-34 harness fix: the original run loop did select+read+feed
    inline — one thread for everything. Python-side ANSI parsing
    (Screen.feed) is far slower than the app's frame emission, so during
    parse bursts the PTY buffer filled, the app blocked in write_all, and
    the event loop stalled: a keypress written at t=3.0 was processed up
    to ~2s late (measured: restart_from_zero firing at frame ~430 of an
    86fps run). The 'r' restart then replayed the deterministic RNG
    sequence, and the pre/post snapshot comparison degenerated into
    comparing the same replay ~0.1s apart — live lingering droplet trails
    read as "identical cells" and S2b failed with 0-24 flaky cells.

    The fix decouples the three jobs:
      - reader thread: os.read from the PTY into a queue (never waits on
        parsing — the app's writes always drain);
      - parser thread: owns the SyncScreen, feeds chunks, serves
        snapshot requests through a handshake queue;
      - main loop: fires actions on wall-clock schedule (key/cfg/snap),
        independent of parse load.
    """

    def __init__(self, argv, config_path, actions, run_secs):
        self.argv = argv
        self.config_path = config_path
        self.actions = actions  # list of (t, kind, payload)
        self.run_secs = run_secs
        self.snapshots = {}  # label -> grid copy
        self.proc = None

    def run(self):
        self.master_fd, slave_fd = pty.openpty()
        os.set_blocking(self.master_fd, False)
        fcntl.ioctl(
            slave_fd,
            termios.TIOCSWINSZ,
            struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0),
        )
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
        self.proc = subprocess.Popen(
            self.argv,
            stdin=slave_fd,
            stdout=slave_fd,
            stderr=slave_fd,
            env=env,
            close_fds=True,
        )
        os.close(slave_fd)
        self.screen = SyncScreen(TERM_COLS, TERM_ROWS)

        import queue as queue_mod
        import threading

        raw_q = queue_mod.Queue()
        snap_q = queue_mod.Queue()
        stop_flag = threading.Event()

        def reader():
            # Drain the PTY at full speed. 64 KiB reads keep the kernel
            # buffer empty so the app never blocks on write_all.
            while not stop_flag.is_set():
                r, _, _ = select.select([self.master_fd], [], [], 0.005)
                if not r:
                    continue
                try:
                    chunk = os.read(self.master_fd, 1 << 16)
                except (BlockingIOError, OSError):
                    continue
                if not chunk:
                    break
                raw_q.put(chunk)

        def parser():
            # Owns the screen. Chunks have priority; snapshot requests are
            # served in the gaps so a snap lags at most one chunk-parse.
            while not stop_flag.is_set():
                try:
                    chunk = raw_q.get(timeout=0.002)
                except queue_mod.Empty:
                    chunk = None
                if chunk is not None:
                    self.screen.feed(chunk)
                    continue
                try:
                    label, done = snap_q.get_nowait()
                except queue_mod.Empty:
                    continue
                self.snapshots[label] = [row[:] for row in self.screen.grid]
                done.set()

        reader_t = threading.Thread(target=reader, daemon=True)
        parser_t = threading.Thread(target=parser, daemon=True)
        reader_t.start()
        parser_t.start()

        def take_snapshot(label):
            done = threading.Event()
            snap_q.put((label, done))
            done.wait(timeout=2.0)

        start = time.monotonic()
        pending_actions = sorted(self.actions, key=lambda a: a[0])
        ai = 0
        while True:
            now = time.monotonic()
            if now - start >= self.run_secs or self.proc.poll() is not None:
                break
            while ai < len(pending_actions) and now - start >= pending_actions[ai][0]:
                t, kind, payload = pending_actions[ai]
                try:
                    if kind == "key":
                        os.write(self.master_fd, payload.encode())
                    elif kind == "cfg":
                        with open(self.config_path, "w") as f:
                            f.write(payload)
                    elif kind == "snap":
                        take_snapshot(payload)
                    print(f"    [act] t={t:5.1f}s {kind}={payload!r:.60}", flush=True)
                except OSError:
                    pass
                ai += 1
            time.sleep(0.002)

        stop_flag.set()
        reader_t.join(timeout=1.0)
        parser_t.join(timeout=1.0)
        # Drain any backlog in the main thread, then take the final snap.
        while True:
            try:
                chunk = raw_q.get_nowait()
            except queue_mod.Empty:
                break
            self.screen.feed(chunk)
        self.snapshots["final"] = [row[:] for row in self.screen.grid]
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        os.close(self.master_fd)
        return self.snapshots


def glyph_residue(snap_a, snap_b, rows):
    """Cells that are non-space glyphs and IDENTICAL across two snapshots.

    Healthy rain rewrites every glyph cell (chars move, colors decay) —
    a glyph frozen for the whole window is physical residue.
    """
    stuck = []
    for y in range(min(rows, len(snap_a), len(snap_b))):
        for x in range(len(snap_a[y])):
            a, b = snap_a[y][x], snap_b[y][x]
            if a[0] not in (" ", "") and a == b:
                stuck.append((x, y, a))
    return stuck


def bg_residue(snap, rows, bad_bg):
    """Cells whose bg is an explicit color (not the default marker)."""
    stuck = []
    for y in range(min(rows, len(snap))):
        for x in range(len(snap[y])):
            _ch, _fg, bg, _bold = snap[y][x]
            if bg != DEFAULT_BG_MARKER and bg in bad_bg:
                stuck.append((x, y, snap[y][x]))
    return stuck


def any_colored_bg(snap, rows):
    stuck = []
    for y in range(min(rows, len(snap))):
        for x in range(len(snap[y])):
            if snap[y][x][2] != DEFAULT_BG_MARKER:
                stuck.append((x, y, snap[y][x]))
    return stuck


def write_cfg(path, content):
    with open(path, "w") as f:
        f.write(content)


def scenario(cfg_dir):
    """Shared: build config path + argv helper."""
    path = os.path.join(cfg_dir, "config.toml")

    def run(extra_argv, actions, run_secs, initial_cfg=CFG_DEFAULT_BG):
        write_cfg(path, initial_cfg)
        argv = [BIN, "--config", path] + extra_argv
        return Run(argv, path, actions, run_secs).run()

    return run, path


def main() -> int:
    which = sys.argv[1:] or ["S1", "S2", "S2b", "S3", "S4"]
    # safepath contract: --config must live under ~/.config/cosmostrix/.
    cfg_dir = os.path.expanduser("~/.config/cosmostrix/cbg34")
    os.makedirs(cfg_dir, exist_ok=True)
    failures = []

    if "S1" in which:
        print("[S1] intro residue (color-bg=default-background, logo intro)")
        run, _ = scenario(cfg_dir)
        snaps = run(
            ["-c", "cosmos"],
            [(6.0, "snap", "mid"), (11.0, "snap", "end")],
            run_secs=11.5,
        )
        stuck = glyph_residue(snaps["mid"], snaps["end"], ANALYSIS_ROWS)
        print(f"    glyph residue: {len(stuck)} cells")
        for x, y, cell in stuck[:8]:
            print(f"      ({x},{y}) {cell!r}")
        if len(stuck) > 5:
            failures.append("S1 intro residue")

    if "S2" in which:
        print("[S2] 'x' scene-switch residue (default-background, intro none)")
        run, _ = scenario(cfg_dir)
        snaps = run(
            ["--intro", "none"],
            [
                (2.9, "snap", "pre"),
                (3.0, "key", "x"),
                (8.0, "snap", "post"),
            ],
            run_secs=8.5,
        )
        stuck = glyph_residue(snaps["pre"], snaps["post"], ANALYSIS_ROWS)
        print(f"    glyph residue: {len(stuck)} cells")
        for x, y, cell in stuck[:8]:
            print(f"      ({x},{y}) {cell!r}")
        if len(stuck) > 5:
            failures.append("S2 scene-switch residue")

    if "S2b" in which:
        print("[S2b] 'r' restart residue (default-background, intro none)")
        run, _ = scenario(cfg_dir)
        # NIGHT-hunt-34: multi-checkpoint persistence. The single pre/post
        # comparison flags live lingering trails as residue — a parked head
        # freezes its trail (fixed dist-from-head colors) for up to 3s
        # (rand_linger_ms max 3000), and the restart's deterministic RNG
        # replay redraws near-identical trail layouts at the post snapshot,
        # so 0-24 live cells read as "identical" (flaky FAIL). Real residue
        # never changes: a cell must be identical across the pre snapshot
        # AND every post-restart checkpoint (5.1s of total freeze) to count.
        # A live parked trail always mutates within its <=3s linger window,
        # so at least one checkpoint differs.
        snaps = run(
            ["--intro", "none"],
            [
                (2.9, "snap", "pre"),
                (3.0, "key", "r"),
                (4.0, "snap", "p1"),
                (5.0, "snap", "p2"),
                (6.0, "snap", "p3"),
                (7.0, "snap", "p4"),
                (8.0, "snap", "post"),
            ],
            run_secs=8.5,
        )
        labels = ["pre", "p1", "p2", "p3", "p4", "post"]
        grids = [snaps[l] for l in labels]
        row_cap = min(ANALYSIS_ROWS, *(len(g) for g in grids))
        stuck = []
        for y in range(row_cap):
            for x in range(len(grids[0][y])):
                c0 = grids[0][y][x]
                if c0[0] in (" ", ""):
                    continue
                if all(g[y][x] == c0 for g in grids[1:]):
                    stuck.append((x, y, c0))
        print(f"    glyph residue: {len(stuck)} cells")
        for x, y, cell in stuck[:8]:
            print(f"      ({x},{y}) {cell!r}")
        if len(stuck) > 5:
            failures.append("S2b restart residue")

    if "S3" in which:
        print("[S3] bg live-reload black -> default-background (with 'x' between)")
        run, _ = scenario(cfg_dir)
        snaps = run(
            ["--intro", "none"],
            [
                (2.5, "cfg", CFG_BLACK_BG),
                (4.0, "key", "x"),
                (5.5, "cfg", CFG_DEFAULT_BG),
            ],
            run_secs=9.0,
        )
        stuck = bg_residue(
            snaps["final"],
            ANALYSIS_ROWS,
            bad_bg={(0, 0, 0), (-2, 40, -1), (-3, 16, -1)},
        )
        print(f"    black bg residue: {len(stuck)} cells")
        for x, y, cell in stuck[:8]:
            print(f"      ({x},{y}) {cell!r}")
        if len(stuck) > 5:
            failures.append("S3 bg reload residue")

    if "S4" in which:
        print(
            "[S4] custom palette bg residue ('x' then live-removal of the custom bg field)"
        )
        run, _ = scenario(cfg_dir)
        cfg_nobg = CFG_CUSTOM.replace('bg = "#02031f"\n', "")
        snaps = run(
            ["-c", "test", "--intro", "none"],
            [
                (2.5, "key", "x"),
                (4.0, "cfg", cfg_nobg),
            ],
            run_secs=8.0,
            initial_cfg=CFG_CUSTOM,
        )
        stale_custom = bg_residue(snaps["final"], ANALYSIS_ROWS, bad_bg={(2, 3, 31)})
        black_res = bg_residue(
            snaps["final"],
            ANALYSIS_ROWS,
            bad_bg={(0, 0, 0), (-2, 40, -1), (-3, 16, -1)},
        )
        print(
            f"    stale custom bg: {len(stale_custom)} cells, black residue: {len(black_res)}"
        )
        for x, y, cell in (stale_custom + black_res)[:8]:
            print(f"      ({x},{y}) {cell!r}")
        if len(stale_custom) + len(black_res) > 5:
            failures.append("S4 custom palette bg residue")

    print()
    if failures:
        print("FAIL: " + "; ".join(failures))
        return 1
    print("PASS: all requested scenarios clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
