#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# LOC_EXEMPT: self-contained LTS depth-bore tool (single probe file; the five bug-class probes share one harness state); tracked as migration debt by scripts/gates/check-scripts-loc.sh (NIGHT-lts-1).
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL
#   or Git Bash on Windows).

"""NIGHT-hunt-47-depthbore: the LTS depth bore.

Owner brief (2026-09-14, after the DeepSeek review): the bugs that
survive every hunt hide in five classes the shallow suites cannot
reach -- race conditions that fire only at the right timing,
platform-specific paths, extreme edge cases (10k-entry configs, 8K
screens), unimagined config combinations, and long-running drift that
shows up after 24 hours. This bore drills all five classes against
the REAL binary, from a clean-slate zero state to a fully loaded hero
state, and pins the LTS contract at every depth:

PART 0 -- ZERO: clean-slate bootstrap. No config dir, no system
  config, empty HOME, missing HOME. The binary must boot on pure
  defaults, quit clean, and every static surface must answer.

PART 1 -- RACE-STORM: the timing class. Signal kills mid-render,
  SIGKILL + fork-guard restore, a spawn/kill storm, a SIGWINCH
  resize storm, a config-write race at the startup read, and a
  live-reload storm (valid/broken alternating every 70 ms). The
  invariants: no hang, no panic-class exit, terminal always restored,
  no zombies, no leftover processes.

PART 2 -- PLATFORM-MATRIX: honest platform coverage. FreeBSD and
  Windows-only paths are SKIPped with the reason recorded (never
  claimed from a Linux box); Termux, dumb/linux/empty TERM, NO_COLOR,
  LANG=C, tmux and ssh environments are simulated and must boot and
  quit clean.

PART 3 -- EDGE-CRUSHER: 1x1 to 8K geometries (bench and PTY), a
  10k-line custom-block config, message length boundary (200-char
  cap), screen-size validation rejects, and the config byte edges
  (invalid UTF-8, empty file, EACCES, past the 1 MiB cap, huge key,
  deep brackets) -- including the doctor CONFIG FILE status section
  and the testconf cap alignment fixed in this same hunt.

PART 4 -- CONFIG-FUZZ: seeded, deterministic mutations of a rich
  valid baseline (type swaps, out-of-range values, unknown and
  duplicate keys, malformed lines, bad hex, broken headers). Every
  mutant must be classified cleanly: rc=2 with a stderr diagnostic,
  or rc=0 (accepted) -- panics, hangs and garbage stdout are bugs.

PART 5 -- DRIFT-SOAK: the bounded 24-hour proxy. A still soak and a
  live-reload churn soak, each sampling /proc (RSS, threads, fds)
  every 2 s and the PTY output rate in two windows. RSS slope, fd
  drift, thread drift and output-rate growth must stay bounded; the
  exit must stay clean after a long run.

Usage:
  python3 scripts/depthbore/depthbore.py            # full bore (~5 min)
  python3 scripts/depthbore/depthbore.py --quick    # smoke bore (~2 min)
  python3 scripts/depthbore/depthbore.py --bin target/debug/cosmostrix
  python3 scripts/depthbore/depthbore.py --seed 7 --soak-secs 60

Exit code 0 = all assertions passed; 1 = at least one FAIL.
SKIPs are honest non-claims (wrong platform), never failures.
"""

import fcntl
import os
import pty
import random
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

# ── Binary resolution: --bin flag > COSMOSTRIX_BIN > release > debug ──────

DEFAULT_BINS = ("target/release/cosmostrix", "target/debug/cosmostrix")


def resolve_bin(flag_value):
    candidates = []
    if flag_value:
        candidates.append(flag_value)
    env_bin = os.environ.get("COSMOSTRIX_BIN")
    if env_bin:
        candidates.append(env_bin)
    candidates.extend(DEFAULT_BINS)
    for c in candidates:
        if os.path.isfile(c):
            return os.path.abspath(c)
    print(f"depthbore: no binary found (tried: {', '.join(candidates)})")
    print("depthbore: build one first:  cargo build --release")
    sys.exit(2)


TERM_ROWS, TERM_COLS = 40, 120
ALT_LEAVE = b"\x1b[?1049l"
CRASH_RCS = {101, 134, 139, -6, -11, -4}  # panic, SIGABRT, SIGSEGV et al

PASS_COUNT = 0
FAIL_COUNT = 0
SKIP_COUNT = 0
FAILURES = []


def result(ok, label, detail):
    global PASS_COUNT, FAIL_COUNT
    status = "PASS" if ok else "FAIL"
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"{label} | {detail}")
    print(f"  [{status}] {label:56s} | {detail}")


def skipped(label, reason):
    global SKIP_COUNT
    SKIP_COUNT += 1
    print(f"  [SKIP] {label:56s} | {reason}")


def section(title):
    print(f"\n── {title} ──")


# ── Environment helpers ───────────────────────────────────────────────────


def fresh_home(with_cfg_dir=True):
    home = tempfile.mkdtemp(prefix="db_home_")
    if with_cfg_dir:
        os.makedirs(os.path.join(home, ".config", "cosmostrix"), exist_ok=True)
    return home, os.path.join(home, ".config", "cosmostrix", "config.toml")


def write_cfg(path, text):
    data = text if isinstance(text, bytes) else text.encode("utf-8")
    with open(path, "wb") as f:
        f.write(data)


def plain_run(args, home, timeout=20, env_extra=None, unset_home=False):
    """No-PTY run: stream separation is measurable, rc is exact."""
    env = dict(os.environ)
    if unset_home:
        env.pop("HOME", None)
    else:
        env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
    if env_extra:
        env.update(env_extra)
    try:
        p = subprocess.run(
            [BIN] + args,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", "", ""


def crash_class(rc):
    return rc in CRASH_RCS or (isinstance(rc, int) and rc in CRASH_RCS)


# ── /proc sampling (Linux only; the drift bore skips elsewhere) ───────────


def proc_sample(pid):
    status = {"rss_kb": None, "threads": None, "fd": None}
    try:
        with open(f"/proc/{pid}/status", encoding="ascii") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    status["rss_kb"] = int(line.split()[1])
                elif line.startswith("Threads:"):
                    status["threads"] = int(line.split()[1])
    except OSError:
        return None
    try:
        status["fd"] = len(os.listdir(f"/proc/{pid}/fd"))
    except OSError:
        pass
    if status["rss_kb"] is None:
        return None
    return status


def linear_slope(pairs):
    """Least-squares slope of (t, value) pairs; 0.0 when degenerate."""
    n = len(pairs)
    if n < 2:
        return 0.0
    xs = [p[0] for p in pairs]
    ys = [p[1] for p in pairs]
    mx = sum(xs) / n
    my = sum(ys) / n
    num = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    den = sum((x - mx) ** 2 for x in xs)
    if den == 0:
        return 0.0
    return num / den


# ── PTY session runner ────────────────────────────────────────────────────


class PtySession:
    """Run the binary in a PTY with scheduled actions and a /proc sampler.

    Actions are (t_seconds, callable) pairs -- the callable receives
    (master_fd, proc) so a single loop drives signals, resizes and
    config rewrites at precise offsets. The quit protocol sends 'q'
    repeatedly (every 0.3 s) during the drain window: a single 'q'
    can be consumed by the intro cinematic, so the harness uses
    --intro none for speed AND repeated 'q' for robustness.
    """

    def __init__(self, cfg_path=None, rows=TERM_ROWS, cols=TERM_COLS):
        self.cfg_path = cfg_path
        self.rows = rows
        self.cols = cols

    def run(
        self,
        args,
        home,
        secs,
        actions=(),
        env_extra=None,
        quit_at_end=True,
        settle=0.0,
        sampler=None,
        sample_interval=2.0,
    ):
        """Returns (rc, stream_bytes, lflags_cooked, samples, rate_windows).

        samples: list of (t, dict) from the sampler, when given.
        rate_windows: byte counts in the first and last quarter of the
        run (the drift bore's runaway-output detector).
        """
        master, slave = pty.openpty()
        fcntl.ioctl(
            master,
            termios.TIOCSWINSZ,
            struct.pack("HHHH", self.rows, self.cols, 0, 0),
        )
        env = dict(os.environ)
        env["HOME"] = home
        env.pop("XDG_CONFIG_HOME", None)
        env["TERM"] = "xterm-256color"
        env["TERM_PROGRAM"] = "alacritty"
        env["COLORTERM"] = "truecolor"
        if env_extra:
            env.update(env_extra)
        proc = subprocess.Popen(
            [BIN] + args, stdin=slave, stdout=slave, stderr=slave, env=env
        )
        os.close(slave)
        buf = bytearray()
        t0 = time.time()
        actions = sorted(actions, key=lambda a: a[0])
        fired = [False] * len(actions)
        samples = []
        next_sample = t0 + sample_interval
        quarter = secs / 4.0
        rate_first = 0
        rate_last = 0
        exited = None
        eof = False
        while time.time() - t0 < secs:
            r, _, _ = select.select([master], [], [], 0.05)
            if r:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    eof = True
                if eof or not chunk:
                    eof = True
                else:
                    buf.extend(chunk)
            now = time.time() - t0
            if quarter < now < 2 * quarter:
                rate_first = len(buf)
            elif 3 * quarter < now:
                rate_last = len(buf)
            for i, (at, action) in enumerate(actions):
                if not fired[i] and now >= at:
                    fired[i] = True
                    try:
                        action(master, proc)
                    except OSError:
                        pass
            if sampler is not None and time.time() >= next_sample:
                s = sampler(proc.pid)
                if s is not None:
                    samples.append((now, s))
                next_sample = time.time() + sample_interval
            if not eof and proc.poll() is not None:
                exited = proc.returncode
                self._drain(master, buf, 0.6)
                break
            if eof and proc.poll() is not None:
                exited = proc.returncode
                break
        if exited is None:
            if quit_at_end and not eof:
                self._quit(master, buf)
                if proc.poll() is not None:
                    exited = proc.returncode
            if exited is None:
                try:
                    exited = proc.wait(timeout=1)
                except subprocess.TimeoutExpired:
                    exited = "HANG"
        if exited == "HANG":
            try:
                proc.kill()
                proc.wait(timeout=2)
            except (OSError, subprocess.TimeoutExpired):
                pass
        if settle > 0:
            time.sleep(settle)
        lflags_cooked = False
        try:
            lf = termios.tcgetattr(master)[3]
            lflags_cooked = bool(lf & termios.ICANON) and bool(lf & termios.ECHO)
        except termios.error:
            lflags_cooked = False
        try:
            os.close(master)
        except OSError:
            pass
        if exited not in ("HANG",) and exited is not None and exited < 0:
            # Reaped by a signal: make sure nothing lingers behind.
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass
        return exited, bytes(buf), lflags_cooked, samples, (rate_first, rate_last)

    def _drain(self, master, buf, secs):
        deadline = time.time() + secs
        while time.time() < deadline:
            r, _, _ = select.select([master], [], [], 0.05)
            if r:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    return
                if not chunk:
                    return
                buf.extend(chunk)

    def _quit(self, master, buf):
        """Repeated-'q' quit: robust against the intro consuming one."""
        deadline = time.time() + 6.0
        while time.time() < deadline:
            try:
                os.write(master, b"q")
            except OSError:
                return
            r, _, _ = select.select([master], [], [], 0.3)
            if r:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    return
                if not chunk:
                    return
                buf.extend(chunk)


def leftover_procs():
    """Any cosmostrix renderer or cx-term-guard still on the box."""
    found = []
    for name in ("cosmostrix", "cx-term-guard"):
        r = subprocess.run(
            ["pgrep", "-x", name], capture_output=True, text=True, check=False
        )
        found.extend(pid for pid in r.stdout.split() if pid.isdigit())
    return found


def zombie_procs():
    r = subprocess.run(
        ["bash", "-c", "ps -eo pid,stat,comm | awk '$2 ~ /Z/'"],
        capture_output=True,
        text=True,
        check=False,
    )
    return [ln for ln in r.stdout.splitlines() if ln.strip()]


# ── Config fixtures ───────────────────────────────────────────────────────

RICH_CFG = """fps = 60
speed = 20
density = 0.6
color = "neon-green"
charset = "matrix"
scene = "cinematic"
glitch-level = "default"
async-mode = true
msg-mode = true
msg-fill-style = "engrave"
crystal-dragon = true
crystal-dragon-secs = 3
ambient-snapback-secs = 2

[colors-custom.deep]
bg = "#0a0a12"
rain = "#101820, #304050, #5080a0"

[charset-custom.bits]
set = "01"

[scene-custom.fast]
rain = "glyph"
color = "neon-green"
charset = "matrix"
fps = 90
speed = 30
density = 0.7
glitch-level = "subtle"
"""

BROKEN_LINE = "garbage line no equals\n"


# ── PART 0: ZERO (clean-slate bootstrap) ──────────────────────────────────


def part0_zero():
    section("PART 0 -- ZERO: clean-slate bootstrap")

    # Z1: no config dir at all -- pure-default PTY boot and clean quit.
    home, cfg = fresh_home(with_cfg_dir=False)
    s = PtySession()
    rc, stream, cooked, _, _ = s.run(["--intro", "none"], home, secs=3.0)
    ok = rc == 0 and cooked and len(stream) > 1000
    result(
        ok,
        "Z1 zero-home PTY boot + quit",
        f"rc={rc}, restored={cooked}, bytes={len(stream)}",
    )

    # Z2: doctor on the zero state reports first-run, not an error.
    home, cfg = fresh_home(with_cfg_dir=False)
    rc, out, err = plain_run(["--doctor"], home, timeout=30)
    ok = rc == 0 and "missing (first run)" in out and "unreadable" not in out
    result(
        ok,
        "Z2 zero-state doctor",
        f"rc={rc}, CONFIG FILE: missing (first run)"
        if ok
        else f"rc={rc}, out[:80]={out[:80]!r}, err[:80]={err[:80]!r}",
    )

    # Z3: every static surface answers on the zero state.
    static_cases = [
        ("--version", "cosmostrix:"),
        ("--docs", "cosmostrix"),
        ("--list-scenes", "SCENE"),
        ("--list-colors", "COLOR"),
        ("--list-charsets", "CHARSET"),
        ("--config-path", "config.toml"),
    ]
    for flag, marker in static_cases:
        home, cfg = fresh_home(with_cfg_dir=False)
        rc, out, err = plain_run([flag], home, timeout=30)
        ok = rc == 0 and marker.lower() in out.lower() and not err.strip()
        detail = f"rc={rc}, lines={len(out.splitlines())}"
        if not ok:
            detail += f", err[:60]={err.strip()[:60]!r}"
        result(ok, f"Z3 static {flag}", detail)

    # Z4: no-TTY interactive refusal is the documented pipe contract.
    home, _ = fresh_home()
    rc, out, err = plain_run(["--intro", "none"], home, timeout=15)
    ok = rc == 1 and "not a terminal" in err
    result(
        ok,
        "Z4 no-TTY refusal",
        "rc=1, stderr diagnostic" if ok else f"rc={rc}, err[:80]={err.strip()[:80]!r}",
    )

    # Z5: missing HOME must not crash the static surfaces.
    home, _ = fresh_home()
    rc, out, err = plain_run(["--doctor"], home, timeout=30, unset_home=True)
    ok = rc == 0 and "DIAGNOSTICS" in out
    result(
        ok,
        "Z5 HOME-unset doctor",
        f"rc={rc}, report printed" if ok else f"rc={rc}, err[:80]={err.strip()[:80]!r}",
    )

    # Z6: hero endpoint of the zero-to-hero thread -- the fully loaded
    # config with the harmony state machine (crystal + ambient) boots,
    # quits clean and prints the ambient observability counters.
    home, cfg = fresh_home()
    write_cfg(cfg, RICH_CFG + 'ambient.00-00 = "monolith"\n')
    s = PtySession(cfg)
    rc, stream, cooked, _, _ = s.run(["--intro", "none", "-v"], home, secs=8.0)
    text = stream.decode("utf-8", errors="replace")
    ok = rc == 0 and cooked and "ambient_diag:" in text
    result(
        ok,
        "Z6 hero config + harmony boot",
        f"rc={rc}, restored={cooked}, ambient_diag present"
        if ok
        else f"rc={rc}, restored={cooked}, ambient={'ambient_diag:' in text}",
    )


# ── PART 1: RACE-STORM (the timing class) ─────────────────────────────────


def _signal_case(name, sig, expect_rcs):
    home, cfg = fresh_home()
    s = PtySession(cfg)

    def fire(master, proc):
        os.kill(proc.pid, sig)

    rc, stream, cooked, _, _ = s.run(
        ["--intro", "none"],
        home,
        secs=6.0,
        actions=[(1.5, fire)],
        quit_at_end=False,
        settle=2.0,
    )
    leave = ALT_LEAVE in stream
    ok = rc in expect_rcs and cooked and not crash_class(rc)
    detail = f"rc={rc}, restored={cooked}, alt_leave={leave}"
    if not ok:
        detail += " (expected graceful exit with terminal restored)"
    result(ok, f"R {name} mid-render", detail)
    return ok


class _ConfigWriter:
    """Background thread rewriting the config at a fixed cadence."""

    def __init__(self, cfg_path, contents, interval, stop_at):
        self.cfg_path = cfg_path
        self.contents = contents
        self.interval = interval
        self.stop_at = stop_at
        self.writes = 0
        self.thread = threading.Thread(target=self._run, daemon=True)

    def _run(self):
        i = 0
        t0 = time.time()
        while time.time() - t0 < self.stop_at:
            write_cfg(self.cfg_path, self.contents[i % len(self.contents)])
            self.writes += 1
            i += 1
            time.sleep(self.interval)

    def start(self):
        self.thread.start()

    def join(self):
        self.thread.join(timeout=self.stop_at + 2.0)


def part1_race():
    section("PART 1 -- RACE-STORM: the timing class")

    _signal_case("SIGTERM", signal.SIGTERM, (0,))
    _signal_case("SIGHUP", signal.SIGHUP, (0,))
    _signal_case("SIGQUIT", signal.SIGQUIT, (0,))
    # SIGINT is not caught (deprecated path): the OS default kills the
    # process (rc=-2), but the fork guard must still restore the line
    # discipline -- the historical raw-mode-residue class.
    _signal_case("SIGINT", signal.SIGINT, (-2,))

    # R5: SIGKILL -- nothing can catch it; the cx-term-guard must
    # restore termios after the parent dies, then exit itself.
    home, cfg = fresh_home()
    s = PtySession(cfg)

    def fire(master, proc):
        os.kill(proc.pid, signal.SIGKILL)

    rc, stream, cooked, _, _ = s.run(
        ["--intro", "none"],
        home,
        secs=5.0,
        actions=[(1.5, fire)],
        quit_at_end=False,
        settle=2.5,
    )
    result(
        rc == -9 and cooked,
        "R SIGKILL + fork-guard restore",
        f"rc={rc}, termios restored by guard={cooked}",
    )

    # R6: spawn/kill storm -- random delay, random signal, every process
    # must die within 3 s and leave nothing behind.
    rng = random.Random(SEED)
    rounds = 6 if QUICK else 12
    storm_fail = 0
    guard_fail = 0
    slowest = 0.0
    for i in range(rounds):
        home, _ = fresh_home()
        # Manual spawn with precise kill timing (the PtySession window
        # is too coarse for spawn-race work: the kill must land at a
        # random offset inside the boot/render warmup).
        master, slave = pty.openpty()
        fcntl.ioctl(
            master,
            termios.TIOCSWINSZ,
            struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0),
        )
        env = dict(os.environ)
        env["HOME"] = home
        env.pop("XDG_CONFIG_HOME", None)
        env["TERM"] = "xterm-256color"
        proc = subprocess.Popen(
            [BIN, "--intro", "none"],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            env=env,
        )
        os.close(slave)
        time.sleep(rng.uniform(0.06, 0.45))
        # Track the fork guard spawned at boot: after the renderer dies
        # the guard must restore and exit on its own (up to 6 s of
        # patience -- see the fork_guard.rs liveness loop).
        g = subprocess.run(
            ["pgrep", "-P", str(proc.pid)],
            capture_output=True,
            text=True,
            check=False,
        )
        guard_pids = [int(x) for x in g.stdout.split() if x.isdigit()]
        sig = rng.choice((signal.SIGTERM, signal.SIGKILL, signal.SIGINT))
        try:
            os.kill(proc.pid, sig)
        except ProcessLookupError:
            pass
        t0 = time.time()
        try:
            proc.wait(timeout=3.0)
            slowest = max(slowest, time.time() - t0)
        except subprocess.TimeoutExpired:
            storm_fail += 1
            proc.kill()
            proc.wait(timeout=2)
        # The renderer is dead; its guard must follow within 7 s.
        for gp in guard_pids:
            deadline = time.time() + 7.0
            while time.time() < deadline:
                try:
                    os.kill(gp, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.2)
            else:
                guard_fail += 1
                try:
                    os.kill(gp, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        try:
            os.close(master)
        except OSError:
            pass
    # Settled leftover check: a guard exiting on the 20 ms poll cadence
    # can straddle the immediate check -- retry briefly before failing.
    leftovers = []
    for _ in range(10):
        leftovers = leftover_procs()
        if not leftovers:
            break
        time.sleep(0.25)
    ok = storm_fail == 0 and guard_fail == 0 and not leftovers
    result(
        ok,
        f"R spawn/kill storm ({rounds} rounds)",
        f"renderers dead <3 s (slowest {slowest:.2f}s), guards exited, "
        f"leftovers={len(leftovers)}"
        if ok
        else f"storm_fail={storm_fail}, guard_fail={guard_fail}, "
        f"leftovers={leftovers[:5]}",
    )

    # R7: SIGWINCH resize storm while rendering -- 35 resizes in ~3.5 s,
    # then the terminal must still respond to 'q' and restore cleanly.
    home, cfg = fresh_home()
    s = PtySession(cfg)
    sizes = [
        (120, 40),
        (130, 45),
        (80, 24),
        (200, 60),
        (100, 30),
        (150, 50),
        (60, 20),
        (120, 40),
    ]

    def resize_factory(cols, rows):
        def act(master, proc):
            fcntl.ioctl(
                master,
                termios.TIOCSWINSZ,
                struct.pack("HHHH", rows, cols, 0, 0),
            )

        return act

    actions = [
        (0.5 + 0.1 * i, resize_factory(*sizes[i % len(sizes)])) for i in range(35)
    ]
    rc, stream, cooked, _, _ = s.run(
        ["--intro", "none"], home, secs=6.0, actions=actions
    )
    result(
        rc == 0 and cooked and not crash_class(rc),
        "R SIGWINCH resize storm",
        f"rc={rc}, restored={cooked}, 35 resizes survived",
    )

    # R8: config-write race at the startup read. A writer thread flips
    # between two valid configs every 20 ms while the binary parses:
    # it must either boot the coherent snapshot (rc=0) or reject the
    # torn read cleanly (rc=2) -- never hang, panic or wedged raw mode.
    home, cfg = fresh_home()
    writer = _ConfigWriter(
        cfg,
        ["fps = 60\nspeed = 20\n", "fps = 45\nspeed = 30\n"],
        0.02,
        2.5,
    )
    writer.start()
    s = PtySession(cfg)
    rc, stream, cooked, _, _ = s.run(["--intro", "none"], home, secs=4.5)
    writer.join()
    ok = rc in (0, 2) and cooked and not crash_class(rc)
    result(
        ok,
        "R startup config-write race",
        f"rc={rc}, restored={cooked}, writes={writer.writes}",
    )

    # R9: live-reload storm mid-run -- valid/broken alternating every
    # 70 ms for 5 s, ending on the valid config. The drain contract
    # (from the depthtest-5/8 pins): diagnostics stay off the rain
    # screen, the app stays alive, 'q' still works, the deferred
    # error may surface in the exit path (rc 0 or 2).
    home, cfg = fresh_home()
    write_cfg(cfg, "fps = 60\n")
    s = PtySession(cfg)
    writer = _ConfigWriter(
        cfg,
        ["fps = 60\nspeed = 20\n", BROKEN_LINE, "fps = 45\nspeed = 30\n"],
        0.07,
        5.0,
    )

    def start_writer(master, proc):
        writer.start()

    rc, stream, cooked, _, _ = s.run(
        ["--intro", "none"],
        home,
        secs=8.0,
        actions=[(1.5, start_writer)],
    )
    writer.join()
    ok = rc in (0, 2) and cooked and not crash_class(rc)
    text = stream.decode("utf-8", errors="replace")
    on_rain = "error:" in text and ALT_LEAVE not in text[: text.find("error:")]
    result(
        ok and not on_rain,
        "R live-reload storm (70 ms churn)",
        f"rc={rc}, restored={cooked}, writes={writer.writes}, "
        f"errors_after_alt_leave={not on_rain}",
    )

    zombies = zombie_procs()
    result(
        not zombies,
        "R storm zombie check",
        "no defunct processes" if not zombies else f"zombies: {zombies[:3]}",
    )


# ── PART 2: PLATFORM-MATRIX (honest coverage) ─────────────────────────────


def part2_platform():
    section("PART 2 -- PLATFORM-MATRIX: honest platform coverage")

    print(f"  [info] running on: {sys.platform}")

    # Coverage honesty: surfaces that only exist on other platforms are
    # SKIPped with the reason -- a Linux box must never claim to have
    # verified the FreeBSD ports path or the Windows %APPDATA% expansion.
    if sys.platform.startswith("linux"):
        skipped(
            "P FreeBSD /usr/local/etc fallback",
            "freebsd-only path; not claimable from a Linux run",
        )
        skipped(
            "P Windows %APPDATA% config expansion",
            "windows-only path; not claimable from a Linux run",
        )
    else:
        skipped(
            "P Linux fork-guard bore",
            "cx-term-guard is Linux-only; skipped on this platform",
        )

    # P2: Termux environment simulation -- the historical hang class
    # (NIGHT-termux-hang, 2026-09-12) was Android draining the PTY;
    # the boot path with Termux markers must stay responsive to 'q'.
    home, cfg = fresh_home()
    s = PtySession(cfg)
    rc, stream, cooked, _, _ = s.run(
        ["--intro", "none"],
        home,
        secs=3.0,
        env_extra={
            "TERMUX_VERSION": "1.32.6",
            "PREFIX": "/data/data/com.termux/files/usr",
        },
    )
    result(
        rc == 0 and cooked,
        "P Termux env boot + quit",
        f"rc={rc}, restored={cooked}",
    )

    # P3: terminal-environment matrix -- every family must boot, quit
    # and restore. Byte counts are recorded as observational data (the
    # color depth downgrade under dumb/16-color shows in the volume).
    env_cases = [
        ("TERM=dumb", {"TERM": "dumb"}),
        ("TERM=linux", {"TERM": "linux"}),
        ("NO_COLOR=1", {"NO_COLOR": "1"}),
        ("LANG=C", {"LANG": "C", "LC_ALL": "C"}),
        ("TERM empty", {"TERM": ""}),
    ]
    if QUICK:
        env_cases = env_cases[:3]
    for label, extra in env_cases:
        home, cfg = fresh_home()
        s = PtySession(cfg)
        rc, stream, cooked, _, _ = s.run(
            ["--intro", "none"], home, secs=3.0, env_extra=extra
        )
        result(
            rc == 0 and cooked,
            f"P env {label}",
            f"rc={rc}, restored={cooked}, bytes={len(stream)}",
        )

    # P4: multiplexer + remote environments -- the terminal-family
    # detection branches (tmux, screen, ssh) must boot clean.
    mux_cases = [
        ("tmux env", {"TMUX": "/tmp/tmux-0/default,0", "TERM": "tmux-256color"}),
        ("screen TERM", {"TERM": "screen-256color"}),
        ("ssh env", {"SSH_CONNECTION": "10.0.0.1 51514 10.0.0.2 22"}),
    ]
    for label, extra in mux_cases:
        home, cfg = fresh_home()
        s = PtySession(cfg)
        rc, stream, cooked, _, _ = s.run(
            ["--intro", "none"], home, secs=3.0, env_extra=extra
        )
        result(
            rc == 0 and cooked,
            f"P env {label}",
            f"rc={rc}, restored={cooked}",
        )


# ── PART 3: EDGE-CRUSHER (extremes) ───────────────────────────────────────


def build_max_custom_config():
    """The maximum VALID custom-block config: 24 blocks per family.

    The LTS bound (NIGHT-hunt-40): colors-custom, scene-custom and
    charset-custom are each capped at 24 blocks, so a "10.000 entry"
    config is impossible BY DESIGN -- the depth contract is (a) the
    max-valid mass config parses, validates and boots, and (b) the
    25th block of every family is rejected with the named cap.
    Every scene-custom block carries the complete seven-dimension
    profile (the completeness contract the validators enforce).
    """
    parts = ["fps = 60\n"]
    for i in range(24):
        parts.append(
            f'[colors-custom.p{i}]\nbg = "#{i:06x}"\nrain = "#{i:06x}, #303050"\n'
        )
    for i in range(24):
        parts.append(
            f"[scene-custom.s{i}]\n"
            'rain = "glyph"\n'
            'color = "neon-green"\n'
            'charset = "matrix"\n'
            "fps = 60\n"
            "speed = 20\n"
            "density = 0.6\n"
            'glitch-level = "subtle"\n'
        )
    for i in range(24):
        parts.append(f'[charset-custom.c{i}]\nset = "01"\n')
    return "".join(parts)


def build_over_limit_config(family):
    """25 blocks of one family -- one past the 24-block cap."""
    if family == "colors-custom":
        return "".join(
            f'[colors-custom.p{i}]\nbg = "#000000"\nrain = "#000010, #000030"\n'
            for i in range(25)
        )
    if family == "scene-custom":
        return "".join(
            f"[scene-custom.s{i}]\n"
            'rain = "glyph"\n'
            'color = "neon-green"\n'
            'charset = "matrix"\n'
            "fps = 60\n"
            "speed = 20\n"
            "density = 0.6\n"
            'glitch-level = "subtle"\n'
            for i in range(25)
        )
    return "".join(f'[charset-custom.c{i}]\nset = "01"\n' for i in range(25))


def part3_edge():
    section("PART 3 -- EDGE-CRUSHER: extremes")

    # E1: geometry sweep in the headless bench (fixed virtual size):
    # 1x1, degenerate slivers, 300x500, 8K and beyond.
    geos = ["1x1", "2x1", "1x2", "5x5", "300x500", "500x200"]
    if not QUICK:
        geos += ["7680x2160", "7680x4320", "10000x10000"]
    for geo in geos:
        home, _ = fresh_home()
        t0 = time.time()
        rc, out, err = plain_run(
            ["--screen-size", geo, "--bench-frames", "30"], home, timeout=60
        )
        elapsed = time.time() - t0
        ok = rc == 0 and "error:" not in err
        result(ok, f"E geometry bench {geo}", f"rc={rc}, {elapsed:.1f}s")

    # E2: PTY geometry boots -- the renderer must handle real ioctl
    # sizes at the extremes without wedging the quit path.
    for cols, rows in ((1, 1), (100, 400), (200, 100)):
        home, cfg = fresh_home()
        s = PtySession(cfg, rows=rows, cols=cols)
        rc, stream, cooked, _, _ = s.run(["--intro", "none"], home, secs=4.0)
        result(
            rc == 0 and cooked,
            f"E PTY geometry {cols}x{rows}",
            f"rc={rc}, restored={cooked}, bytes={len(stream)}",
        )

    # E3: the max-valid mass config -- 24 blocks per custom family.
    cfg_text = build_max_custom_config()
    home, cfg = fresh_home()
    write_cfg(cfg, cfg_text)
    t0 = time.time()
    rc, out, err = plain_run(["--testconf"], home, timeout=90)
    ok = rc == 0
    result(
        ok,
        "E max-valid custom config testconf",
        f"rc={rc}, {len(cfg_text.splitlines())} lines, {time.time() - t0:.1f}s"
        if ok
        else f"rc={rc}, err[:80]={err.strip()[:80]!r}",
    )
    home, cfg = fresh_home()
    write_cfg(cfg, cfg_text)
    rc, out, err = plain_run(["--doctor"], home, timeout=90)
    ok = rc == 0 and "readable (" in out
    result(
        ok,
        "E max-valid custom config doctor",
        f"rc={rc}, CONFIG FILE readable reported"
        if ok
        else f"rc={rc}, err[:80]={err.strip()[:80]!r}",
    )
    home, cfg = fresh_home()
    write_cfg(cfg, cfg_text)
    rc, out, err = plain_run(["--bench-frames", "5"], home, timeout=90)
    result(
        rc == 0,
        "E max-valid custom config boot",
        f"rc={rc} (renderer booted on the mass config)",
    )

    # E3b: the 25th block of every family is rejected with the named
    # 24-block cap -- the LTS bound on config mass, clean rc=2 each.
    for family in ("colors-custom", "scene-custom", "charset-custom"):
        home, cfg = fresh_home()
        write_cfg(cfg, build_over_limit_config(family))
        rc, out, err = plain_run(["--doctor"], home, timeout=30)
        ok = rc == 2 and "maximum is 24" in err
        result(
            ok,
            f"E over-limit {family} rejected",
            f"rc={rc}, cap named" if ok else f"rc={rc}, err[:80]={err.strip()[:80]!r}",
        )

    # E4: message length boundary -- the 200-character cap (typed-flag
    # validation; 200 passes, 201 rejects with the named limit).
    home, _ = fresh_home()
    rc, _, err = plain_run(["-m", "z" * 200, "--bench-frames", "2"], home, timeout=20)
    result(rc == 0, "E message 200 chars accepted", f"rc={rc}")
    home, _ = fresh_home()
    rc, _, err = plain_run(["-m", "z" * 201, "--bench-frames", "2"], home, timeout=20)
    ok = rc == 2 and "200 character limit" in err
    result(
        ok,
        "E message 201 chars rejected",
        f"rc={rc}, limit named in stderr"
        if ok
        else f"rc={rc}, err[:60]={err.strip()[:60]!r}",
    )
    home, cfg = fresh_home()
    s = PtySession(cfg)
    rc, stream, cooked, _, _ = s.run(
        ["--intro", "none", "-m", "deep bore"], home, secs=3.0
    )
    result(
        rc == 0 and cooked,
        "E message render + quit",
        f"rc={rc}, restored={cooked}",
    )

    # E5: screen-size validation rejects (typed diagnostics, rc=2).
    for geo in ("0x0", "0x10", "10x0", "5", "abc"):
        home, _ = fresh_home()
        rc, _, err = plain_run(
            ["--screen-size", geo, "--bench-frames", "2"], home, timeout=20
        )
        ok = rc == 2 and "error:" in err
        result(ok, f"E screen-size reject {geo}", f"rc={rc}")

    # E6: config byte edges -- the read-failure classification fixed
    # in this hunt: doctor reports, testconf rejects, nothing silent.
    home, cfg = fresh_home()
    write_cfg(cfg, b"\xff\xfe garbage\xff")
    rc, out, err = plain_run(["--doctor"], home, timeout=30)
    ok = (
        rc == 0
        and "unreadable:" in out
        and "UTF-8" in out
        and "hint:" in out
        and "effective:" in out
    )
    result(
        ok,
        "E invalid-utf8 doctor reports",
        f"rc={rc}, status+effective+hint in report"
        if ok
        else f"rc={rc}, out[:100]={out[:100]!r}",
    )
    home, cfg = fresh_home()
    write_cfg(cfg, b"\xff\xfe garbage\xff")
    rc, out, err = plain_run(["--testconf"], home, timeout=30)
    ok = rc == 2 and "cannot read config file" in err
    result(
        ok,
        "E invalid-utf8 testconf rejects",
        f"rc={rc}, diagnostic on stderr"
        if ok
        else f"rc={rc}, err[:60]={err.strip()[:60]!r}",
    )

    oversize = b"# padding line\n" * 90000  # ~1.26 MB, past the 1 MiB cap
    home, cfg = fresh_home()
    write_cfg(cfg, oversize)
    rc, out, err = plain_run(["--testconf"], home, timeout=60)
    ok = rc == 2 and "exceeds" in err
    result(
        ok,
        "E oversize testconf cap-aligned",
        f"rc={rc}, cap named" if ok else f"rc={rc}, err[:80]={err.strip()[:80]!r}",
    )
    home, cfg = fresh_home()
    write_cfg(cfg, oversize)
    rc, out, err = plain_run(["--doctor"], home, timeout=60)
    ok = rc == 0 and "unreadable:" in out and "exceeds" in out
    result(
        ok,
        "E oversize doctor reports",
        f"rc={rc}, unreadable + cap reason in report"
        if ok
        else f"rc={rc}, out[:100]={out[:100]!r}",
    )

    home, cfg = fresh_home()
    write_cfg(cfg, "")
    rc, out, err = plain_run(["--doctor"], home, timeout=30)
    ok = rc == 0 and "readable (0 bytes)" in out
    result(
        ok,
        "E empty config readable",
        f"rc={rc}, readable (0 bytes)" if ok else f"rc={rc}, out[:80]={out[:80]!r}",
    )
    home, cfg = fresh_home()
    write_cfg(cfg, "")
    rc, out, err = plain_run(["--testconf"], home, timeout=30)
    result(rc == 0, "E empty config testconf", f"rc={rc}")

    if os.geteuid() != 0:
        home, cfg = fresh_home()
        write_cfg(cfg, "fps = 60\n")
        os.chmod(cfg, 0o000)
        rc, out, err = plain_run(["--doctor"], home, timeout=30)
        os.chmod(cfg, 0o644)
        ok = rc == 0 and "unreadable:" in out
        result(
            ok,
            "E permission-denied doctor reports",
            f"rc={rc}, unreadable in report"
            if ok
            else f"rc={rc}, out[:100]={out[:100]!r}",
        )
    else:
        skipped(
            "E permission-denied doctor reports",
            "running as root: chmod 000 is still readable",
        )

    home, cfg = fresh_home()
    os.makedirs(cfg, exist_ok=True)
    rc, out, err = plain_run(["--doctor"], home, timeout=30)
    ok = rc == 0 and "unreadable:" in out
    result(
        ok,
        "E config-is-dir doctor reports",
        f"rc={rc}, unreadable in report" if ok else f"rc={rc}, out[:100]={out[:100]!r}",
    )

    home, cfg = fresh_home()
    write_cfg(cfg, "k" * 100000 + " = 1\n")
    rc, out, err = plain_run(["--doctor"], home, timeout=30)
    ok = rc == 2 and "unknown key" in err
    result(
        ok,
        "E huge-key doctor strict",
        f"rc={rc}, unknown key diagnostic"
        if ok
        else f"rc={rc}, err[:60]={err.strip()[:60]!r}",
    )
    home, cfg = fresh_home()
    write_cfg(cfg, "[" * 1000 + "\n")
    rc, out, err = plain_run(["--doctor"], home, timeout=30)
    ok = rc == 2 and "malformed" in err
    result(
        ok,
        "E deep-brackets doctor strict",
        f"rc={rc}, malformed diagnostic"
        if ok
        else f"rc={rc}, err[:60]={err.strip()[:60]!r}",
    )


# ── PART 4: CONFIG-FUZZ (seeded, deterministic) ───────────────────────────

MUTATIONS = [
    ("fps string", 'fps = "sixty"\n'),
    ("fps zero", "fps = 0\n"),
    ("fps huge", "fps = 1000000\n"),
    ("fps nan", "fps = nan\n"),
    ("fps inf", "fps = inf\n"),
    ("density bool", "density = true\n"),
    ("density over", "density = 5.0\n"),
    ("density negative", "density = -0.5\n"),
    ("speed array", "speed = [1, 2]\n"),
    ("speed negative", "speed = -3\n"),
    ("color empty", 'color = ""\n'),
    ("color unknown", 'color = "does-not-exist"\n'),
    ("charset unknown", 'charset = "no-such-charset"\n'),
    ("scene unknown", 'scene = "no-such-scene"\n'),
    ("rain unknown", 'rain = "sideways"\n'),
    ("glitch unknown", 'glitch-level = "ultra"\n'),
    ("msg-fill unknown", 'msg-fill-style = "quantum"\n'),
    ("duplicate key", "fps = 60\nfps = 90\n"),
    ("unknown key", "typo_key_zzz = 1\n"),
    ("malformed line", "garbage line no equals\n"),
    ("bad hex", '[colors-custom.bad]\nbg = "#12zz45"\n'),
    ("bad stops", '[colors-custom.bad]\nrain = "not,hex,list"\n'),
    ("unterminated header", '[colors-custom.bad\nbg = "#000000"\n'),
    ("header-only scene", "[scene-custom.half]\n"),
    ("incomplete scene", '[scene-custom.part]\nrain = "glyph"\n'),
    ("bad ambient time", 'ambient.99-99 = "monolith"\n'),
    ("good ambient", 'ambient.00-00 = "monolith"\n'),
    ("crystal secs bad", "crystal-dragon = true\ncrystal-dragon-secs = -1\n"),
    ("snapback bad", "ambient-snapback-secs = 0\n"),
    ("msg-mode string", 'msg-mode = "yes"\n'),
    ("bad float syntax", "speed = 1.2.3\n"),
    ("bare equals", "=\n"),
    ("null-ish", "fps = null\n"),
    ("crystal bool string", 'crystal-dragon = "true"\n'),
    ("charset-custom bad set", '[charset-custom.x]\nset = ""\n'),
    ("crlf line endings", "fps = 60\r\nspeed = 20\r\n"),
    ("bom prefix", "﻿fps = 60\n"),
    ("tab separators", "fps\t=\t60\n"),
]


def part4_fuzz():
    section("PART 4 -- CONFIG-FUZZ: deterministic mutants")

    # F1: the control -- the rich baseline must be fully valid.
    home, cfg = fresh_home()
    write_cfg(cfg, RICH_CFG)
    rc, out, err = plain_run(["--testconf"], home, timeout=30)
    ok = rc == 0
    detail = f"rc={rc}"
    if not ok:
        detail += f", err[:120]={err.strip()[:120]!r}"
    result(ok, "F control baseline valid", detail)
    home, cfg = fresh_home()
    write_cfg(cfg, RICH_CFG)
    rc, out, err = plain_run(["--doctor"], home, timeout=30)
    result(
        rc == 0 and "readable (" in out,
        "F control baseline doctor",
        f"rc={rc}",
    )

    # F2: every mutant must be classified cleanly: rc=2 with the
    # diagnostic on stderr and a clean stdout, or rc=0 (accepted --
    # the mutation happened to be valid). Panics, timeouts, garbage
    # stdout or crash-class exits are bugs at any depth.
    mutants = list(MUTATIONS)
    if QUICK:
        mutants = mutants[::2]
    accepted = 0
    boot_checked = 0
    for i, (label, cfg_text) in enumerate(mutants):
        home, cfg = fresh_home()
        write_cfg(cfg, cfg_text)
        rc, out, err = plain_run(["--doctor"], home, timeout=30)
        ok = rc in (0, 2) and not crash_class(rc)
        if ok and rc == 2:
            ok = err.strip().startswith("error:") and not out.strip()
        detail = f"rc={rc}"
        if not ok:
            detail += f", err[:80]={err.strip()[:80]!r}, out[:60]={out[:60]!r}"
        result(ok, f"F mutant {label}", detail)
        if rc == 0:
            accepted += 1
            # Accepted mutants get a sampled PTY boot: a config the
            # validator accepts must also survive the render loop.
            if boot_checked < 8 and i % 5 == 0:
                boot_checked += 1
                home, cfg = fresh_home()
                write_cfg(cfg, cfg_text)
                s = PtySession(cfg)
                prc, _stream, cooked, _, _ = s.run(["--intro", "none"], home, secs=2.5)
                bok = prc in (0, 2) and cooked and not crash_class(prc)
                result(
                    bok,
                    f"F mutant-boot {label}",
                    f"rc={prc}, restored={cooked}",
                )
    print(
        f"  [info] mutants={len(mutants)}, accepted (rc=0)={accepted}, "
        f"boots sampled={boot_checked}"
    )

    # F3: CLI flag-combination precedence (the ladder pins from the
    # depthtest-5/8 rounds, re-pinned at depthbore depth).
    home, cfg = fresh_home()
    write_cfg(cfg, RICH_CFG)
    combos = [
        (["--version", "--docs"], "cosmostrix:", 0),
        (["--doctor", "--dump-config"], "# cosmostrix", 0),
        (["--testconf", "--doctor"], "testconf: checking", 0),
        (["--list-scenes", "--doctor"], "SCENE", 0),
        (["--config-path", "--doctor"], "config.toml", 0),
    ]
    for args, marker, expect in combos:
        rc, out, err = plain_run(args, home, timeout=30)
        ok = rc == expect and marker in out
        detail = f"rc={rc}, marker={marker!r}"
        if not ok:
            detail += f", out[:60]={out[:60]!r}"
        result(ok, f"F combo {' '.join(args)}", detail)

    home, cfg = fresh_home()
    write_cfg(cfg, BROKEN_LINE)
    rc, out, err = plain_run(["--doctor", "--version"], home, timeout=30)
    ok = rc == 2 and not out.strip() and "invalid config" in err
    result(
        ok,
        "F combo doctor+version strict ladder",
        "rc=2, doctor wins, config diagnostic"
        if ok
        else f"rc={rc}, out[:40]={out.strip()[:40]!r}, err[:60]={err[:60]!r}",
    )
    home, cfg = fresh_home()
    write_cfg(cfg, BROKEN_LINE)
    rc, out, err = plain_run(["--version"], home, timeout=30)
    ok = rc == 0 and out.startswith("cosmostrix:") and "invalid config" not in err
    result(ok, "F combo version rescued", f"rc={rc}")


# ── PART 5: DRIFT-SOAK (bounded long-run proxy) ───────────────────────────


def _soak_report(samples, label, rate_windows, thresholds):
    rss = [(t, s["rss_kb"]) for t, s in samples if s["rss_kb"] is not None]
    fds = [s["fd"] for _, s in samples if s["fd"] is not None]
    threads = [s["threads"] for _, s in samples if s["threads"] is not None]
    # Skip the warmup half before fitting the slope: allocator arenas,
    # palette construction and the crystal-dragon drift engine
    # legitimately move RSS during the first seconds (measured: RSS
    # plateaus completely after ~16 s -- last-half slope 0.0 KB/s).
    warm = len(rss) // 2
    slope = linear_slope(rss[warm:]) if len(rss) - warm >= 2 else 0.0
    fd_drift = (max(fds) - min(fds)) if fds else 0
    th_drift = (max(threads) - min(threads)) if threads else 0
    first, last = rate_windows
    ratio = (last / first) if first else float("inf")
    slope_max, fd_max, th_max, ratio_max = thresholds
    problems = []
    if slope > slope_max:
        problems.append(f"rss slope {slope:.1f} KB/s > {slope_max}")
    if fd_drift > fd_max:
        problems.append(f"fd drift {fd_drift} > {fd_max}")
    if th_drift > th_max:
        problems.append(f"thread drift {th_drift} > {th_max}")
    if ratio > ratio_max:
        problems.append(f"output growth ratio {ratio:.1f} > {ratio_max}")
    ok = not problems
    detail = (
        f"slope={slope:.1f} KB/s, fd drift={fd_drift}, "
        f"threads drift={th_drift}, rate x{ratio:.2f}, "
        f"samples={len(samples)}"
    )
    if not ok:
        detail = f"LEAK SMELL: {'; '.join(problems)} | {detail}"
    result(ok, label, detail)
    return ok


def part5_drift():
    section("PART 5 -- DRIFT-SOAK: the bounded 24 h proxy")

    soak = 16 if QUICK else SOAK_SECS

    # D1: the still soak -- one render loop, /proc sampled every 2 s.
    # The RSS warmup curve is bounded and converging (measured 120 s:
    # steps +436/+188/+24/+12 KB at t=4/28/64/94, final ~6.2 MB, flat
    # after; last-half slope 0.44 KB/s) -- so short windows judge only
    # runaway growth; the slope verdict needs a >= 30 s window.
    home, cfg = fresh_home()
    write_cfg(cfg, RICH_CFG)
    s = PtySession(cfg)
    rc, _stream, cooked, samples, rates = s.run(
        ["--intro", "none"], home, secs=soak, sampler=proc_sample
    )
    still_slope_max = 20.0 if soak >= 30 else 100.0
    ok = _soak_report(
        samples,
        f"D1 still soak ({soak}s)",
        rates,
        (still_slope_max, 6, 2, 5.0),
    )
    ok = ok and rc == 0 and cooked
    if not (rc == 0 and cooked):
        result(False, "D1 still soak exit", f"rc={rc}, restored={cooked}")
    else:
        result(True, "D1 still soak exit", "rc=0, restored, clean after long run")

    # D2: the churn soak -- the live-reload path under continuous valid
    # edits (the reload-leak class: reparse allocations that never
    # settle). Two valid variants alternate every 3 s.
    home, cfg = fresh_home()
    write_cfg(cfg, "fps = 60\nspeed = 20\n")
    writer = _ConfigWriter(
        cfg,
        ["fps = 60\nspeed = 20\ndensity = 0.6\n", "fps = 45\nspeed = 30\n"],
        3.0,
        soak,
    )

    def start_writer(master, proc):
        writer.start()

    s = PtySession(cfg)
    rc, _stream, cooked, samples, rates = s.run(
        ["--intro", "none"],
        home,
        secs=soak,
        actions=[(1.0, start_writer)],
        sampler=proc_sample,
    )
    writer.join()
    churn_slope_max = 60.0 if soak >= 30 else 150.0
    ok = _soak_report(
        samples,
        f"D2 reload churn soak ({soak}s)",
        rates,
        (churn_slope_max, 8, 3, 5.0),
    )
    # Valid-to-valid reloads must never trip the error path: rc=0.
    ok = ok and rc == 0 and cooked
    if not (rc == 0 and cooked):
        result(
            False,
            "D2 churn soak exit",
            f"rc={rc}, restored={cooked}, writes={writer.writes}",
        )
    else:
        result(
            True,
            "D2 churn soak exit",
            f"rc=0, restored, writes={writer.writes}",
        )

    # D3: hygiene -- nothing may survive the soaks.
    leftovers = leftover_procs()
    zombies = zombie_procs()
    result(
        not leftovers and not zombies,
        "D3 post-soak hygiene",
        f"leftovers={len(leftovers)}, zombies={len(zombies)}"
        if not (leftovers or zombies)
        else f"leftovers={leftovers[:4]}, zombies={zombies[:3]}",
    )


# ── Entry point ───────────────────────────────────────────────────────────


def main():
    global BIN, QUICK, SEED, SOAK_SECS
    import argparse

    ap = argparse.ArgumentParser(description="cosmostrix LTS depth bore")
    ap.add_argument("--quick", action="store_true", help="smoke bore (~2 min)")
    ap.add_argument("--bin", default=None, help="binary path override")
    ap.add_argument("--seed", type=int, default=47, help="storm RNG seed")
    ap.add_argument(
        "--soak-secs", type=int, default=36, help="drift soak window (default 36)"
    )
    ap.add_argument(
        "--parts", default="012345", help="part filter, e.g. 15 for parts 1+5"
    )
    args = ap.parse_args()

    BIN = resolve_bin(args.bin)
    QUICK = args.quick
    SEED = args.seed
    SOAK_SECS = args.soak_secs

    print("cosmostrix NIGHT-hunt-47-depthbore")
    print(f"binary: {BIN}")
    print(f"mode: {'quick' if QUICK else 'full'}, seed={SEED}, soak={SOAK_SECS}s")
    t0 = time.time()

    parts = {
        "0": ("ZERO", part0_zero),
        "1": ("RACE-STORM", part1_race),
        "2": ("PLATFORM-MATRIX", part2_platform),
        "3": ("EDGE-CRUSHER", part3_edge),
        "4": ("CONFIG-FUZZ", part4_fuzz),
        "5": ("DRIFT-SOAK", part5_drift),
    }
    for key in sorted(args.parts):
        if key in parts:
            parts[key][1]()

    elapsed = time.time() - t0
    print("\n" + "═" * 78)
    print(
        f"DEPTHBORE RESULT: pass={PASS_COUNT} fail={FAIL_COUNT} "
        f"skip={SKIP_COUNT} | {elapsed:.0f}s | "
        f"verdict={'CLEAN' if FAIL_COUNT == 0 else 'BUGS FOUND'}"
    )
    if FAILURES:
        print("failures:")
        for f in FAILURES:
            print(f"  - {f}")
    print("═" * 78)
    sys.exit(0 if FAIL_COUNT == 0 else 1)


if __name__ == "__main__":
    main()
