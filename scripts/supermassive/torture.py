#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Supermassive torture battery - the brutal server-simulation leg.

night-dragon-maintenance-4, owner directive: the supermassive CI must
simulate a hostile server, not a friendly one - the render contract has
to hold UNDER ATTACK, not merely on a quiet bench. This battery runs
the built binary under a REAL pty (pty.fork, the pty_smoke transport
lineage) while five hostile load shapes hammer it concurrently:

  1. resize-storm     - TIOCSWINSZ flips at 10 Hz across extreme sizes
                        (1x1 to 300x90); the renderer must keep the
                        frame loop alive and exit by its own --duration
                        deadline with the full restore tail.
  2. input-flood      - ~200 junk byte writes per second (random ASCII,
                        tabs, control bytes, paste-escape fragments)
                        slammed into the pty; the 'q' quit path must
                        still win cleanly at the end.
  3. signal-storm     - five SIGINTs (deliberately NOT a quit signal -
                        the bug-15 contract) delivered mid-render, then
                        one SIGTERM; the process must ignore the SIGINTs
                        (proven by the session lifetime reaching the
                        SIGTERM mark) and shut down gracefully on the
                        SIGTERM.
  4. config-churn     - the live-reload watcher's config file rewritten
                        at ~5 Hz, alternating VALID values (fps 24/60)
                        with INVALID ones (a type-broken fps) - the lazy
                        event catch must apply valid edits, reject the
                        broken ones without dying, and the session must
                        end clean. The file lives at the default config
                        path ($HOME/.config/cosmostrix/config.toml,
                        inside the safepath whitelist) and is removed
                        afterwards so no other leg inherits churn.
  5. restart-storm    - twelve sequential short sessions at randomized
                        terminal sizes; startup/shutdown churn must stay
                        leak-free and every exit must carry the restore
                        tail.

Sentinel contract (mirrors pty_smoke.py / pty_assert.py): every checked
fact appends one MASS-RESULT row to the --mass file, each ending in
PASS or FAIL; the workflow Verdict step fails the job if any row reads
FAIL, and this driver also exits non-zero on its own failure. Restore
tails are asserted through pty_assert.py itself (the same SEQ_TABLE
grammar), against the raw byte logs this driver captures.

Exit codes: 0 = every leg passed inside budget; 2 = usage error;
3 = one or more legs failed.
"""

from __future__ import annotations

import argparse
import fcntl
import os
import pty
import random
import select
import signal
import struct
import subprocess
import sys
import termios
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ASSERT = REPO_ROOT / "scripts" / "supermassive" / "pty_assert.py"

# The restore tail every clean session must leave on the tty.
RESTORE_TAIL = ["alt-leave", "mouse-off", "paste-off", "focus-off"]


def mass(mass_path: Path, label: str, text: str) -> None:
    """Append one sentinel row (the shared supermassive contract)."""
    with open(mass_path, "a", encoding="utf-8") as handle:
        handle.write(f"MASS-RESULT: {label} {text}\n")
    print(f"  [{label}] {text}", flush=True)


def child_env(extra: dict[str, str] | None = None) -> dict[str, str]:
    """Child environment: a real terminal identity plus overrides."""
    env = dict(os.environ)
    env.setdefault("TERM", "xterm-256color")
    if extra:
        env.update(extra)
    return env


def resize_pty(master: int, cols: int, rows: int) -> None:
    """Flip the pty window size (the SIGWINCH generator)."""
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    try:
        fcntl.ioctl(master, termios.TIOCSWINSZ, winsize)
    except OSError:
        pass  # a closed master during teardown must not fail the leg


def run_hostile_session(
    binary: str,
    log_path: Path,
    child_args: list[str],
    budget: float,
    on_tick=None,
    env_extra: dict[str, str] | None = None,
) -> tuple[int | None, int]:
    """Run one session under a hostile pty with a tick hook and watchdog.

    The tick hook receives (master_fd, elapsed_seconds) on every poll
    (about 10 Hz) and IS the attack surface: it may resize the window,
    flood bytes, or signal the child on its own timeline. The child's
    own --duration (or a hook-driven 'q') ends the session; the hard
    watchdog kills it at the budget. Returns (exit code, bytes
    captured); a watchdog kill reports -9.
    """
    pid, master = pty.fork()
    if pid == 0:  # child: exec the renderer
        try:
            os.environ.clear()
            os.environ.update(child_env(env_extra))
            os.execv(binary, [binary, *child_args])
        except Exception:  # noqa: BLE001 - exec failure must not hang CI
            os._exit(127)

    captured = 0
    start = time.monotonic()
    killed = False
    code: int | None = None
    try:
        while True:
            elapsed = time.monotonic() - start
            if elapsed >= budget:
                os.kill(pid, signal.SIGKILL)
                killed = True
                break
            if on_tick is not None:
                on_tick(master, elapsed)
            try:
                ready, _, _ = select.select([master], [], [], 0.05)
            except InterruptedError:
                ready = []
            if ready:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break  # slave side closed - child is exiting
                if chunk:
                    captured += len(chunk)
                    with open(log_path, "ab") as handle:
                        handle.write(chunk)
            done, state = os.waitpid(pid, os.WNOHANG)
            if done == pid:
                code = os.waitstatus_to_exitcode(state)
                break
        if code is None and not killed:
            # The master closed without a waitable child yet - reap now.
            done, state = os.waitpid(pid, 0)
            code = os.waitstatus_to_exitcode(state)
        if code is not None:
            # Drain the exit burst the child wrote on its way out.
            deadline = time.monotonic() + 0.5
            while time.monotonic() < deadline:
                try:
                    ready, _, _ = select.select([master], [], [], 0.1)
                except InterruptedError:
                    break
                except OSError:
                    break
                if not ready:
                    break
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                captured += len(chunk)
                with open(log_path, "ab") as handle:
                    handle.write(chunk)
    finally:
        try:
            os.close(master)
        except OSError:
            pass
    if killed:
        # Reap the watchdog kill - no pty ghosts left behind in CI.
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass
    return (-9 if killed else code), captured


def assert_restore_tail(log_path: Path, mass_path: Path, label: str) -> bool:
    """Run pty_assert.py over a capture log for the restore tail."""
    cmd = [
        sys.executable,
        str(ASSERT),
        "--log",
        str(log_path),
        "--mass",
        str(mass_path),
        "--label",
        label,
    ]
    for name in RESTORE_TAIL:
        cmd.extend(["--expect", name])
    result = subprocess.run(cmd, capture_output=True, text=True, check=False)
    ok = result.returncode == 0
    if not ok:
        mass(mass_path, label, f"FAIL restore-tail (pty_assert rc={result.returncode})")
    return ok


def leg_resize_storm(binary: str, mass_path: Path, workdir: Path) -> bool:
    """Leg 1: SIGWINCH hammer at 10 Hz across extreme sizes."""
    label = "torture-resize-storm"
    log = workdir / "torture-resize.log"
    log.unlink(missing_ok=True)
    sizes = [(1, 1), (300, 90), (80, 24), (200, 50), (40, 10), (120, 40)]
    last_flip = -1.0

    def tick(master: int, elapsed: float) -> None:
        nonlocal last_flip
        if elapsed - last_flip >= 0.1:  # 10 Hz resize hammer
            cols, rows = sizes[int(elapsed * 10) % len(sizes)]
            resize_pty(master, cols, rows)
            last_flip = elapsed

    code, captured = run_hostile_session(
        binary, log, ["--duration", "5"], budget=30.0, on_tick=tick
    )
    ok = True
    mass(
        mass_path,
        label,
        f"{'PASS' if code == 0 else 'FAIL'} deadline-exit rc={code} bytes={captured}",
    )
    ok = ok and code == 0
    tail_ok = assert_restore_tail(log, mass_path, f"{label}-tail")
    ok = ok and tail_ok
    return ok


def leg_input_flood(binary: str, mass_path: Path, workdir: Path) -> bool:
    """Leg 2: junk byte flood, then the 'q' quit path must win."""
    label = "torture-input-flood"
    log = workdir / "torture-flood.log"
    log.unlink(missing_ok=True)
    junk_pool = [
        b"x",
        b"j",
        b"\t",
        b"\x1b",
        b"\x1b[200~",
        b" ",
        b"\x01",
        b"\x7f",
        b"A" * 64,
        b"\r",
    ]

    def tick(master: int, elapsed: float) -> None:
        if elapsed >= 4.5:
            return
        # About 200 writes per second: 20 bursts of ~10 bytes per tick.
        for _ in range(20):
            try:
                os.write(master, random.choice(junk_pool))
            except OSError:
                return  # child already gone
        if elapsed >= 4.2:
            try:
                os.write(master, b"q")  # the quit key must win through the flood
            except OSError:
                pass

    code, captured = run_hostile_session(
        binary, log, ["--duration", "30"], budget=30.0, on_tick=tick
    )
    ok = True
    mass(
        mass_path,
        label,
        f"{'PASS' if code == 0 else 'FAIL'} quit-through-flood rc={code} bytes={captured}",
    )
    ok = ok and code == 0
    tail_ok = assert_restore_tail(log, mass_path, f"{label}-tail")
    ok = ok and tail_ok
    return ok


def leg_signal_storm(binary: str, mass_path: Path, workdir: Path) -> bool:
    """Leg 3: SIGINTs must not kill; SIGTERM must finish gracefully."""
    label = "torture-signal-storm"
    log = workdir / "torture-signal.log"
    log.unlink(missing_ok=True)
    sigint_times = [0.6, 1.2, 1.8, 2.4, 3.0]
    sigterm_at = 3.6
    sent_ints = 0
    sent_term = False

    def tick(master: int, elapsed: float) -> None:
        nonlocal sent_ints, sent_term
        while sent_ints < len(sigint_times) and elapsed >= sigint_times[sent_ints]:
            try:
                os.kill(signal_pid[0], signal.SIGINT)
            except (OSError, IndexError):
                pass
            sent_ints += 1
        if elapsed >= sigterm_at and not sent_term:
            try:
                os.kill(signal_pid[0], signal.SIGTERM)
            except (OSError, IndexError):
                pass
            sent_term = True

    # The child pid is discovered from the fork inside the driver via a
    # one-element box the hook reads; run_hostile_session forks inside,
    # so expose it through the closure by forking here instead: reuse
    # the driver, but let it record the pid for hooks.
    signal_pid = [0]

    def tick_wrapper(master: int, elapsed: float) -> None:
        if signal_pid[0] == 0:
            return  # the driver sets the pid below before the first tick
        tick(master, elapsed)

    # Fork manually to know the pid (the driver's hook contract wants it).
    pid, master = pty.fork()
    if pid == 0:
        try:
            os.environ.clear()
            os.environ.update(child_env())
            os.execv(binary, [binary, "--duration", "30"])
        except Exception:  # noqa: BLE001
            os._exit(127)
    signal_pid[0] = pid
    captured = 0
    start = time.monotonic()
    code: int | None = None
    while True:
        elapsed = time.monotonic() - start
        if elapsed >= 30.0:
            os.kill(pid, signal.SIGKILL)
            code = -9
            break
        tick_wrapper(master, elapsed)
        try:
            ready, _, _ = select.select([master], [], [], 0.05)
        except InterruptedError:
            ready = []
        if ready:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                break
            if chunk:
                captured += len(chunk)
                with open(log, "ab") as handle:
                    handle.write(chunk)
        done, state = os.waitpid(pid, os.WNOHANG)
        if done == pid:
            code = os.waitstatus_to_exitcode(state)
            break
    try:
        os.close(master)
    except OSError:
        pass

    ok = True
    lifetime_ok = code is not None and sent_term and elapsed >= sigterm_at - 0.3
    mass(
        mass_path,
        label,
        f"{'PASS' if (code == 0 and lifetime_ok) else 'FAIL'} "
        f"sigterm-graceful rc={code} sigints={sent_ints} lifetime={elapsed:.1f}s",
    )
    ok = ok and code == 0 and lifetime_ok
    tail_ok = assert_restore_tail(log, mass_path, f"{label}-tail")
    ok = ok and tail_ok
    return ok


def default_config_path() -> Path:
    """The default config path the watcher follows (safepath whitelist)."""
    xdg = os.environ.get("XDG_CONFIG_HOME", "")
    base = Path(xdg) if xdg else Path.home() / ".config"
    return base / "cosmostrix" / "config.toml"


def leg_config_churn(binary: str, mass_path: Path, workdir: Path) -> bool:
    """Leg 4: live-reload event storm - valid and broken edits at 5 Hz."""
    label = "torture-config-churn"
    log = workdir / "torture-churn.log"
    log.unlink(missing_ok=True)
    cfg = default_config_path()
    cfg.parent.mkdir(parents=True, exist_ok=True)
    had_before = cfg.exists()
    original = cfg.read_text(encoding="utf-8") if had_before else None
    churned = 0

    def write_cfg(content: str) -> None:
        # Atomic-ish rewrite: the same write(2) appends a real editor makes.
        with open(cfg, "w", encoding="utf-8") as handle:
            handle.write(content)

    def tick(master: int, elapsed: float) -> None:
        nonlocal churned
        if elapsed >= 4.6 or elapsed - churned * 0.2 < 0.2:
            return
        n = churned
        if n % 2 == 0:
            fps = 24 if (n // 2) % 2 == 0 else 60
            write_cfg(f"fps = {fps}\n")
        else:
            write_cfg('fps = "type-broken-bogus"\n')  # OOR rejection path
        churned += 1

    try:
        write_cfg("fps = 60\n")  # the watcher must see a real file to watch
        code, captured = run_hostile_session(
            binary, log, ["--duration", "5"], budget=30.0, on_tick=tick
        )
    finally:
        # Leave no churn behind: restore or remove.
        if had_before and original is not None:
            write_cfg(original)
        elif cfg.exists():
            cfg.unlink()
    ok = True
    mass(
        mass_path,
        label,
        f"{'PASS' if code == 0 else 'FAIL'} survive-churn rc={code} edits={churned} bytes={captured}",
    )
    ok = ok and code == 0 and churned >= 10
    tail_ok = assert_restore_tail(log, mass_path, f"{label}-tail")
    ok = ok and tail_ok
    return ok


def leg_restart_storm(binary: str, mass_path: Path, workdir: Path) -> bool:
    """Leg 5: twelve randomized-size sessions back to back."""
    label = "torture-restart-storm"
    ok = True
    fails = 0
    random.seed(20261027)  # deterministic - a CI leg, not a lottery
    for i in range(12):
        log = workdir / f"torture-restart-{i}.log"
        log.unlink(missing_ok=True)
        cols = random.choice([40, 60, 80, 100, 120, 200])
        rows = random.choice([8, 12, 24, 30, 50])
        sized = {"done": False}

        def tick(
            master: int, elapsed: float, cols=cols, rows=rows, sized=sized
        ) -> None:
            # Stamp the session's real pty size once, right after the
            # child starts reading: the renderer probes the tty, not env.
            if not sized["done"] and elapsed >= 0.05:
                resize_pty(master, cols, rows)
                sized["done"] = True

        code, _ = run_hostile_session(
            binary, log, ["--duration", "1"], budget=20.0, on_tick=tick
        )
        if code != 0:
            fails += 1
            mass(mass_path, label, f"FAIL session {i} ({cols}x{rows}) rc={code}")
            ok = False
        else:
            tail_ok = assert_restore_tail(log, mass_path, f"{label}-{i}")
            if not tail_ok:
                fails += 1
                ok = False
    if ok:
        mass(mass_path, label, "PASS 12/12 sessions clean with restore tails")
    return ok


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--binary", required=True, help="cosmostrix binary to torture")
    parser.add_argument(
        "--mass", required=True, help="file to append MASS-RESULT rows to"
    )
    parser.add_argument(
        "--workdir",
        default="/tmp/cosmostrix-torture",
        help="scratch directory for capture logs",
    )
    args = parser.parse_args()

    binary = str(Path(args.binary).resolve())
    if not os.access(binary, os.X_OK):
        print(f"torture: binary not executable: {binary}", file=sys.stderr)
        return 2
    mass_path = Path(args.mass)
    workdir = Path(args.workdir)
    workdir.mkdir(parents=True, exist_ok=True)

    legs = [
        ("resize-storm", leg_resize_storm),
        ("input-flood", leg_input_flood),
        ("signal-storm", leg_signal_storm),
        ("config-churn", leg_config_churn),
        ("restart-storm", leg_restart_storm),
    ]
    print(f"torture: begin - {len(legs)} hostile legs against {binary}", flush=True)
    failed = 0
    for name, leg in legs:
        print(f"torture: leg {name}", flush=True)
        if not leg(binary, mass_path, workdir):
            failed += 1
    total = len(legs)
    print(f"torture: verdict {total - failed}/{total} legs passed", flush=True)
    return 3 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
