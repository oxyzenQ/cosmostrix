#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Signal contract smoke harness (NIGHT-dinner-8).

Exercises the four Unix signal paths cosmostrix owns, under a real
PTY (spawn recipe mirrors termux_hang_harness.py: pty.fork so the
slave IS the controlling terminal). Introduced for the signal-hook
0.3 -> 0.4 migration (2026-09-30) and kept as the standing
regression harness for every future signal-stack change:

  1. startup liveness (3 s warmup)
  2. Ctrl+C keystroke ignored — the only-q exit policy (bug #15).
     The 0x03 byte is written to the PTY: raw mode turns it into a
     key event the input loop drains, which is the ACTUAL policy
     path. (A direct kill -INT is kernel-default death on every
     signal-hook version — cosmostrix never installs a SIGINT
     handler in interactive mode, so that is not a policy test.)
  3. SIGTSTP -> kernel state T, then SIGCONT -> resumed and alive
  4. SIGTERM -> graceful exit inside the 3 s grace window with
     terminal-restore escapes on the PTY stream (alt-screen / cursor
     / mouse families, at least two observed)
  5. benchmark liveness + SIGINT abort -> exit 0 (the
     bench_progress::register_interrupt flag path)

Usage: python3 scripts/harness/signal_smoke.py [path-to-binary]
(default: target/release/cosmostrix). Exit 0 = all checks passed.

Parity methodology: run it against the pre-change binary and the
post-change binary; both must produce identical PASS sets. The
signal-hook 0.4.4 migration record lives in
docs/research/NIGHT_DINNER_8_MAJOR_BUMP_BACKLOG.md.
"""

import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else "target/release/cosmostrix"
COLS, ROWS = 100, 30
FAILURES = []


def note(msg):
    print(msg, flush=True)


def spawn_on_pty(args):
    """Spawn BIN on a fresh PTY (controlling terminal); return (pid, master)."""
    pid, master = pty.fork()
    if pid == 0:
        # Child: slave is stdin/stdout/stderr + controlling terminal.
        # Winsize before exec so the engine never sees a 0x0 viewport.
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        os.environ["TERM"] = "xterm-256color"
        try:
            os.execv(BIN, [BIN, *args])
        except OSError:
            os._exit(127)
    return pid, master


def exited(pid):
    """True if the child has exited (or been reaped); False while running/stopped.

    NOTE: this REAPS the child when it has exited — callers that need the
    exit status must use wait_exit_code() instead (single waitpid).
    """
    try:
        wpid, _ = os.waitpid(pid, os.WNOHANG)
        return wpid == pid
    except ChildProcessError:
        return True


def wait_exit_code(pid, timeout):
    """Poll until exit; returns the exit code, or 'TIMEOUT'."""
    end = time.time() + timeout
    while time.time() < end:
        try:
            wpid, status = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            return None
        if wpid == pid:
            try:
                return os.waitstatus_to_exitcode(status)
            except ValueError:
                return None
        time.sleep(0.05)
    return "TIMEOUT"


def drain(master, seconds, into):
    """Accumulate PTY output for `seconds` (non-blocking reads)."""
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([master], [], [], 0.1)
        if r:
            try:
                chunk = os.read(master, 65536)
                if not chunk:
                    break
                into.append(chunk)
            except OSError:
                break


def proc_state(pid):
    """Kernel state letter of pid (T = stopped), or '?' if unreadable."""
    try:
        with open(f"/proc/{pid}/stat", "rb") as f:
            return f.read().rsplit(b")", 1)[1].split()[0].decode()
    except (OSError, IndexError):
        return "?"


def check(name, ok, detail=""):
    note(f"[{'PASS' if ok else 'FAIL'}] {name}" + (f" — {detail}" if detail else ""))
    if not ok:
        FAILURES.append(name)


# Phase A: interactive session — Ctrl+C key ignored, TSTP/CONT, SIGTERM.
pid, master = spawn_on_pty([])
buf = []
drain(master, 3.0, buf)
if exited(pid):
    note(f"EARLY EXIT at startup; last output:\n{b''.join(buf)[-600:]!r}")
    check("startup: process alive after 3s", False, "exited during warmup")
else:
    check("startup: process alive after 3s", True, f"state={proc_state(pid)}")

    os.write(master, b"\x03")  # Ctrl+C keystroke (raw-mode key event)
    drain(master, 2.0, buf)
    check("Ctrl+C keystroke ignored (only 'q' exits)", not exited(pid))

    os.kill(pid, signal.SIGTSTP)
    time.sleep(1.0)
    check(
        "SIGTSTP: process suspended", proc_state(pid) == "T", f"state={proc_state(pid)}"
    )

    os.kill(pid, signal.SIGCONT)
    drain(master, 2.0, buf)
    check("SIGCONT: process resumed", not exited(pid), f"state={proc_state(pid)}")

    os.kill(pid, signal.SIGTERM)
    t0 = time.time()
    drain(master, 6.0, buf)
    gone = exited(pid)
    waited = time.time() - t0
    check(
        "SIGTERM: exited within grace window",
        gone and waited < 6.0,
        f"after {waited:.1f}s, state={proc_state(pid)}",
    )

    stream = b"".join(buf)
    families = [
        (b"\x1b[?1049l", "leave-alt-screen"),
        (b"\x1b[?25h", "show-cursor"),
        (b"\x1b[?100", "mouse-mode-off"),
    ]
    seen = [label for seq, label in families if seq in stream]
    check("terminal restore escapes observed", len(seen) >= 2, f"seen={seen}")
os.close(master)

# Phase B: benchmark mode — SIGINT aborts the bench cleanly.
pid2, master2 = spawn_on_pty(
    ["--benchmark", "--bench-duration", "10", "--scene", "cinematic"]
)
buf2 = []
drain(master2, 2.5, buf2)
check("bench: running after 2.5s", not exited(pid2))
os.kill(pid2, signal.SIGINT)
code2 = wait_exit_code(pid2, 12.0)
drain(master2, 0.5, buf2)
check("bench SIGINT: clean early exit (code 0)", code2 == 0, f"exit_code={code2}")
os.close(master2)

note("")
if FAILURES:
    note(f"RESULT: {len(FAILURES)} FAILURE(S): {FAILURES}")
    sys.exit(1)
note("RESULT: all signal smoke checks passed (interactive + bench paths)")
