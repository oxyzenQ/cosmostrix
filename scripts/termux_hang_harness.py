#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-termux-hang reproduction harness (owner report 2026-09-12).

Reproduces the Termux screen-lock hang on any Unix with a PTY: the
child (cosmostrix) renders into a PTY whose master is NEVER drained.
When Android locks the screen, Termux stops reading its PTY master;
the slave buffer (~64 KB) fills and every further write() blocks.

Pre-fix signature: the render loop wedges in its frame flush, the
watchdog wedges on its own restore write into the same full PTY, and
the SIGTERM thread's grace window ends without anyone exiting the
process. The owner saw: screen frozen, no shortcut answered,
`pkill -f cosmostrix` a no-op, only `kill -9` worked.

Post-fix contract (what this harness asserts):
  Phase 1 (jam, no signal): the watchdog's nonblocking force-exit
    ends the process within JAM_EXIT_TIMEOUT (default 25 s). The
    intro also bumps FRAME_COUNTER, so the watchdog's stuck arm is
    armed during the intro as well.
  Phase 2 (jam + SIGTERM): the signal thread's 3 s grace window ends
    with a nonblocking force-exit (exit code 143, or the watchdog's 1
    if it wins the race) within TERM_EXIT_TIMEOUT (default 8 s) —
    the `pkill` contract.

Usage (repo root, binary built):
  python3 scripts/termux_hang_harness.py
  BIN=target/release/cosmostrix python3 scripts/termux_hang_harness.py

Knobs (env):
  BIN                binary path (default target/debug/cosmostrix)
  SIZE               "cols x rows" (default 120x40)
  JAM_EXIT_TIMEOUT   phase 1 budget, seconds (default 25)
  TERM_EXIT_TIMEOUT  phase 2 budget after SIGTERM, seconds (default 8)
  JAM_SETTLE         seconds to jam before sending SIGTERM (default 3)

Output: one line per phase (PASS/FAIL + time-to-exit + exit status),
final verdict line, exit code 0 on full pass, 1 otherwise.
"""

import fcntl
import os
import pty
import signal
import struct
import sys
import termios
import time

BIN = os.environ.get("BIN", "target/debug/cosmostrix")
COLS, ROWS = (int(v) for v in os.environ.get("SIZE", "120x40").split("x"))
JAM_EXIT_TIMEOUT = float(os.environ.get("JAM_EXIT_TIMEOUT", "25"))
TERM_EXIT_TIMEOUT = float(os.environ.get("TERM_EXIT_TIMEOUT", "8"))
JAM_SETTLE = float(os.environ.get("JAM_SETTLE", "3"))


def spawn_on_pty(bin_path: str) -> tuple[int, int]:
    """Spawn the binary on a fresh PTY; return (pid, master_fd)."""

    pid, master = pty.fork()
    if pid == 0:
        # Child: the pty slave is stdin/stdout/stderr and the
        # controlling terminal. Set the winsize BEFORE exec so the
        # child never observes a degenerate 0x0 viewport (it may not
        # re-query until SIGWINCH).
        fcntl.ioctl(
            0,
            termios.TIOCSWINSZ,
            struct.pack("HHHH", ROWS, COLS, 0, 0),
        )
        os.environ["TERM"] = "xterm-256color"
        try:
            os.execv(bin_path, [bin_path])
        except OSError:
            os._exit(127)
    return pid, master


def wait_exit(pid: int, timeout: float) -> tuple[bool, int | None]:
    """Poll waitpid(WNOHANG) until exit or timeout. True + status."""

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        wpid, status = os.waitpid(pid, os.WNOHANG)
        if wpid == pid:
            return True, status
        time.sleep(0.05)
    return False, None


def describe_status(status: int) -> str:
    if os.WIFEXITED(status):
        return f"exit {os.WEXITSTATUS(status)}"
    if os.WIFSIGNALED(status):
        return f"signal {os.WTERMSIG(status)}"
    return f"status {status}"


def kill_straggler(pid: int, master: int) -> None:
    try:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    except ProcessLookupError:
        pass
    try:
        os.close(master)
    except OSError:
        pass


def phase_jam() -> bool:
    """Phase 1: jam the PTY (never read) and expect the watchdog exit."""

    pid, master = spawn_on_pty(BIN)
    t0 = time.monotonic()
    # The jam IS the test: no select, no read on master, ever. The
    # kernel pty buffer fills, the child's writes block.
    exited, status = wait_exit(pid, JAM_EXIT_TIMEOUT)
    elapsed = time.monotonic() - t0
    if exited and status is not None:
        print(
            f"[+] phase 1 (jam only): exited in {elapsed:.1f}s "
            f"({describe_status(status)}) — watchdog nonblocking force-exit works"
        )
        ok = True
    else:
        print(
            f"[X] phase 1 (jam only): STILL ALIVE after {JAM_EXIT_TIMEOUT:.0f}s "
            "— the hang reproduced (watchdog exit path is wedged)"
        )
        ok = False
    if not exited:
        kill_straggler(pid, master)
    else:
        try:
            os.close(master)
        except OSError:
            pass
    return ok


def phase_sigterm() -> bool:
    """Phase 2: jam, then SIGTERM (the pkill path), expect fast death."""

    pid, master = spawn_on_pty(BIN)
    time.sleep(JAM_SETTLE)  # let the buffer fill and the loop wedge
    t0 = time.monotonic()
    os.kill(pid, signal.SIGTERM)
    exited, status = wait_exit(pid, TERM_EXIT_TIMEOUT)
    elapsed = time.monotonic() - t0
    if exited and status is not None:
        code = os.WEXITSTATUS(status) if os.WIFEXITED(status) else None
        # 143 = 128+SIGTERM (signal thread force-exit), 1 = the
        # watchdog's own exit; both satisfy the pkill contract. 0
        # would mean a graceful exit happened while jammed — fine
        # too, but not expected on this path.
        print(
            f"[+] phase 2 (jam + SIGTERM): dead in {elapsed:.1f}s "
            f"({describe_status(status)}) — SIGTERM is lethal again"
        )
        ok = True
        if code is not None and code not in (0, 1, 143):
            print(f"[!] phase 2: unexpected exit code {code} (expected 1/143/0)")
    else:
        print(
            f"[X] phase 2 (jam + SIGTERM): STILL ALIVE {elapsed:.1f}s "
            "after SIGTERM — pkill reproduced as a no-op"
        )
        ok = False
    if not exited:
        kill_straggler(pid, master)
    else:
        try:
            os.close(master)
        except OSError:
            pass
    return ok


def main() -> int:
    if not os.path.exists(BIN):
        print(f"[X] binary not found: {BIN} (build it or set BIN=)")
        return 1
    print(f"termux-hang harness: BIN={BIN} SIZE={COLS}x{ROWS}")
    ok1 = phase_jam()
    ok2 = phase_sigterm()
    if ok1 and ok2:
        print("PASS: both jam phases exited — the Termux hang is fixed")
        return 0
    print("FAIL: at least one jam phase hung — the deadlock reproduced")
    return 1


if __name__ == "__main__":
    sys.exit(main())
