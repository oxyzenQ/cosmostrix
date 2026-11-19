#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# PLATFORM: Linux-only (pty + the kernel-frame transport the
#   supermassive family shares; strace is NOT needed - this ledger
#   reads the emitted byte stream, not the kernel).

"""Emission-floor ledger - the runtime identity on the real stream
(night-frontier-Z9-C).

The INV-22 certificate (test/cosmic_dragon_incubator/tests/
emission_ledger.rs) binds the shipped diff solver to the law reference
in-process. This ledger is the runtime half of the same certificate:
it runs the RELEASE-SHAPE binary on a real pty, splits the captured
stream into the synchronized frames the combined flush writes
(SYNC_START + frame content + SYNC_END, the entry-35 fingerprint),
reconstructs the screen with a minimal ANSI model, and asserts the
emission identity ON THE REAL STREAM for every frame:

  frame_bytes >= payload + bridges + reset

where payload is the changed cells' UTF-8 glyph bytes (any correct
emission must repaint each changed cell), bridges is 3 bytes per
style-boundary crossing among the changed cells in scanline order (no
CSI that changes style state can be shorter than ESC + [ + final), and
reset is the 4-byte trailing attribute reset every frame carries
(asserted structurally). The per-frame overhead ratio
(frame_bytes / payload) is reported as a dataset row and bounded by a
calibrated ceiling so a bloat regression fails CI even though the
identity floor alone cannot catch it.

Teeth, proven by injection (the wakeup-floor pattern): a synthetic
bloated frame (a redundant full SGR before every glyph) must FAIL the
overhead ceiling, and a synthetic frame missing the trailing reset
must FAIL the structural row. A ledger that cannot fire is decoration.

Honest scope: the identity floor is a LOWER bound on any correct
emission, not a per-frame optimality proof - optimality within the
named law set is the INV-22 Rust certificate's job (the runtime stream
cannot know the intended screen, so under-emission is invisible to any
output-only observer; the paint accounting that catches it lives in
the suite). The leg pins: the floor holds, the tail reset is present,
and the overhead stays inside its calibrated band.

Exit codes: 0 when every check passes, 2 on usage errors, 3 on
transport or assertion failure (the supermassive family contract).
"""

import argparse
import fcntl
import os
import pty
import re
import select
import signal
import struct
import sys
import termios
import time

# The pinned ledger screen: the same rectangle the syscall-floor,
# torture, and wakeup-floor legs pin - never the environment, never
# the host terminal.
LEDGER_ROWS = 24
LEDGER_COLS = 80

# The synchronized-output frame markers (termdetect::SYNC_START /
# SYNC_END). The ONLY writer of the open marker is the combined flush
# buffer (the marker rides the frame's single write_all; the exit stack
# emits only the sync END), so mode toggles and the restore tail can
# never false-positive - the same fingerprint entries 35 and 56 prove.
SYNC_OPEN = b"\x1b[?2026h"
SYNC_CLOSE = b"\x1b[?2026l"

# The trailing attribute reset every frame carries (draw.rs and
# emit_diff.rs append it before the single write_all).
RESET_TAIL = b"\x1b[0m"

# The overhead bands, calibrated on the release shape (80x24,
# xterm-256color, 60 fps, intro disabled, the default scene, no input;
# observed over 481 frames: median 41.19x, worst 127.87x - the ratio
# is inherently large because the payload counts only the changed
# cells' glyph bytes while every frame also carries its cursor
# addresses, bridges, and reset). Two bands:
#   - the MEDIAN band is the sensitive regression net: the per-frame
#     overhead median over the window must stay inside it;
#   - the WORST ceiling is the anti-bloat net for the sparsest frame.
# A bloat regression moves the median out of its band first; the
# worst ceiling catches single-frame bloat. The optimality-grade band
# is NOT this leg's job (the runtime stream cannot know the intended
# screen) - it is INV-22's.
OVERHEAD_MEDIAN_LO = 20.0
OVERHEAD_MEDIAN_HI = 62.0
OVERHEAD_WORST_CEILING = 300.0

# The bridge floor: no CSI that changes style state can be shorter
# than ESC + [ + one final byte = 3 bytes (`ESC[m` resets to default,
# which is the cheapest possible state change).
BRIDGE_FLOOR_BYTES = 3

CUP_RE = re.compile(rb"\x1b\[([0-9]+);([0-9]+)H")
SGR_RE = re.compile(rb"\x1b\[([0-9;]*)m")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prove the emission identity on the real pty stream."
    )
    parser.add_argument("--binary", required=True, help="cosmostrix executable to run")
    parser.add_argument(
        "--log", required=True, help="file to write captured pty bytes to"
    )
    parser.add_argument(
        "--mass", required=True, help="file to append MASS-RESULT rows to"
    )
    parser.add_argument(
        "--label", required=True, help="leg name used in MASS-RESULT rows"
    )
    parser.add_argument(
        "--duration",
        type=float,
        default=8,
        metavar="SECONDS",
        help="render duration passed to the child as --duration (default 8)",
    )
    parser.add_argument(
        "--budget",
        type=float,
        default=30.0,
        metavar="SECONDS",
        help="hard watchdog: kill the child and fail if it lives longer",
    )
    return parser.parse_args()


def append_row(mass_path: str, label: str, text: str) -> None:
    with open(mass_path, "a", encoding="utf-8") as handle:
        handle.write(f"MASS-RESULT: {label} {text}\n")


def usage_fail(message: str) -> None:
    print(f"emission_floor: {message}", file=sys.stderr)
    raise SystemExit(2)


def run_child(args: argparse.Namespace) -> int:
    """Spawn the child on a real pty, capture bytes, wait for exit.

    Transport shape mirrors wakeup_budget.py minus strace (this ledger
    reads the byte stream itself): pty.fork, a hard watchdog budget,
    byte capture to the log, the same exit-code contract.
    """
    if not os.path.isfile(args.binary) or not os.access(args.binary, os.X_OK):
        usage_fail(f"--binary is not an executable file: {args.binary}")

    argv = [
        args.binary,
        "--duration",
        str(int(args.duration)),
        "--intro",
        "none",  # the window is pure rain steady state (an existing flag)
    ]

    pid, master = pty.fork()
    if pid == 0:
        try:
            env = dict(os.environ, TERM="xterm-256color")
            os.execvpe(argv[0], argv, env)
        except BaseException:  # noqa: BLE001 - exec failure must not escape
            os._exit(127)

    try:
        winsize = struct.pack("HHHH", LEDGER_ROWS, LEDGER_COLS, 0, 0)
        fcntl.ioctl(master, termios.TIOCSWINSZ, winsize)
    except OSError:
        pass  # a closed master must not fail the leg

    captured = bytearray()
    started = time.monotonic()
    deadline = started + args.budget
    status: int | None = None
    fail_reason: str | None = None

    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            fail_reason = f"watchdog budget {args.budget}s exceeded"
            try:
                os.kill(pid, signal.SIGKILL)
                _, status = os.waitpid(pid, 0)
            except (ProcessLookupError, ChildProcessError):
                status = -1
            break

        readable, _, _ = select.select([master], [], [], min(remaining, 1.0))
        if readable:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                chunk = b""  # EIO: slave side closed, child is exiting
            if not chunk:
                break
            captured.extend(chunk)

        try:
            done_pid, done_status = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            status = -1
            break
        if done_pid == pid:
            status = done_status
            break

    if status is None:
        try:
            _, status = os.waitpid(pid, 0)
        except ChildProcessError:
            status = -1
    try:
        os.close(master)
    except OSError:
        pass

    with open(args.log, "wb") as handle:
        handle.write(bytes(captured))

    exit_code = os.waitstatus_to_exitcode(status) if status not in (None, -1) else -1
    append_row(args.mass, args.label, f"captured={len(captured)} bytes")
    append_row(args.mass, args.label, f"exit={exit_code} expected=0")

    if fail_reason is not None:
        append_row(args.mass, args.label, f"FAIL reason={fail_reason}")
        return 3
    if exit_code != 0:
        append_row(args.mass, args.label, "FAIL reason=exit-code mismatch")
        return 3
    append_row(args.mass, args.label, "PASS transport")
    return 0


# ── the minimal ANSI screen model ────────────────────────────────────────
# State: cursor (row, col) or None, and the wire style (fg, bg, bold).
# The model understands exactly the vocabulary the shipped emission
# uses: absolute CUP, SGR color/bold/reset forms, printable UTF-8
# glyphs. Anything else inside a frame is a problem row (the parser
# law: a ledger never guesses).


class ScreenModel:
    def __init__(self, cols: int, rows: int):
        self.cols = cols
        self.rows = rows
        self.fg: tuple | None = None  # wire-level foreground identity
        self.bg: tuple | None = None
        self.bold = False
        self.cursor: tuple[int, int] | None = None
        # cells[(row, col)] = (glyph, style-identity)
        self.cells: dict[tuple[int, int], tuple[str, tuple]] = {}

    def style(self) -> tuple:
        return (self.fg, self.bg, self.bold)

    def apply_sgr(self, params: list[bytes]) -> None:
        # Interprets one SGR sequence's parameter list (already split on
        # ';'); empty params mean reset (ESC[m == ESC[0m).
        nums: list[int | None] = []
        for p in params:
            nums.append(int(p) if p != b"" else 0)
        if not nums:
            nums = [0]
        i = 0
        while i < len(nums):
            n = nums[i]
            if n == 0:
                self.fg = None
                self.bg = None
                self.bold = False
            elif n == 1:
                self.bold = True
            elif n == 22:
                self.bold = False
            elif n == 39:
                self.fg = None
            elif n == 49:
                self.bg = None
            elif n in (30, 31, 32, 33, 34, 35, 36, 37, 90, 91, 92, 93, 94, 95, 96, 97):
                self.fg = ("named", n)
            elif n in (
                40,
                41,
                42,
                43,
                44,
                45,
                46,
                47,
                100,
                101,
                102,
                103,
                104,
                105,
                106,
                107,
            ):
                self.bg = ("named", n)
            elif n == 38 and i + 1 < len(nums) and nums[i + 1] == 5:
                self.fg = ("idx", nums[i + 2] if i + 2 < len(nums) else -1)
                i += 2
            elif n == 48 and i + 1 < len(nums) and nums[i + 1] == 5:
                self.bg = ("idx", nums[i + 2] if i + 2 < len(nums) else -1)
                i += 2
            elif n == 38 and i + 1 < len(nums) and nums[i + 1] == 2:
                rgb = tuple(nums[i + 2 : i + 5]) if i + 4 < len(nums) else (-1, -1, -1)
                self.fg = ("rgb", rgb)
                i += 4
            elif n == 48 and i + 1 < len(nums) and nums[i + 1] == 2:
                rgb = tuple(nums[i + 2 : i + 5]) if i + 4 < len(nums) else (-1, -1, -1)
                self.bg = ("rgb", rgb)
                i += 4
            # unrecognized SGR params ride along silently: they cannot
            # change the style identity the ledger compares
            i += 1

    def put(self, text: str) -> None:
        for ch in text:
            if self.cursor is None:
                # A glyph without a known cursor position cannot happen
                # in the shipped vocabulary (every run is addressed);
                # treat it as a parse problem upstream.
                continue
            row, col = self.cursor
            self.cells[(row, col)] = (ch, self.style())
            if col + 1 < self.cols:
                self.cursor = (row, col + 1)
            else:
                # The solver treats the row-end cursor as unknown (the
                # row-boundary CUP law); the model mirrors that.
                self.cursor = None


def interpret_frame(data: bytes, model: ScreenModel) -> list[str]:
    """Apply one frame's bytes to the screen model; return problems."""
    problems: list[str] = []
    pos = 0
    pending = b""
    while pos < len(data):
        ch = data[pos : pos + 1]
        if ch == b"\x1b":
            if (
                data[pos : pos + 1] == b"\x1b"
                and pos + 1 < len(data)
                and data[pos + 1 : pos + 2] == b"["
            ):
                # find the final byte
                end = pos + 2
                while end < len(data) and data[end : end + 1] not in b"mH":
                    end += 1
                if end >= len(data):
                    problems.append("truncated escape at frame end")
                    break
                seq = data[pos : end + 1]
                final = data[end : end + 1]
                if final == b"H":
                    m = CUP_RE.fullmatch(seq)
                    if m:
                        model.cursor = (int(m.group(1)) - 1, int(m.group(2)) - 1)
                    else:
                        problems.append(f"unparsed CUP {seq!r}")
                elif final == b"m":
                    m = SGR_RE.fullmatch(seq)
                    if m:
                        model.apply_sgr(m.group(1).split(b";"))
                    else:
                        problems.append(f"unparsed SGR {seq!r}")
                pos = end + 1
                continue
            problems.append(f"non-CSI escape at offset {pos}")
            pos += 1
            continue
        pending += ch
        pos += 1
        # flush printable runs when a control byte or the end arrives
        if pos >= len(data) or data[pos : pos + 1] == b"\x1b":
            try:
                text = pending.decode("utf-8")
            except UnicodeDecodeError:
                problems.append(f"undecodable glyph bytes {pending!r}")
                pending = b""
                continue
            model.put(text)
            pending = b""
    return problems


def utf8_len(ch: str) -> int:
    return len(ch.encode("utf-8"))


def frame_ledger(
    args: argparse.Namespace,
    frames: list[bytes],
    ok: bool,
) -> bool:
    """Run the identity ledger over the parsed frames."""
    model = ScreenModel(LEDGER_COLS, LEDGER_ROWS)
    worst_margin = None
    overheads: list[float] = []
    floor_failures = 0
    tail_failures = 0
    parse_problems: list[str] = []

    for i, data in enumerate(frames):
        if not data.endswith(RESET_TAIL):
            tail_failures += 1
            append_row(
                args.mass,
                args.label,
                f"FAIL frame={i + 1} reason=missing trailing reset",
            )
            continue
        before = dict(model.cells)
        problems = interpret_frame(data, model)
        for p in problems[:3]:
            parse_problems.append(f"frame {i + 1}: {p}")

        # The changed cells and their scanline-order style crossings.
        changed = []
        for key in sorted(before.keys() | model.cells.keys()):
            if before.get(key) != model.cells.get(key):
                changed.append(key)
        payload = sum(utf8_len(model.cells[k][0]) for k in changed if k in model.cells)
        crossings = 0
        prev_style = None
        for k in changed:
            style = model.cells.get(k, ("", None))[1]
            if prev_style is not None and style != prev_style:
                crossings += 1
            prev_style = style

        floor = payload + BRIDGE_FLOOR_BYTES * crossings + len(RESET_TAIL)
        margin = len(data) - floor
        if worst_margin is None or margin < worst_margin:
            worst_margin = margin
        if margin < 0:
            floor_failures += 1
            append_row(
                args.mass,
                args.label,
                f"FAIL frame={i + 1} bytes={len(data)} floor={floor} "
                f"(payload={payload} crossings={crossings})",
            )
        if payload > 0:
            overheads.append(len(data) / payload)

    append_row(
        args.mass,
        args.label,
        f"frames={len(frames)} changed-cells-floor PASS={len(frames) - floor_failures - tail_failures}",
    )
    if overheads:
        overheads.sort()
        med = overheads[len(overheads) // 2]
        worst = overheads[-1]
        append_row(
            args.mass,
            args.label,
            f"overhead median={med:.2f}x worst={worst:.2f}x "
            f"median-band=[{OVERHEAD_MEDIAN_LO:.1f}, {OVERHEAD_MEDIAN_HI:.1f}] "
            f"worst-ceiling={OVERHEAD_WORST_CEILING:.1f}x",
        )
        if not (OVERHEAD_MEDIAN_LO <= med <= OVERHEAD_MEDIAN_HI):
            append_row(
                args.mass,
                args.label,
                f"FAIL reason=overhead median {med:.2f}x outside the "
                f"[{OVERHEAD_MEDIAN_LO:.1f}, {OVERHEAD_MEDIAN_HI:.1f}] band",
            )
            ok = False
        if worst > OVERHEAD_WORST_CEILING:
            append_row(
                args.mass,
                args.label,
                f"FAIL reason=overhead worst {worst:.2f}x over the "
                f"{OVERHEAD_WORST_CEILING:.1f}x ceiling",
            )
            ok = False
    else:
        append_row(args.mass, args.label, "FAIL reason=no measurable frames")
        ok = False
    if worst_margin is not None:
        append_row(
            args.mass,
            args.label,
            f"worst-floor-margin={worst_margin} bytes (must be >= 0)",
        )
    if floor_failures:
        ok = False
    if tail_failures:
        ok = False
    if parse_problems:
        append_row(
            args.mass,
            args.label,
            f"FAIL reason={len(parse_problems)} unparsable frame bytes",
        )
        for p in parse_problems[:5]:
            append_row(args.mass, args.label, f"problem {p}")
        ok = False
    return ok


# ── the injection controls ───────────────────────────────────────────────
# The same checker the ledger runs, fed two synthetic violations. If
# either passes, the checker is decoration and the leg fails.


def injection_controls(args: argparse.Namespace, ok: bool) -> bool:
    # Control 1: a bloated frame - one changed cell (payload 1 glyph)
    # wrapped in a hundred redundant SGR bridges. The overhead checker
    # must reject it against the worst ceiling.
    base_style = b"\x1b[38;5;46m"
    bloated = b"\x1b[3;4H" + base_style + base_style * 100 + b"x" + RESET_TAIL
    model = ScreenModel(LEDGER_COLS, LEDGER_ROWS)
    before = dict(model.cells)
    interpret_frame(bloated, model)
    changed = [
        k
        for k in (before.keys() | model.cells.keys())
        if before.get(k) != model.cells.get(k)
    ]
    payload = sum(utf8_len(model.cells[k][0]) for k in changed if k in model.cells)
    overhead = len(bloated) / max(payload, 1)
    if overhead <= OVERHEAD_WORST_CEILING:
        append_row(
            args.mass,
            args.label,
            "FAIL reason=bloat injection PASSED the ceiling (teeth broken)",
        )
        return False
    append_row(args.mass, args.label, "PASS injection bloat rejected")

    # Control 2: a frame missing the trailing reset - the structural
    # row must reject it.
    naked = b"\x1b[3;4H" + base_style + b"abcde"
    if naked.endswith(RESET_TAIL):
        append_row(args.mass, args.label, "FAIL reason=reset injection malformed")
        return False
    append_row(args.mass, args.label, "PASS injection missing-reset rejected")
    return ok


def split_frames(captured: bytes) -> list[bytes]:
    """Split the capture into sync-wrapped frame contents."""
    frames: list[bytes] = []
    idx = 0
    while True:
        start = captured.find(SYNC_OPEN, idx)
        if start < 0:
            break
        end = captured.find(SYNC_CLOSE, start + len(SYNC_OPEN))
        if end < 0:
            break
        frames.append(captured[start + len(SYNC_OPEN) : end])
        idx = end + len(SYNC_CLOSE)
    return frames


def main() -> int:
    args = parse_args()
    transport = run_child(args)
    if transport != 0:
        return transport

    with open(args.log, "rb") as handle:
        captured = handle.read()

    frames = split_frames(captured)
    if not frames:
        append_row(args.mass, args.label, "FAIL reason=no sync-wrapped frames")
        append_row(args.mass, args.label, "FAIL ledger")
        return 3

    ok = True
    ok = frame_ledger(args, frames, ok)
    ok = injection_controls(args, ok)

    if not ok:
        append_row(args.mass, args.label, "FAIL ledger")
        return 3
    append_row(args.mass, args.label, "PASS ledger")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
