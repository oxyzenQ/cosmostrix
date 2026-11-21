#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). Requires a POSIX pty; the
#   GitHub runner image provides /dev/ptmx natively.
"""PTY smoke driver for the supermassive runtime battery (night-improve-7).

Runs the cosmostrix binary attached to a REAL pseudo-terminal and
captures every byte the child writes to the tty. This is the transport
half of the pty-smoke leg: the unit suite asserts logic on synthetic
strings, while this driver exercises the surfaces only a live tty can
reach - raw mode entry, real input bytes ('q' quit key), real signal
delivery (SIGTERM graceful shutdown), and the restore sequences the
LTS-1-audited exit stack emits on its way out.

The assertions half lives in pty_assert.py, which reads the captured
byte log this driver writes. Splitting transport from assertions keeps
each script single-purpose and independently testable.

Sentinel contract (mirrors the zelynic supermassive relay shape): every
run appends MASS-RESULT rows to the --mass file, one row per checked
fact, each ending in PASS or FAIL. The workflow's Verdict step fails if
any MASS-RESULT row reads FAIL. The driver exits non-zero on its own
failure too, so the step fails even if the mass file is unreadable.

Exit codes: 0 = child exited as expected within budget; 2 = usage error;
3 = child exited wrong / was watchdog-killed / transport failure.
"""

from __future__ import annotations

import argparse
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time

QUIT_KEY = b"q"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run a binary under a real pty and capture its tty bytes."
    )
    parser.add_argument("--binary", required=True, help="executable to spawn")
    parser.add_argument(
        "--log", required=True, help="file to write captured tty bytes to"
    )
    parser.add_argument(
        "--mass", required=True, help="file to append MASS-RESULT rows to"
    )
    parser.add_argument(
        "--label", required=True, help="leg name used in MASS-RESULT rows"
    )
    parser.add_argument(
        "--arg",
        action="append",
        default=[],
        dest="child_args",
        help="child argument, repeatable (passed through verbatim)",
    )
    parser.add_argument(
        "--env",
        action="append",
        default=[],
        dest="env_set",
        metavar="KEY=VALUE",
        help="extra child env var, repeatable",
    )
    parser.add_argument(
        "--unset-env",
        action="append",
        default=[],
        dest="env_unset",
        metavar="KEY",
        help="env var removed from the child, repeatable",
    )
    parser.add_argument(
        "--quit-key",
        type=float,
        default=None,
        metavar="SECONDS",
        help="write the 'q' quit key into the pty after this many seconds",
    )
    parser.add_argument(
        "--inject",
        action="append",
        default=[],
        dest="inject",
        metavar="KEY@SECONDS",
        help="write KEY (its literal bytes) into the pty at SECONDS; "
        "repeatable (e.g. --inject i@2 toggles the HUD mid-run)",
    )
    parser.add_argument(
        "--sigterm",
        type=float,
        default=None,
        metavar="SECONDS",
        help="deliver SIGTERM to the child after this many seconds",
    )
    parser.add_argument(
        "--budget",
        type=float,
        default=30.0,
        metavar="SECONDS",
        help="hard watchdog: kill the child and fail if it lives longer",
    )
    parser.add_argument(
        "--expect-exit",
        type=int,
        default=0,
        metavar="CODE",
        help="child exit code the leg considers a pass (default 0)",
    )
    parser.add_argument(
        "--rss-log",
        default=None,
        metavar="PATH",
        help="sample the child's VmRSS into this file (soak legs)",
    )
    parser.add_argument(
        "--winsize",
        default=None,
        metavar="COLSxROWS",
        help="pin the pty window size (e.g. 80x24) right after fork; "
        "legs whose assertions depend on a real screen size must set "
        "this - an unset winsize leaves the pty at the host default "
        "(0x0 in CI), which the renderer clamps to a minimal screen",
    )
    parser.add_argument(
        "--rss-interval",
        type=float,
        default=10.0,
        metavar="SECONDS",
        help="seconds between RSS samples (default 10)",
    )
    return parser.parse_args()


def build_child_env(args: argparse.Namespace) -> dict[str, str]:
    env = dict(os.environ)
    for key in args.env_unset:
        env.pop(key, None)
    for entry in args.env_set:
        key, sep, value = entry.partition("=")
        if not sep:
            usage_fail(f"--env expects KEY=VALUE, got {entry!r}")
        env[key] = value
    return env


def usage_fail(message: str) -> None:
    print(f"pty_smoke: {message}", file=sys.stderr)
    raise SystemExit(2)


def append_row(mass_path: str, label: str, text: str) -> None:
    with open(mass_path, "a", encoding="utf-8") as handle:
        handle.write(f"MASS-RESULT: {label} {text}\n")


def read_rss_kb(pid: int) -> int | None:
    """Read the child's current VmRSS in kB from /proc (None if gone)."""
    try:
        with open(f"/proc/{pid}/status", encoding="ascii") as handle:
            for line in handle:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
    except (OSError, ValueError, IndexError):
        return None
    return None


def run_child(
    args: argparse.Namespace,
    inject_specs: list[tuple[float, bytes]],
    winsize: tuple[int, int] | None,
) -> int:
    pid, master = pty.fork()
    if pid == 0:
        try:
            env = build_child_env(args)
            os.execvpe(args.binary, [args.binary, *args.child_args], env)
        except BaseException:  # noqa: BLE001 - exec failure must not escape as a pty ghost
            os._exit(127)

    if winsize is not None:
        # The torture battery's TIOCSWINSZ pattern: a leg that asserts
        # content on a real screen must pin one, because a CI pty with
        # no controlling terminal defaults to 0x0 and the renderer's
        # clamp would make content assertions trivially true.
        try:
            winsize_struct = struct.pack("HHHH", winsize[1], winsize[0], 0, 0)
            fcntl.ioctl(master, termios.TIOCSWINSZ, winsize_struct)
        except OSError:
            pass  # a closed master must not fail the leg
    captured = bytearray()
    rss_samples: list[tuple[float, int]] = []
    started = time.monotonic()
    quit_at = started + args.quit_key if args.quit_key is not None else None
    term_at = started + args.sigterm if args.sigterm is not None else None
    # --inject timeline: (deadline, key bytes), sorted so the loop fires
    # them in the caller's given order as their moments arrive. Each
    # fired entry is dropped; the wake computation below mirrors
    # quit_at/term_at so a pending injection always wakes the loop.
    inject_pending = [[started + delay, key] for delay, key in sorted(inject_specs)]
    rss_at = started + args.rss_interval if args.rss_log else None
    deadline = started + args.budget
    status: int | None = None
    fail_reason: str | None = None

    while True:
        now = time.monotonic()
        if now >= deadline:
            fail_reason = f"watchdog budget {args.budget}s exceeded"
            os.kill(pid, signal.SIGKILL)
            try:
                _, status = os.waitpid(pid, 0)
            except ChildProcessError:
                status = -1
            break

        inject_at = min((e[0] for e in inject_pending), default=None)
        wake = min(
            x for x in (deadline, quit_at, term_at, rss_at, inject_at) if x is not None
        )
        readable, _, _ = select.select(
            [master], [], [], max(0.0, wake - time.monotonic())
        )
        if readable:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                chunk = b""  # EIO: slave side closed, child is exiting
            if not chunk:
                break
            captured.extend(chunk)

        now = time.monotonic()
        if quit_at is not None and now >= quit_at:
            try:
                os.write(master, QUIT_KEY)
            except OSError:
                pass  # child already gone; the read loop will notice
            quit_at = None
        if term_at is not None and now >= term_at:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            term_at = None
        while inject_pending and now >= inject_pending[0][0]:
            _, key_bytes = inject_pending.pop(0)
            try:
                os.write(master, key_bytes)
            except OSError:
                pass  # child already gone; the read loop will notice
        if rss_at is not None and now >= rss_at:
            rss_kb = read_rss_kb(pid)
            if rss_kb is not None:
                rss_samples.append((now - started, rss_kb))
            rss_at = now + args.rss_interval

    if status is None:
        _, status = os.waitpid(pid, 0)
    try:
        os.close(master)
    except OSError:
        pass

    with open(args.log, "wb") as handle:
        handle.write(bytes(captured))

    if args.rss_log:
        with open(args.rss_log, "w", encoding="utf-8") as handle:
            for sample_time, rss_kb in rss_samples:
                handle.write(f"t={sample_time:.1f}s rss_kb={rss_kb}\n")
        if rss_samples:
            peak = max(rss for _, rss in rss_samples)
            append_row(
                args.mass, args.label, f"rss_peak_kb={peak} samples={len(rss_samples)}"
            )

    elapsed = time.monotonic() - started
    exit_code = os.waitstatus_to_exitcode(status)
    append_row(
        args.mass, args.label, f"captured={len(captured)} bytes in {elapsed:.1f}s"
    )
    append_row(args.mass, args.label, f"exit={exit_code} expected={args.expect_exit}")

    if fail_reason is not None:
        append_row(args.mass, args.label, f"FAIL reason={fail_reason}")
        return 3
    if exit_code != args.expect_exit:
        append_row(args.mass, args.label, "FAIL reason=exit-code mismatch")
        return 3
    append_row(args.mass, args.label, "PASS")
    return 0


def main() -> int:
    args = parse_args()
    if not os.path.isfile(args.binary) or not os.access(args.binary, os.X_OK):
        usage_fail(f"--binary is not an executable file: {args.binary}")
    if args.quit_key is not None and args.sigterm is not None:
        usage_fail("--quit-key and --sigterm are mutually exclusive in one run")
    winsize = None
    if args.winsize is not None:
        cols, sep, rows = args.winsize.partition("x")
        if not sep or not cols.isdigit() or not rows.isdigit():
            usage_fail(f"--winsize expects COLSxROWS, got {args.winsize!r}")
        winsize = (int(cols), int(rows))
    specs = []
    for spec in args.inject:
        key, sep, at = spec.partition("@")
        if not sep or not key or not at.replace(".", "", 1).isdigit():
            usage_fail(f"--inject expects KEY@SECONDS, got {spec!r}")
        specs.append((float(at), key.encode()))
    return run_child(args, inject_specs=specs, winsize=winsize)


if __name__ == "__main__":
    raise SystemExit(main())
