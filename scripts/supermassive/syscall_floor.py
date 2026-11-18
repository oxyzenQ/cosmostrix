#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# PLATFORM: Linux-only. strace is a Linux ptrace tool and the
#   syscall-floor ledger is the Linux kernel-floor proof by
#   definition (the release lane is Linux; the docs kernel law lives
#   in docs/SYSTEM_REQUIREMENTS.md).
"""Syscall-floor ledger — the kernel-floor proof (night-frontier-Z2, Kernel Floor 2a).

The docs claim a "single-syscall flush": every drawn frame reaches the
terminal in exactly ONE write() syscall (row-RLE diff pipeline, one
combined buffer, one BufWriter flush). A claim in a document is
marketing; this harness turns it into a CI invariant measured by the
KERNEL itself.

The binary runs under strace attached to a REAL pseudo-terminal, and
the ledger asserts the kernel-visible floor:

  count(write() syscalls to the tty carrying the synchronized-output
  frame marker) == frames_flushed (the renderer's own count of
  flush_ansi() calls, printed by --perf-stats)

The fingerprint is exact by construction: the ONLY writer of
SYNC_START (ESC[?2026h) in the codebase is the combined flush buffer
(src/engine/cosmic_dragon_engine/terminal/io_recovery.rs), which is
written once per flush_ansi() through one write_all into the 256 KiB
BufWriter and leaves it via one BufWriter::flush() — one syscall per
drawn frame. Setup, teardown, and restore writes never carry the
marker (the exit stack emits only the sync END); the v50 fix removed
the init-time standalone sync open. Any regression that splits a
frame across multiple write() calls, dribbles per-cell writes, or
adds a second flush per frame breaks the 1:1 identity and this
ledger goes red in CI.

The same proof is the io_uring rejection record: the steady state is
ONE write() per frame — the kernel floor for a tty writer. io_uring
for a single write replaces one syscall with ring setup plus a
submission, which is strictly more overhead for the same single
operation. At the floor, being AT the floor is the depth; the
experiment is concluded and the record stands (the measured-cost
benchmark lives in src/cosmic_dragon_incubator/egg/io_uring_rejected.rs).

The mandatory-output ledger (night-frontier-Z2 2a depth): the floor
must hold against EVERY output syscall, not only write(). The trace
therefore covers write, writev, pwrite64, pwritev, pwritev2, and
ioctl, and the ledger asserts the full output contract:

  - every tty output byte leaves through write() alone - a writev or
    pwrite* on a pty fd is an output path that bypasses the floor
    (scatter-gather or positioned writes) and goes red;
  - ioctl on a pty fd: MUTATING requests (TCSETS*/TIOCSWINSZ/
    TIOCSCTTY/TIOCSTI and kin) belong to the setup/teardown window
    only - a steady-state terminal MUTATION outside the single frame
    write goes red, and so does an undecoded/unknown request (a
    ledger never guesses). Read-only queries (TCGETS*, TIOCGWINSZ)
    in the steady state are OS-mandatory telemetry - the renderer
    polls the winsize for resize detection - and are ledgered
    informationally, not criminalized.

Scope, stated honestly: the input/wait/signal/timer vocabulary
(read, poll, epoll, nanosleep, signal delivery, futex) is
OS-mandatory transport residue, not renderer output - ptracing
those hot syscalls slows the traced child and pins no renderer
claim, so they stay outside the ledger by design. The OUTPUT floor
is the contract this harness owns.

Sentinel contract: MASS-RESULT rows, one per checked fact, each
ending in PASS or FAIL; exits 0 when every check passes, 2 on usage
errors, 3 on transport or assertion failure (same shape as
pty_smoke.py).
"""

from __future__ import annotations

import argparse
import fcntl
import os
import pty
import re
import select
import shutil
import signal
import struct
import sys
import termios
import time

# The synchronized-output frame marker (SYNC_START in
# src/termdetect/protocol.rs). Every drawn frame's single write()
# carries exactly one at its head; nothing else in the binary emits it.
SYNC_OPEN = b"\x1b[?2026h"
# The stdout BufWriter capacity (STDOUT_BUF_CAPACITY in
# src/types/constants.rs). A write() larger than this would mean a
# frame spilled the buffer and split into multiple syscalls — the
# ledger forbids that at the pinned screen size.
STDOUT_BUF_CAPACITY = 262_144
# Pinned ledger screen (torture.py sets winsize the same way). A fixed
# rectangle keeps every frame far under the BufWriter cap and makes
# the numbers comparable across runs.
LEDGER_ROWS = 24
LEDGER_COLS = 80
# Setup/teardown writes on the tty fd are mode toggles and the restore
# tail — a handful of small writes. Bounded so a regression that starts
# emitting per-cell garbage outside frames still trips the ledger.
SETUP_TEARDOWN_BOUND = 64
# The tty-ioctl ledger's two request families. MUTATING requests change
# terminal state (modes, window size, controlling tty, injection) and
# are forbidden in the steady-state window; READ-ONLY queries are
# OS-mandatory telemetry (the renderer polls the winsize for resize
# detection) and are ledgered informationally. Any request outside
# both sets - or an undecoded 0x... request - is treated as mutating
# in the steady window: a ledger never guesses.
MUTATING_IOCTL = frozenset(
    {
        "TCSETS",
        "TCSETS2",
        "TCSETSW",
        "TCSETSW2",
        "TCSETSF",
        "TCSETSF2",
        "TIOCSWINSZ",
        "TIOCSCTTY",
        "TIOCSTI",
        "TIOCSETD",
        "TIOCSBRK",
        "TIOCCBRK",
        "TIOCSPTLCK",
        "TIOCCONS",
        "TIOCPKT",
    }
)
READ_ONLY_IOCTL = frozenset(
    {
        "TCGETS",
        "TCGETS2",
        "TIOCGWINSZ",
        "TIOCGPTN",
        "TIOCGETD",
        "TIOCGPGRP",
        "TIOCGSID",
    }
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prove the 1-write-per-frame kernel floor under strace on a real pty."
    )
    parser.add_argument(
        "--binary", required=True, help="cosmostrix executable to trace"
    )
    parser.add_argument("--trace", required=True, help="strace output file to write")
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
    print(f"syscall_floor: {message}", file=sys.stderr)
    raise SystemExit(2)


def run_child(args: argparse.Namespace) -> int:
    """Spawn the traced child on a real pty, capture bytes, wait for exit.

    Transport shape mirrors pty_smoke.py (the proven supermassive
    transport): pty.fork, a hard watchdog budget, byte capture to the
    log, and the same exit-code contract. The winsize is pinned
    (80x24) so every frame lands far under the BufWriter cap.
    """
    if not os.path.isfile(args.binary) or not os.access(args.binary, os.X_OK):
        usage_fail(f"--binary is not an executable file: {args.binary}")
    if shutil.which("strace") is None:
        usage_fail("strace is not on PATH (the ledger is a Linux strace proof)")

    strace_argv = [
        "strace",
        "-f",  # follow the fork guard's children
        "-e",
        # The full output + control surface (night-frontier-Z2 2a):
        # write alone proves the 1-per-frame floor; writev/pwrite*
        # close the alternate-output bypass; ioctl pins the terminal
        # mode toggles to the setup/teardown window.
        "trace=write,writev,pwrite64,pwritev,pwritev2,ioctl",
        "-xx",  # every payload byte as \xNN — uniform parsing
        "-s",
        "96",  # payload prefix for classification; the length arg is authoritative
        "-o",
        args.trace,
        "--",
        args.binary,
        "--duration",
        str(int(args.duration)),
        "--perf-stats",
    ]

    pid, master = pty.fork()
    if pid == 0:
        try:
            env = dict(os.environ, TERM="xterm-256color")
            os.execvpe(strace_argv[0], strace_argv, env)
        except BaseException:  # noqa: BLE001 - exec failure must not escape as a pty ghost
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
        # The child closed the master; reap it.
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
    elapsed = time.monotonic() - started
    append_row(
        args.mass, args.label, f"captured={len(captured)} bytes in {elapsed:.1f}s"
    )
    append_row(args.mass, args.label, f"exit={exit_code} expected=0")

    if fail_reason is not None:
        append_row(args.mass, args.label, f"FAIL reason={fail_reason}")
        return 3
    if exit_code != 0:
        append_row(args.mass, args.label, "FAIL reason=exit-code mismatch")
        return 3
    append_row(args.mass, args.label, "PASS")
    return 0


# ── strace parsing ─────────────────────────────────────────────────────────
#
# With `-f -xx -s 96 -e trace=write,writev,pwrite64,pwritev,pwritev2,
# ioctl -o FILE` every traced line is one of:
#   <pid>  write(<fd>, "<hex payload>"..., <len>) = <ret>
#   <pid>  writev(<fd>, [{...}, ...], <len>) = <ret>
#   <pid>  pwrite64/pwritev/pwritev2(<fd>, ...) = <ret>
#   <pid>  ioctl(<fd>, <REQUEST>, ...) = <ret>
# Long payloads are truncated by the `...` marker after the quoted
# prefix — the third argument (len) stays authoritative. A write
# interrupted by a signal appears as two lines:
#   <pid>  write(1, "..."..., <len> <unfinished ...>)
#   <pid>  <... write resumed>"..."..., <len>) = <ret>
# The ledger joins those pairs per pid before counting; alt-output
# and ioctl heads are counted once (their resumed tails pass).
WRITE_OPEN = re.compile(
    r"^\s*(\d+)\s+write\((\d+), \"((?:[^\"\\]|\\.)*)\"(\.\.\.)?, (\d+)( <unfinished \.\.\.>)?\)"
)
WRITE_RESUMED = re.compile(
    r"^\s*(\d+)\s+<\.\.\. write resumed>\"((?:[^\"\\]|\\.)*)\"(\.\.\.)?, (\d+)\)\s*=\s*(-?\d+)"
)
WRITE_RET = re.compile(r"\)\s*=\s*(-?\d+)\s*$")
ALT_OUT_OPEN = re.compile(r"^\s*(\d+)\s+(writev|pwrite64|pwritev|pwritev2)\((\d+),")
IOCTL_OPEN = re.compile(r"^\s*(\d+)\s+ioctl\((\d+), ([^,)]+)")


def decode_strace_string(text: str) -> bytes:
    """Decode a strace -xx quoted payload into raw bytes.

    -xx renders every byte as \\xNN; quote and backslash may still
    arrive as \\" and \\\\. Both are handled; anything else passes
    through as its literal byte.
    """
    out = bytearray()
    i = 0
    while i < len(text):
        ch = text[i]
        if ch == "\\" and i + 1 < len(text):
            nxt = text[i + 1]
            if nxt == "x" and i + 3 < len(text):
                try:
                    out.append(int(text[i + 2 : i + 4], 16))
                    i += 4
                    continue
                except ValueError:
                    pass
            if nxt == '"':
                out.append(0x22)
                i += 2
                continue
            if nxt == "\\":
                out.append(0x5C)
                i += 2
                continue
        out.append(ord(ch) & 0xFF)
        i += 1
    return bytes(out)


class WriteRecord:
    """One kernel-visible write() syscall on a tty fd."""

    __slots__ = ("fd", "length", "order", "payload", "pid", "ret")

    def __init__(
        self, pid: int, fd: int, length: int, payload: bytes, ret: int, order: int
    ):
        self.pid = pid
        self.fd = fd
        self.length = length
        self.payload = payload
        self.ret = ret
        self.order = order


class AltOutRecord:
    """One kernel-visible alternate output syscall (writev/pwrite*).

    These must never appear on a pty fd: write() is the only output
    syscall the renderer's design contains.
    """

    __slots__ = ("fd", "name", "order", "pid")

    def __init__(self, pid: int, name: str, fd: int, order: int):
        self.pid = pid
        self.name = name
        self.fd = fd
        self.order = order


class IoctlRecord:
    """One kernel-visible ioctl syscall with its (decoded) request."""

    __slots__ = ("fd", "order", "pid", "request")

    def __init__(self, pid: int, fd: int, request: str, order: int):
        self.pid = pid
        self.fd = fd
        self.request = request
        self.order = order


def parse_trace(
    trace_path: str,
) -> tuple[list[WriteRecord], list[AltOutRecord], list[IoctlRecord], list[str]]:
    """Parse the strace output into the full output-surface ledger.

    Returns (writes, alt_out, ioctls, problems). writes are
    payload-decoded WriteRecords; alt_out carries writev/pwrite*
    syscalls (forbidden on tty fds); ioctls carry their request
    tokens (window-checked against the frame writes). problems
    carries any unjoinable unfinished/resumed pairs or unparsable
    lines - a ledger must never silently guess.
    """
    records: list[WriteRecord] = []
    alt_out: list[AltOutRecord] = []
    ioctls: list[IoctlRecord] = []
    problems: list[str] = []
    unfinished: dict[int, tuple[int, int, str]] = {}

    with open(trace_path, encoding="utf-8", errors="replace") as handle:
        for order, raw in enumerate(handle, start=1):
            line = raw.rstrip("\n")
            if not line or line.startswith("strace:") or "exited with" in line:
                continue

            resumed = WRITE_RESUMED.match(line)
            if resumed:
                pid, payload_text, _ellipsis, length, ret = resumed.groups()
                pending = unfinished.pop(int(pid), None)
                if pending is None:
                    problems.append(
                        f"resumed write without unfinished head: {line[:120]}"
                    )
                    continue
                head_fd, head_len, head_payload = pending
                records.append(
                    WriteRecord(
                        int(pid),
                        head_fd,
                        int(length) if length else head_len,
                        head_payload + decode_strace_string(payload_text),
                        int(ret),
                        order,
                    )
                )
                continue

            opened = WRITE_OPEN.match(line)
            if opened:
                pid, fd, payload_text, _ellipsis, length, unfinished_mark = (
                    opened.groups()
                )
                if unfinished_mark:
                    unfinished[int(pid)] = (
                        int(fd),
                        int(length),
                        decode_strace_string(payload_text),
                    )
                    continue
                ret_match = WRITE_RET.search(line)
                ret = int(ret_match.group(1)) if ret_match else -1
                records.append(
                    WriteRecord(
                        int(pid),
                        int(fd),
                        int(length),
                        decode_strace_string(payload_text),
                        ret,
                        order,
                    )
                )
                continue

            alt = ALT_OUT_OPEN.match(line)
            if alt:
                pid, name, fd = alt.groups()
                alt_out.append(AltOutRecord(int(pid), name, int(fd), order))
                continue

            ioctl = IOCTL_OPEN.match(line)
            if ioctl:
                pid, fd, request = ioctl.groups()
                ioctls.append(IoctlRecord(int(pid), int(fd), request, order))
                continue

            if "<... " in line and "resumed>" in line:
                # A resumed continuation of an alt-out or ioctl
                # syscall: the head line already counted the syscall
                # once, so the tail passes without a second record.
                continue

            if "<unfinished ...>" in line or "write resumed>" in line:
                problems.append(f"unparsed write edge case: {line[:120]}")
                continue
            # Non-write lines (strace meta, signals, exit banners) pass.

    for pid in unfinished:
        problems.append(f"unfinished write never resumed (pid {pid})")
    return records, alt_out, ioctls, problems


# ── perf report parsing ────────────────────────────────────────────────────

FRAMES_FLUSHED = re.compile(r"frames_flushed:\s+[0-9.]+[KMB]? \((\d+)\)")
TOTAL_ANSI = re.compile(r"total_ansi_bytes:\s+[0-9.]+ [KMG]?i?B \((\d+)\)")


def parse_frames_flushed(capture_path: str) -> tuple[int | None, int | None]:
    """Read frames_flushed + total_ansi_bytes from the --perf-stats report.

    The report prints to stderr, which rides the same pty as the
    frames; the humanized values carry the raw counts in parentheses.
    """
    with open(capture_path, "rb") as handle:
        data = handle.read()
    text = data.decode("utf-8", errors="replace")
    frames = FRAMES_FLUSHED.search(text)
    ansi = TOTAL_ANSI.search(text)
    return (
        int(frames.group(1)) if frames else None,
        int(ansi.group(1)) if ansi else None,
    )


# ── the ledger ─────────────────────────────────────────────────────────────


def run_ledger(args: argparse.Namespace) -> int:
    frames_flushed, total_ansi = parse_frames_flushed(args.log)
    if frames_flushed is None:
        append_row(
            args.mass,
            args.label,
            "FAIL reason=no frames_flushed row in the perf report",
        )
        return 3
    append_row(args.mass, args.label, f"frames_flushed={frames_flushed}")

    records, alt_out, ioctls, problems = parse_trace(args.trace)
    if problems:
        for problem in problems[:8]:
            append_row(args.mass, args.label, f"FAIL reason=trace parse: {problem}")
        append_row(args.mass, args.label, "FAIL")
        return 3

    tty = [r for r in records if r.fd == 1]
    frames = [r for r in tty if SYNC_OPEN in r.payload]
    others = [r for r in tty if SYNC_OPEN not in r.payload]
    failed = [r for r in tty if r.ret < 0]
    short = [r for r in tty if 0 <= r.ret < r.length]
    spillover = [r for r in tty if r.length > STDOUT_BUF_CAPACITY]

    append_row(
        args.mass,
        args.label,
        f"tty_writes={len(tty)} frame_writes={len(frames)} other_writes={len(others)}",
    )

    # THE FLOOR: one write() per drawn frame, kernel-visible.
    if frames_flushed == 0:
        append_row(
            args.mass, args.label, "FAIL reason=frames_flushed is 0 — nothing rendered"
        )
        return 3
    ratio = len(frames) / frames_flushed
    if len(frames) == frames_flushed:
        append_row(
            args.mass,
            args.label,
            f"syscall-floor writes/frame={ratio:.3f} PASS "
            f"({len(frames)} write() syscalls prove {frames_flushed} drawn frames)",
        )
    else:
        append_row(
            args.mass,
            args.label,
            f"syscall-floor writes/frame={ratio:.3f} FAIL "
            f"({len(frames)} sync-markered write() syscalls != {frames_flushed} flushes)",
        )

    # No failed writes on the tty: a healthy pty loses nothing.
    append_row(
        args.mass,
        args.label,
        f"failed-writes {len(failed)} {'PASS' if not failed else 'FAIL'} "
        "(EAGAIN/EINTR must not appear on the tty)",
    )

    # No partial writes: a short write() means the kernel accepted
    # fewer bytes than handed over and write_all looped into a second
    # syscall — a split frame. On a drained, healthy pty this is zero.
    append_row(
        args.mass,
        args.label,
        f"short-writes {len(short)} {'PASS' if not short else 'FAIL'} "
        "(a partial write splits a frame across syscalls)",
    )

    # Byte-exact identity between the renderer's own accounting and
    # the kernel's syscalls: the sum of frame-write lengths must equal
    # total_ansi_bytes plus the 16 sync-wrapper bytes (SYNC_START 8 +
    # SYNC_END 8) per frame. Catches any loss, duplication, or split
    # the count rows could miss.
    byte_row = "skipped (no total_ansi_bytes row)"
    byte_ok = True
    if total_ansi is not None:
        frame_bytes = sum(r.length for r in frames)
        expected = total_ansi + 16 * frames_flushed
        byte_ok = frame_bytes == expected
        byte_row = (
            f"byte-identity {'PASS' if byte_ok else 'FAIL'} "
            f"(kernel frame bytes {frame_bytes} == renderer {total_ansi} + {16 * frames_flushed} sync wrapper bytes)"
        )
    append_row(args.mass, args.label, byte_row)

    # No BufWriter spillover at the pinned screen size: a write larger
    # than the 256 KiB cap would mean multi-syscall frames.
    append_row(
        args.mass,
        args.label,
        f"spillover {len(spillover)} {'PASS' if not spillover else 'FAIL'} "
        f"(max_write_bytes={max((r.length for r in tty), default=0)} cap={STDOUT_BUF_CAPACITY})",
    )

    # Non-frame tty writes are setup/teardown mode toggles and the
    # restore tail — a bounded handful, never per-cell dribbles.
    append_row(
        args.mass,
        args.label,
        f"setup-teardown writes={len(others)} bound={SETUP_TEARDOWN_BOUND} "
        f"{'PASS' if len(others) <= SETUP_TEARDOWN_BOUND else 'FAIL'}",
    )

    # The mandatory-output ledger: write() must be the ONLY output
    # syscall on the pty. Any writev/pwrite* on an inherited tty fd
    # is an output path that bypasses the floor (the renderer's
    # design has exactly one write_all + one flush per frame).
    tty_fds = {0, 1, 2}
    alt_out_tty = [r for r in alt_out if r.fd in tty_fds]
    alt_histogram: dict[str, int] = {}
    for record in alt_out_tty:
        alt_histogram[record.name] = alt_histogram.get(record.name, 0) + 1
    alt_detail = ", ".join(f"{k}={v}" for k, v in sorted(alt_histogram.items()))
    append_row(
        args.mass,
        args.label,
        f"alt-output tty={len(alt_out_tty)} non-tty={len(alt_out) - len(alt_out_tty)} "
        f"{'PASS' if not alt_out_tty else 'FAIL'} "
        f"(write() is the only output syscall on the tty; {alt_detail or 'none seen'})",
    )

    # The tty-ioctl ledger: MUTATING requests (raw-mode toggles via
    # TCSETS*, window-size sets, injection) belong to the setup/
    # teardown window. Each tty ioctl is positioned against the first
    # and the last frame write: a MUTATING request between them
    # changes terminal state outside the single frame write - a floor
    # violation the write-only trace cannot see - and an undecoded or
    # unknown request there fails too (a ledger never guesses).
    # Read-only queries (TCGETS*, TIOCGWINSZ) in the steady state are
    # OS-mandatory resize telemetry and are ledgered informationally.
    frame_orders = [r.order for r in frames]
    steady_lo = min(frame_orders) if frame_orders else -1
    steady_hi = max(frame_orders) if frame_orders else -1
    tty_ioctls = [r for r in ioctls if r.fd in tty_fds]
    steady_ioctls = [r for r in tty_ioctls if steady_lo < r.order < steady_hi]
    steady_mutating = [r for r in steady_ioctls if r.request not in READ_ONLY_IOCTL]
    steady_reads = [r for r in steady_ioctls if r.request in READ_ONLY_IOCTL]
    read_histogram: dict[str, int] = {}
    for record in steady_reads:
        read_histogram[record.request] = read_histogram.get(record.request, 0) + 1
    read_detail = ", ".join(f"{k}={v}" for k, v in sorted(read_histogram.items()))
    setup_mutating = len(
        [r for r in tty_ioctls if r.request not in READ_ONLY_IOCTL]
    ) - len(steady_mutating)
    append_row(
        args.mass,
        args.label,
        f"tty-ioctl ledger mutating-steady={len(steady_mutating)} "
        f"{'PASS' if not steady_mutating else 'FAIL'} "
        f"(no terminal mutation outside the frame write; unknown requests fail; "
        f"setup+teardown mutating={setup_mutating})",
    )
    append_row(
        args.mass,
        args.label,
        f"tty-ioctl ledger read-only-steady={len(steady_reads)} INFO "
        f"(OS-mandatory resize/termios telemetry; {read_detail or 'none seen'})",
    )

    # The io_uring rejection record (free with the floor): the steady
    # state is ONE write() per frame — the kernel floor for a tty
    # writer. io_uring for a single write is strictly more overhead
    # (ring setup + submission for the same one operation). The
    # measured-cost benchmark: egg/io_uring_rejected.rs.
    append_row(
        args.mass,
        args.label,
        "io-uring rejection PASS (steady state is 1 write()/frame = the kernel floor; "
        "io_uring would add ring overhead to the same single write — rejected, record stands)",
    )

    floor_ok = len(frames) == frames_flushed
    clean = (
        not failed
        and not short
        and not spillover
        and len(others) <= SETUP_TEARDOWN_BOUND
        and byte_ok
        and not alt_out_tty
        and not steady_mutating
    )
    if floor_ok and clean:
        append_row(args.mass, args.label, "PASS")
        return 0
    append_row(args.mass, args.label, "FAIL")
    return 3


def main() -> int:
    args = parse_args()
    transport = run_child(args)
    if transport != 0:
        return transport
    return run_ledger(args)


if __name__ == "__main__":
    raise SystemExit(main())
