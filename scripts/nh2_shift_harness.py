#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-hunter-2 round 2: content-level "glitch rain shift" detector.

The first round (nh2_pty_harness.py) measured the output CADENCE (frame
sizes, gaps). The owner's post-e3d1834 report needs CONTENT analysis:
the shift reportedly (a) lands in the first 9-40 s, (b) re-appears for a
few seconds on the FIRST charset/color shortkey after a long clean run,
(c) never reproduces on the monolith scene. This harness renders the
ANSI byte stream through a minimal terminal emulator (cursor moves, SGR
state, autowrap, BSU frame boundaries) and, after every synchronized
frame, diffs the reconstructed screen against the previous snapshot to
detect horizontal row shifts (occupancy cross-correlation at non-zero
lag) plus per-frame glyph churn.

Usage (repo root, release binary built):

  python3 scripts/nh2_shift_harness.py                     # 75 s, no keys
  KEYS_AT=70: python3 scripts/nh2_shift_harness.py         # 's' at t=70
  KEYS_AT=70: SCENE=monolith: python3 scripts/nh2_shift_harness.py
  KEYS_AT=50,65,80: python3 scripts/nh2_shift_harness.py   # s, c, s

Knobs (env):
  RUN_SECS   wall seconds (default 75).
  KEYS_AT    comma list of "t:ch" entries; "70" alone means "70:s".
  SIZE       "cols x rows" (default 200x56).
  SCENE      scene argument (--scene <name>), default none.
  BIN        binary path (default target/release/cosmostrix).
  OUT        report TSV path (default /tmp/cosmostrix-nh2/shift.tsv).
"""

import fcntl
import os
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import termios
import time
import unicodedata

TERM_COLS, TERM_ROWS = (int(x) for x in os.environ.get("SIZE", "200x56").split("x"))
RUN_SECS = float(os.environ.get("RUN_SECS", "75"))
KEYS_RAW = os.environ.get("KEYS_AT", "")
SCENE = os.environ.get("SCENE", "")
BIN = os.environ.get("BIN", "target/release/cosmostrix")
OUT = os.environ.get("OUT", "/tmp/cosmostrix-nh2/shift.tsv")
TERM_PROGRAM = os.environ.get("TERM_PROGRAM", "alacritty")

BSU_ON = b"\x1b[?2026h"
BSU_OFF = b"\x1b[?2026l"

DEFAULT_FG = (-1, -1, -1)  # marker: SGR reset / unspecified


def char_width(ch):
    if unicodedata.combining(ch):
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


class Screen:
    """Minimal terminal emulator: (char, fg, bg, bold) grid + cursor.

    Stream-safe: unconsumed tail bytes (partial escape sequences split
    across feed() chunks) are kept in `residual` and prepended on the
    next call, so sequences never leak into the grid as glyphs.
    """

    def __init__(self, cols, rows):
        self.cols = cols
        self.rows = rows
        self.x = 0
        self.y = 0
        self.fg = DEFAULT_FG
        self.bg = DEFAULT_FG
        self.bold = False
        self.residual = bytearray()
        # grid[y][x] = (ch, fg, bg, bold)
        self.grid = [[(" ", DEFAULT_FG, DEFAULT_FG, False) for _ in range(cols)] for _ in range(rows)]

    def put(self, ch):
        w = char_width(ch)
        if w == 0:
            return
        if self.x >= self.cols:
            self.x = 0
            self.y = min(self.y + 1, self.rows - 1)
        if self.y < self.rows:
            cell = (ch, self.fg, self.bg, self.bold)
            self.grid[self.y][self.x] = cell
            if w == 2 and self.x + 1 < self.cols:
                self.grid[self.y][self.x + 1] = ("", self.fg, self.bg, self.bold)
        self.x += w

    def move_to(self, row, col):
        self.x = min(max(col, 0), self.cols - 1)
        self.y = min(max(row, 0), self.rows - 1)

    def feed(self, data):
        data = bytes(self.residual) + bytes(data)
        self.residual = bytearray()
        i = 0
        n = len(data)
        while i < n:
            b = data[i]
            if b == 0x1B and i + 1 < n:
                if data[i + 1] == 0x5B:  # CSI
                    j = i + 2
                    params = bytearray()
                    while j < n and 0x30 <= data[j] <= 0x3F:
                        params.append(data[j])
                        j += 1
                    inter = bytearray()
                    while j < n and 0x20 <= data[j] <= 0x2F:
                        inter.append(data[j])
                        j += 1
                    if j >= n:
                        self.residual = bytearray(data[i:])
                        return
                    final = data[j]
                    self.csi(bytes(params), final)
                    i = j + 1
                    continue
                if data[i + 1] == 0x5D:  # OSC: consume until BEL or ST
                    j = i + 2
                    while j < n and data[j] != 0x07:
                        if data[j] == 0x1B and j + 1 < n and data[j + 1] == 0x5C:
                            break
                        j += 1
                    if j >= n:
                        self.residual = bytearray(data[i:])
                        return
                    i = j + 2 if data[j] == 0x1B else j + 1
                    continue
                if data[i + 1] in (0x28, 0x29, 0x2B, 0x2D):  # charset designation
                    if i + 2 < n:
                        i += 3
                    else:
                        self.residual = bytearray(data[i:])
                        return
                    continue
                if data[i + 1] in (0x50, 0x58, 0x5E, 0x5F):  # DCS/SOS/PM/APC: until ST
                    j = i + 2
                    while j < n:
                        if data[j] == 0x1B and j + 1 < n and data[j + 1] == 0x5C:
                            break
                        j += 1
                    if j >= n:
                        self.residual = bytearray(data[i:])
                        return
                    i = j + 2
                    continue
                i += 2
                continue
            if b == 0x0D:
                self.x = 0
                i += 1
                continue
            if b == 0x0A:
                self.y = min(self.y + 1, self.rows - 1)
                i += 1
                continue
            if b < 0x20 or b == 0x7F:
                i += 1
                continue
            # UTF-8 decode
            if b < 0x80:
                self.put(chr(b))
                i += 1
                continue
            ln = 2 if b >= 0xC0 and b < 0xE0 else 3 if b < 0xF0 else 4
            chunk = bytes(data[i : i + ln])
            if len(chunk) < ln:
                self.residual = bytearray(data[i:])
                return
            try:
                self.put(chunk.decode("utf-8"))
            except UnicodeDecodeError:
                pass
            i += ln
        return i

    def csi(self, params, final):
        if final in (0x48, 0x66):  # H / f cursor position
            parts = params.decode("ascii", "ignore").split(";") if params else []
            row = int(parts[0]) - 1 if parts and parts[0] else 0
            col = int(parts[1]) - 1 if len(parts) > 1 and parts[1] else 0
            self.move_to(row, col)
        elif final == 0x6D:  # SGR
            self.sgr(params.decode("ascii", "ignore"))
        elif final == 0x4A:  # ED (2J full clear etc.)
            p = params.decode("ascii", "ignore") or "0"
            if p in ("2", "3"):
                blank = (" ", self.fg, self.bg, False)
                self.grid = [[blank for _ in range(self.cols)] for _ in range(self.rows)]
        elif final == 0x4B:  # EL
            p = params.decode("ascii", "ignore") or "0"
            blank = (" ", self.fg, self.bg, False)
            if p == "0":
                for x in range(self.x, self.cols):
                    self.grid[self.y][x] = blank
        elif final == 0x43:  # CUF cursor forward
            d = int(params.decode("ascii", "ignore") or "1")
            self.x = min(self.x + d, self.cols - 1)
        elif final == 0x44:  # CUB
            d = int(params.decode("ascii", "ignore") or "1")
            self.x = max(self.x - d, 0)
        elif final == 0x41:  # CUU
            d = int(params.decode("ascii", "ignore") or "1")
            self.y = max(self.y - d, 0)
        elif final == 0x42:  # CUD
            d = int(params.decode("ascii", "ignore") or "1")
            self.y = min(self.y + d, self.rows - 1)
        elif final == 0x47:  # CHA column absolute
            c = int(params.decode("ascii", "ignore") or "1") - 1
            self.x = min(max(c, 0), self.cols - 1)
        elif final == 0x64:  # VPA row absolute
            r = int(params.decode("ascii", "ignore") or "1") - 1
            self.y = min(max(r, 0), self.rows - 1)

    def sgr(self, s):
        codes = [int(c) if c else 0 for c in s.split(";")] if s else [0]
        k = 0
        while k < len(codes):
            c = codes[k]
            if c == 0:
                self.fg = DEFAULT_FG
                self.bg = DEFAULT_FG
                self.bold = False
            elif c == 1:
                self.bold = True
            elif c == 22:
                self.bold = False
            elif 30 <= c <= 37 or 90 <= c <= 97:
                self.fg = (-2, c, -1)
            elif 40 <= c <= 47 or 100 <= c <= 107:
                self.bg = (-2, c, -1)
            elif c == 39:
                self.fg = DEFAULT_FG
            elif c == 49:
                self.bg = DEFAULT_FG
            elif c == 38 and k + 4 < len(codes) and codes[k + 1] == 2:
                self.fg = (codes[k + 2], codes[k + 3], codes[k + 4])
                k += 4
            elif c == 48 and k + 4 < len(codes) and codes[k + 1] == 2:
                self.bg = (codes[k + 2], codes[k + 3], codes[k + 4])
                k += 4
            elif c == 38 and k + 2 < len(codes) and codes[k + 1] == 5:
                self.fg = (-3, codes[k + 2], -1)
                k += 2
            elif c == 48 and k + 2 < len(codes) and codes[k + 1] == 5:
                self.bg = (-3, codes[k + 2], -1)
                k += 2
            k += 1

    def snapshot(self):
        return [[cell for cell in row] for row in self.grid]


def row_occupancy(row):
    # occupied = visible glyph or colored space (trail ghost)
    return [
        1 if (ch != " " or fg != DEFAULT_FG) else 0
        for (ch, fg, bg, bold) in row
    ]


def cell_kind(cell):
    ch, fg, bg, bold = cell
    if ch not in (" ", ""):
        return "G"  # glyph
    if fg != DEFAULT_FG:
        return "C"  # colored space (trail ghost)
    return "B"  # blank


def composition(snap):
    g = c = b = 0
    for row in snap:
        for cell in row:
            k = cell_kind(cell)
            if k == "G":
                g += 1
            elif k == "C":
                c += 1
            else:
                b += 1
    return g, c, b


def classify(prev, cur):
    """Classify per-cell transitions: G->B, G->C, C->B, B->G, B->C, G->G', C->C'."""
    counts = {}
    for y in range(len(cur)):
        prow, crow = prev[y], cur[y]
        for x in range(len(crow)):
            a, b = prow[x], crow[x]
            if a == b:
                continue
            ka, kb = cell_kind(a), cell_kind(b)
            key = f"{ka}>{kb}"
            counts[key] = counts.get(key, 0) + 1
    return counts


def dump_grid(snap, path):
    with open(path, "w") as f:
        for row in snap:
            f.write("".join(ch if ch not in (" ", "") else ("." if ch == " " else "") for (ch, fg, bg, bold) in row) + "\n")


def analyze(prev, cur, max_lag=12, min_match=0.7, min_occ=8):
    """Detect rows whose occupancy pattern translated horizontally.

    Only MASS events matter (the owner symptom is "all rain shifts"):
    sparse rows and single-row coincidences are rain motion, not a shift.
    """
    shifted = {}
    churn = 0
    for y in range(len(cur)):
        po = row_occupancy(prev[y])
        pc = row_occupancy(cur[y])
        no = sum(po)
        nc = sum(pc)
        diff = sum(1 for a, b in zip(po, pc) if a != b)
        churn += diff
        if no < min_occ or nc < min_occ or diff == 0:
            continue
        best_lag, best_ov = 0, 0.0
        base = min(no, nc)
        for lag in range(-max_lag, max_lag + 1):
            if lag == 0:
                continue
            ov = 0
            for x in range(len(pc)):
                xx = x - lag
                if 0 <= xx < len(po) and pc[x] and po[xx]:
                    ov += 1
            frac = ov / base
            if frac > best_ov:
                best_ov, best_lag = frac, lag
        if best_ov >= min_match:
            shifted.setdefault(best_lag, []).append(y)
    return shifted, churn


def main() -> int:
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
    pending = bytearray()
    in_frame = False
    frame_acc = bytearray()
    screen = Screen(TERM_COLS, TERM_ROWS)
    prev_snap = None
    rows_out = []
    frame_idx = 0
    next_key = 0
    dump_count = [0]
    hud_log = []  # (t, prs, dsty, ehs, fps, tgt)
    last_hud_t = 0.0
    final_snap = None

    def sample_hud(t):
        nonlocal last_hud_t
        if t - last_hud_t < 0.25:
            return
        last_hud_t = t
        text = "\n".join("".join(c[0] for c in row) for row in screen.grid)
        m_p = re.search(r"prs:\s*([0-9.]+)", text)
        m_d = re.search(r"dsty:\s*([0-9.]+)", text)
        m_e = re.search(r"ehs:\s*([0-9.]+)", text)
        m_f = re.search(r"fps:\s*([0-9.]+)", text)
        m_t = re.search(r"tgt:\s*([0-9.]+)", text)
        g, c, b = composition(screen.grid)
        hud_log.append(
            (
                t,
                m_p.group(1) if m_p else "-",
                m_d.group(1) if m_d else "-",
                m_e.group(1) if m_e else "-",
                m_f.group(1) if m_f else "-",
                m_t.group(1) if m_t else "-",
                g,
                c,
                b,
            )
        )

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
        r, _, _ = select.select([master_fd], [], [], 0.002)
        if not r:
            continue
        try:
            chunk = os.read(master_fd, 1 << 20)
        except (BlockingIOError, OSError):
            continue
        if not chunk:
            break
        # Pull the parser's residual back in front so marker searches see
        # the TRUE stream order (a BSU marker can be split across reads,
        # with its head parked in screen.residual after an incomplete CSI).
        if screen.residual:
            pending[:0] = screen.residual
            screen.residual = bytearray()
        pending.extend(chunk)

        while True:
            if in_frame:
                off = pending.find(BSU_OFF)
                if off == -1:
                    # mid-frame chunk: parser keeps partial-sequence tail in
                    # its own residual; the accounting still counts the bytes
                    screen.feed(pending)
                    frame_acc.extend(pending)
                    pending.clear()
                    break
                body = bytes(pending[:off])
                screen.feed(body)
                frame_acc.extend(body)
                pending = pending[off + len(BSU_OFF) :]
                t_end = time.monotonic() - start
                frame_idx += 1
                snap = screen.snapshot()
                if prev_snap is not None:
                    shifted, churn = analyze(prev_snap, snap)
                    # mass-shift event: >=5 rows translating at the same lag
                    mass = {lag: ys for lag, ys in shifted.items() if len(ys) >= 5}
                    if mass:
                        for lag, ys in sorted(mass.items()):
                            rows_out.append(
                                f"{t_end:.3f}\t{frame_idx}\t{lag}\t{len(ys)}\t{churn}\t{','.join(map(str, ys[:12]))}"
                            )
                    elif churn > 0:
                        rows_out.append(f"{t_end:.3f}\t{frame_idx}\t0\t0\t{churn}\t")
                    if churn > 500:
                        cc = classify(prev_snap, snap)
                        cc_s = " ".join(f"{k}={v}" for k, v in sorted(cc.items(), key=lambda kv: -kv[1]))
                        g, c, b = composition(snap)
                        print(
                            f"[hi-churn] t={t_end:7.2f}s f#{frame_idx:6d} churn={churn:5d} G={g} C={c} B={b}  {cc_s}",
                            flush=True,
                        )
                        if churn > 2000 and dump_count[0] < 6:
                            dump_count[0] += 1
                            base = f"/tmp/cosmostrix-nh2/dump_{dump_count[0]}_{t_end:.2f}"
                            dump_grid(prev_snap, base + "_prev.txt")
                            dump_grid(snap, base + "_cur.txt")
                prev_snap = snap
                frame_acc = bytearray()
                in_frame = False
                sample_hud(t_end)
                final_snap = snap
            else:
                on = pending.find(BSU_ON)
                if on == -1:
                    # inter-frame bytes (rare): keep the grid in sync
                    screen.feed(pending)
                    pending.clear()
                    break
                pre = bytes(pending[:on])
                if pre:
                    screen.feed(pre)
                pending = pending[on + len(BSU_ON) :]
                in_frame = True

    if final_snap is not None:
        dump_grid(final_snap, "/tmp/cosmostrix-nh2/final_grid.txt")

    try:
        proc.send_signal(signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        f.write("# t_end_sec\tframe_idx\tlag\trows_shifted\tchurn_cells\trows\n")
        f.write("\n".join(rows_out) + "\n")

    shifts = [r for r in rows_out if not r.endswith("\t0\t0\t") and not r.endswith("\t0\t")]
    nonzero = [
        [x for x in r.split("\t")]
        for r in rows_out
        if r.split("\t")[2] not in ("0",)
    ]
    print(
        f"frames={frame_idx}  span={time.monotonic() - start:.0f}s  "
        f"rows_with_lag_events={sum(1 for r in nonzero)}  tsv -> {OUT}"
    )
    if hud_log:
        print("HUD telemetry (t, prs, dsty, ehs, fps, tgt, G, C, B):")
        for t, prs, dsty, ehs, fps, tgt, g, c, b in hud_log:
            print(f"  t={t:7.2f}s prs={prs:>5} dsty={dsty:>5} ehs={ehs:>5} fps={fps:>6} tgt={tgt:>4} G={g:5d} C={c:5d} B={b:5d}")
    if nonzero:
        print("NON-ZERO-LAG ROW SHIFTS (t, frame, lag, nrows, churn):")
        shown = 0
        for t, fi, lag, nrows, churn, *_ in nonzero:
            print(f"  t={float(t):7.2f}s f#{int(fi):6d} lag={int(lag):+3d} rows={int(nrows):3d} churn={int(churn):5d}")
            shown += 1
            if shown > 80:
                print("  ... (truncated)")
                break
    else:
        print("no non-zero-lag row shifts detected")
    return 0


if __name__ == "__main__":
    sys.exit(main())
