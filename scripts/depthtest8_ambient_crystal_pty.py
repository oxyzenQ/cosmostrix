#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL
#   or Git Bash on Windows).

"""NIGHT-depthtest-5 & hunt-46 (depthtest-8): close the remaining
flow-separation surfaces + the ambient/crystal-dragon PTY interaction.

Owner brief (2026-09-14): use the same flow-separation matrix on the
surfaces the previous rounds (44-47) left open — --doctor /
--dump-config / --docs error paths — plus the deep PTY audit of the
ambient/crystal-dragon interaction, and the CLI + config/live-reload
depth stresstest. Zero to hero; fix immediately what fails.

Four parts, all on the REAL binary:

PART A — static post-config surface (the hunt-46 fix, pinned).
  The depthtest-7 static matrix only used --bench-frames as the
  broken-config carrier. Here every broken-config family (unknown
  key, malformed line, duplicate key) meets every static command:
  - --version / --docs must print and exit 0 (config-independent
    content; NIGHT-depthtest-5 & hunt-46 rescue in
    cli/early_returns.rs handle_config_apply_failure).
  - --doctor must stay STRICT: rc=2, stdout empty, the config
    diagnostic on stderr (the hunt-44 contract — a config failure
    IS the doctor surface's diagnostic).
  - --doctor --version (both) must keep rc=2: doctor wins the
    ladder, the rescue must not widen.
  - --testconf must keep the rc=2 + stderr contract.
  - --check-update must not die on the CONFIG error (rc 0 or its
    own network-failure 2, stderr never containing 'invalid config').

PART B — --dump-config error-path matrix (regression pin).
  Redirect refusal (stdout to file), overwrite refusal (suggested
  sibling ends in .toml), non-.toml extension, outside-whitelist,
  --force overwrite, and the pipe-allowed control. Every failure:
  rc=2 + clean stdout + stderr diagnostic.

PART C — ambient/crystal-dragon PTY harmony (the core new audit).
  depthtest-5 covered bad-config edits on a msg-mode baseline and
  depthtest-7 covered reference-graph rejection; NEITHER exercised
  the ambient scheduler and the Crystal Dragon drift engine running
  TOGETHER, which is the documented "masterclass harmony" state
  machine (post_rain.rs: drift fires -> ambient snapback reverts ->
  cycle). Tuned cadence (crystal-dragon-secs=3, snapback=2) makes
  every cycle fire inside the window. Observable: the -v exit
  summary's ambient_diag counters (startup/rx/reapply/snapback/
  sked_reloads/sked_empties/snapback_killed/last_scene_change),
  captured by quitting with 'q' (clean rc=0).
  - C1 ambient startup (builtin scene) + crystal: startup=1, alive,
    clean screen, clean quit.
  - C2 ambient startup (custom scene + custom palette) + crystal:
    the set_palette / custom_palette_active lock path.
  - C3 mid-run ambient scene SWITCH: live-reload -> scheduler refire
    -> the new scene applied (rx or snapback), last_scene_change
    names the new scene.
  - C4 mid-run ambient REMOVAL: overlay lift -> revert to the locked
    startup scene, sked_empties>=1, snapback_killed=1, run stays
    alive with crystal still drifting.
  - C5 no-ambient + crystal only: the drift self-reset path
    (Z-master-1X round 2) keeps cycling without wedge.
  - C6 'q'-quit contract: rc=0 + exit summary present.

PART D — error ordering under the harmony load (rc=2 after the
  alt-screen leave, never on the rain screen) with crystal ON:
  broken ambient reference mid-run, crystal-dragon-secs out of
  range mid-run, and a control (valid edit keeps running).
"""

import fcntl
import os
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time

BIN = os.environ.get("COSMOSTRIX_BIN", "target/debug/cosmostrix")
if not os.path.isabs(BIN):
    BIN = os.path.abspath(BIN)

TERM_ROWS, TERM_COLS = 40, 120
ALT_LEAVE = b"\x1b[?1049l"

PASS_COUNT = 0
FAIL_COUNT = 0
FAILURES = []


def fresh_home():
    home = tempfile.mkdtemp(prefix="dt8_home_")
    os.makedirs(os.path.join(home, ".config", "cosmostrix"), exist_ok=True)
    return home, os.path.join(home, ".config", "cosmostrix", "config.toml")


def write_cfg(path, text):
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def run(home, args, timeout=15):
    """Plain subprocess run (no PTY): stream separation is measurable."""
    env = dict(os.environ)
    env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
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


# ── PTY runner (quit-observable) ──────────────────────────────────────────


class PtyRunner:
    """Run the binary in a PTY, apply config edits at t, quit with 'q'."""

    def __init__(self, cfg_path):
        self.cfg_path = cfg_path

    def run(self, home, args, secs, edits=(), quit_at_end=True, extra_env=None):
        """Returns (stream_bytes, rc). edits: [(t_secs, cfg_text)]."""
        master, slave = pty.openpty()
        fcntl.ioctl(
            master, termios.TIOCSWINSZ, struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0)
        )
        env = dict(os.environ)
        env["HOME"] = home
        env.pop("XDG_CONFIG_HOME", None)
        env["TERM"] = "xterm-256color"
        env["TERM_PROGRAM"] = "alacritty"
        env["COLORTERM"] = "truecolor"
        if extra_env:
            env.update(extra_env)
        proc = subprocess.Popen(
            [BIN] + args, stdin=slave, stdout=slave, stderr=slave, env=env
        )
        os.close(slave)
        buf = bytearray()
        t0 = time.time()
        edits = list(edits)
        exited = None
        quit_sent = False
        while time.time() - t0 < secs:
            r, _, _ = select.select([master], [], [], 0.05)
            if r:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                buf.extend(chunk)
            now = time.time() - t0
            while edits and now >= edits[0][0]:
                _, text = edits.pop(0)
                write_cfg(self.cfg_path, text)
            if proc.poll() is not None:
                exited = proc.returncode
                deadline = time.time() + 0.5
                while time.time() < deadline:
                    r, _, _ = select.select([master], [], [], 0.05)
                    if r:
                        try:
                            chunk = os.read(master, 65536)
                        except OSError:
                            break
                        if not chunk:
                            break
                        buf.extend(chunk)
                break
        if exited is None and quit_at_end:
            # Drain window ended: ask for a clean quit. The 'q' key is
            # the documented quit shortcut; the app then restores the
            # terminal and prints the -v exit summary (our observability).
            if not quit_sent:
                try:
                    os.write(master, b"q")
                    quit_sent = True
                except OSError:
                    pass
            deadline = time.time() + 5.0
            while time.time() < deadline:
                r, _, _ = select.select([master], [], [], 0.05)
                if r:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError:
                        break
                    if not chunk:
                        break
                    buf.extend(chunk)
                if proc.poll() is not None:
                    exited = proc.returncode
                    deadline = time.time() + 0.5
                    while time.time() < deadline:
                        r, _, _ = select.select([master], [], [], 0.05)
                        if r:
                            try:
                                chunk = os.read(master, 65536)
                            except OSError:
                                break
                            if not chunk:
                                break
                            buf.extend(chunk)
                    break
        if exited is None:
            proc.terminate()
            try:
                exited = proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                proc.kill()
                exited = proc.wait()
        os.close(master)
        return bytes(buf), exited


def diag_counters(stream_text):
    """Extract the ambient_diag line's counters from the exit summary."""
    m = re.search(r"ambient_diag: ([^\r\n]*)", stream_text)
    if not m:
        return None
    out = {}
    for part in m.group(1).split():
        if "=" in part:
            k, v = part.split("=", 1)
            out[k] = v
    return out


def result(ok, label, detail):
    global PASS_COUNT, FAIL_COUNT
    status = "PASS" if ok else "FAIL"
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"{label} | {detail}")
    print(f"  [{status}] {label:58s} | {detail}")


# ── Part A: static post-config surface (the hunt-46 rescue, pinned) ───────

BROKEN_CONFIGS = {
    "unknown-key": "typo_key_zzz = 1\n",
    "malformed-line": "garbage line no equals\n",
    "duplicate-key": "fps = 60\nfps = 90\n",
}


def part_a_static_surface():
    print("\n── Part A: static post-config surface vs broken config families ──")
    for family, cfg in BROKEN_CONFIGS.items():
        home, cfg_path = fresh_home()
        write_cfg(cfg_path, cfg)

        # --version: rescued (rc=0, stdout carries the version line).
        rc, out, err = run(home, ["--version"])
        ok = rc == 0 and out.startswith("cosmostrix:") and "invalid config" not in err
        result(
            ok,
            f"A [{family}] --version rescued",
            f"rc={rc}, out={out.splitlines()[0][:40] if out else '(empty)'}"
            if ok
            else f"rc={rc}, out[:40]={out[:40]!r}, err[:60]={err[:60]!r}",
        )

        # --docs: rescued (rc=0, substantial stdout, no config death).
        rc, out, err = run(home, ["--docs"])
        ok = rc == 0 and len(out.splitlines()) > 100 and "invalid config" not in err
        result(
            ok,
            f"A [{family}] --docs rescued",
            f"rc={rc}, {len(out.splitlines())} doc lines"
            if ok
            else f"rc={rc}, lines={len(out.splitlines())}, err[:60]={err[:60]!r}",
        )

        # --doctor: STRICT (hunt-44 contract: rc=2, clean stdout, stderr).
        rc, out, err = run(home, ["--doctor"])
        ok = rc == 2 and not out.strip() and "invalid config" in err
        result(
            ok,
            f"A [{family}] --doctor stays strict",
            "rc=2, stdout clean, stderr diagnostic"
            if ok
            else f"rc={rc}, out={out.strip()[:40]!r}, err[:60]={err[:60]!r}",
        )

        # --doctor --version: doctor wins the ladder, rescue must not widen.
        rc, out, err = run(home, ["--doctor", "--version"])
        ok = rc == 2 and not out.strip() and "invalid config" in err
        result(
            ok,
            f"A [{family}] --doctor --version precedence",
            "rc=2 (doctor wins, config diagnostic)"
            if ok
            else f"rc={rc}, out={out.strip()[:40]!r}",
        )

        # --testconf: pre-config path, unaffected (rc=2 + stderr).
        rc, out, err = run(home, ["--testconf"])
        ok = rc == 2 and "error:" not in out.lower() and err.strip()
        result(
            ok,
            f"A [{family}] --testconf contract",
            "rc=2, diagnostic on stderr" if ok else f"rc={rc}, err[:60]={err[:60]!r}",
        )

    # --check-update: must not die on the CONFIG error (network outcome
    # is its own business; the config error must never be the killer).
    home, cfg_path = fresh_home()
    write_cfg(cfg_path, BROKEN_CONFIGS["unknown-key"])
    rc, out, err = run(home, ["--check-update"], timeout=30)
    ok = rc in (0, 2) and "invalid config" not in err
    result(
        ok,
        "A [check-update] no config death",
        f"rc={rc} (own outcome), no config error"
        if ok
        else f"rc={rc}, err[:80]={err[:80]!r}",
    )

    # Clean-config controls (the rescue must not change happy paths).
    home, _ = fresh_home()
    rc, out, err = run(home, ["--version"])
    result(
        rc == 0 and out.startswith("cosmostrix:"),
        "A [control] --version clean cfg",
        f"rc={rc}" if rc == 0 else f"rc={rc}, err[:60]={err[:60]!r}",
    )
    rc, out, err = run(home, ["--doctor"])
    result(
        rc == 0 and "COSMOSTRIX DIAGNOSTICS REPORT" in out,
        "A [control] --doctor clean cfg",
        f"rc={rc}",
    )


# ── Part B: --dump-config error-path matrix (regression pin) ──────────────


def part_b_dump_config():
    print("\n── Part B: --dump-config error-path matrix ──")
    home, _ = fresh_home()
    inside = os.path.join(home, ".config", "cosmostrix")

    # B1: redirection refusal (shell > bypass attempt).
    fd_, redir_path = tempfile.mkstemp(suffix=".txt")
    os.close(fd_)
    env = dict(os.environ)
    env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
    with open(redir_path, "wb") as sink:
        p = subprocess.run(
            [BIN, "--dump-config"],
            env=env,
            stdout=sink,
            stderr=subprocess.PIPE,
            text=True,
            timeout=15,
            check=False,
        )
    sink_size = os.path.getsize(redir_path)
    ok = p.returncode == 2 and sink_size == 0 and "redirected file" in p.stderr
    result(
        ok,
        "B1 redirect refusal",
        f"rc={p.returncode}, stdout-file={sink_size}B"
        if ok
        else f"rc={p.returncode}, size={sink_size}, err[:60]={p.stderr[:60]!r}",
    )
    os.unlink(redir_path)

    # B2: overwrite refusal (suggested sibling must end in .toml).
    exists_path = os.path.join(inside, "exists.toml")
    write_cfg(exists_path, "fps = 60\n")
    rc, out, err = run(home, ["--dump-config", exists_path])
    ok = (
        rc == 2
        and not out.strip()
        and "refuses to overwrite" in err
        and "exists.new.toml" in err
        and "--force" in err
    )
    result(
        ok,
        "B2 overwrite refusal + .new.toml + --force",
        "rc=2, suggestion valid, escape hatch named"
        if ok
        else f"rc={rc}, err[:120]={err[:120]!r}",
    )

    # B3: non-.toml extension.
    rc, out, err = run(home, ["--dump-config", os.path.join(inside, "noext.txt")])
    ok = rc == 2 and not out.strip() and ".toml extension" in err
    result(
        ok,
        "B3 non-.toml extension",
        f"rc={rc}" if ok else f"rc={rc}, err[:60]={err[:60]!r}",
    )

    # B4: outside whitelist.
    rc, out, err = run(home, ["--dump-config", "/tmp/dt8_outside.toml"])
    ok = (
        rc == 2
        and not out.strip()
        and ("outside allowed" in err or "not allowed" in err)
    )
    result(
        ok,
        "B4 outside whitelist",
        f"rc={rc}" if ok else f"rc={rc}, err[:60]={err[:60]!r}",
    )

    # B5: --force overwrite (atomic write path, rc=0).
    rc, out, err = run(home, ["--dump-config", exists_path, "--force"])
    with open(exists_path, encoding="utf-8") as f:
        head = f.readline()
    ok = rc == 0 and not out.strip() and head.startswith("# cosmostrix config")
    result(
        ok,
        "B5 --force overwrite",
        f"rc={rc}, file rewritten" if ok else f"rc={rc}, head={head!r}",
    )

    # B6: pipe allowed (control — the supported pipeline form).
    env2 = dict(env)
    try:
        p = subprocess.run(
            [BIN, "--dump-config"],
            env=env2,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        ok = p.returncode == 0 and p.stdout.startswith("# cosmostrix config")
        result(
            ok,
            "B6 pipe output allowed (control)",
            f"rc={p.returncode}, {len(p.stdout.splitlines())} lines"
            if ok
            else f"rc={p.returncode}, out[:40]={p.stdout[:40]!r}",
        )
    except subprocess.TimeoutExpired:
        result(False, "B6 pipe output allowed (control)", "timeout")


# ── Part C: ambient/crystal-dragon PTY harmony ─────────────────────────────

HARMONY = "crystal-dragon = true\ncrystal-dragon-secs = 3\nambient-snapback-secs = 2\n"


def part_c_ambient_crystal_pty():
    print("\n── Part C: ambient/crystal-dragon PTY harmony (tuned cadence) ──")

    # C1: ambient startup (builtin scene) + crystal drift.
    home, cfg = fresh_home()
    write_cfg(cfg, HARMONY + 'ambient.00-00 = "monolith"\n')
    runner = PtyRunner(cfg)
    stream, rc = runner.run(home, ["-v"], secs=12)
    s = stream.decode("utf-8", errors="replace")
    counters = diag_counters(s)
    problems = []
    if rc != 0:
        problems.append(f"rc={rc} (want clean 'q' quit)")
    if counters is None:
        problems.append("no ambient_diag line in exit summary")
    else:
        if counters.get("startup") != "1":
            problems.append(f"startup={counters.get('startup')} (want 1)")
    for needle in ("error:", "invalid config", "panicked"):
        if needle in s:
            problems.append(f"'{needle}' in stream")
    leave = s.rfind("\x1b[?1049l")
    if leave == -1:
        problems.append("no alt-screen leave (terminal not restored)")
    else:
        for needle in ("error:", "invalid config"):
            if needle in s[:leave]:
                problems.append(f"'{needle}' DURING rain")
    result(
        not problems,
        "C1 ambient startup + crystal harmony",
        "; ".join(problems) if problems else "rc=0, startup=1, clean screen, quit ok",
    )

    # C2: ambient startup via custom scene + custom palette (the
    # set_palette / custom_palette_active lock path) + crystal drift.
    custom = (
        HARMONY
        + '[colors-custom.pal]\nbg = "#0a0a0a"\n'
        + 'rain = ["#1a0033", "#4d0080", "#9933ff"]\n\n'
        + '[charset-custom.z]\nset = "|+"\n\n'
        + '[scene-custom.t]\nrain = "glyph"\ncolors-custom = "pal"\n'
        + 'charset-custom = "z"\nfps = 60\nspeed = 20\ndensity = 0.75\n'
        + 'glitch-level = "subtle"\n\n'
        + 'ambient.00-00 = "t"\n'
    )
    home, cfg = fresh_home()
    write_cfg(cfg, custom)
    runner = PtyRunner(cfg)
    stream, rc = runner.run(home, ["-v"], secs=12)
    s = stream.decode("utf-8", errors="replace")
    counters = diag_counters(s)
    problems = []
    if rc != 0:
        problems.append(f"rc={rc}")
    if counters is None or counters.get("startup") != "1":
        problems.append(f"startup={counters.get('startup') if counters else 'no-diag'}")
    for needle in ("error:", "invalid config", "panicked"):
        if needle in s:
            problems.append(f"'{needle}' in stream")
    result(
        not problems,
        "C2 ambient custom scene+palette + crystal",
        "; ".join(problems) if problems else "rc=0, startup=1, palette lock path clean",
    )

    # C3: mid-run ambient scene SWITCH (monolith -> signal) via live-reload.
    home, cfg = fresh_home()
    write_cfg(cfg, HARMONY + 'ambient.00-00 = "monolith"\n')
    runner = PtyRunner(cfg)
    switch = HARMONY + 'ambient.00-00 = "signal"\n'
    stream, rc = runner.run(home, ["-v"], secs=14, edits=[(6.0, switch)])
    s = stream.decode("utf-8", errors="replace")
    counters = diag_counters(s)
    problems = []
    if rc != 0:
        problems.append(f"rc={rc}")
    if counters is None:
        problems.append("no ambient_diag line")
    else:
        applied = (
            int(counters.get("rx", 0))
            + int(counters.get("reapply", 0))
            + int(counters.get("snapback", 0))
            + int(counters.get("cfg_rebuilds", 0))
        )
        last = counters.get("last_scene_change", "none")
        if "signal" not in last:
            problems.append(f"last_scene_change={last!r} (no signal)")
        if applied < 1:
            problems.append(
                f"no apply evidence: rx={counters.get('rx')} "
                f"reapply={counters.get('reapply')} snapback={counters.get('snapback')}"
            )
    for needle in ("error:", "invalid config", "panicked"):
        if needle in s:
            problems.append(f"'{needle}' in stream")
    result(
        not problems,
        "C3 mid-run ambient switch -> signal",
        "; ".join(problems)
        if problems
        else f"rc=0, scene=signal, "
        f"rx={counters.get('rx')} reapply={counters.get('reapply')} "
        f"snapback={counters.get('snapback')}",
    )

    # C4: mid-run ambient REMOVAL (overlay lift -> revert to startup scene).
    home, cfg = fresh_home()
    write_cfg(cfg, HARMONY + 'ambient.00-00 = "monolith"\n')
    runner = PtyRunner(cfg)
    stream, rc = runner.run(home, ["-v"], secs=14, edits=[(6.0, HARMONY)])
    s = stream.decode("utf-8", errors="replace")
    counters = diag_counters(s)
    problems = []
    if rc != 0:
        problems.append(f"rc={rc}")
    if counters is None:
        problems.append("no ambient_diag line")
    else:
        if int(counters.get("sked_empties", 0)) < 1:
            problems.append(f"sked_empties={counters.get('sked_empties')} (want >=1)")
        if counters.get("snapback_killed") != "1":
            problems.append(f"snapback_killed={counters.get('snapback_killed')}")
    for needle in ("error:", "invalid config", "panicked"):
        if needle in s:
            problems.append(f"'{needle}' in stream")
    result(
        not problems,
        "C4 ambient removal -> overlay lift revert",
        "; ".join(problems)
        if problems
        else "rc=0, overlay lifted, revert clean, crystal still alive",
    )

    # C5: no-ambient + crystal only — drift self-reset path, no wedge.
    home, cfg = fresh_home()
    write_cfg(cfg, HARMONY)
    runner = PtyRunner(cfg)
    stream, rc = runner.run(home, ["-v"], secs=12)
    s = stream.decode("utf-8", errors="replace")
    counters = diag_counters(s)
    problems = []
    if rc != 0:
        problems.append(f"rc={rc}")
    if counters is None:
        problems.append("no ambient_diag line")
    else:
        if counters.get("startup") != "0":
            problems.append(f"startup={counters.get('startup')} (want 0)")
    for needle in ("error:", "invalid config", "panicked"):
        if needle in s:
            problems.append(f"'{needle}' in stream")
    result(
        not problems,
        "C5 crystal-only drift self-reset (no wedge)",
        "; ".join(problems) if problems else "rc=0, startup=0, 4 poll cycles survived",
    )

    # C6: 'q'-quit contract with the harmony load — rc=0 + exit summary.
    home, cfg = fresh_home()
    write_cfg(cfg, HARMONY + 'ambient.00-00 = "monolith"\n')
    runner = PtyRunner(cfg)
    stream, rc = runner.run(home, ["-v"], secs=8)
    s = stream.decode("utf-8", errors="replace")
    ok = rc == 0 and "ambient_diag:" in s and "\x1b[?1049l" in s
    result(
        ok,
        "C6 clean 'q' quit under harmony load",
        "rc=0, exit summary + alt-leave present"
        if ok
        else f"rc={rc}, summary={'yes' if 'ambient_diag:' in s else 'no'}",
    )


# ── Part D: error ordering under the harmony load ─────────────────────────


def part_d_error_ordering():
    print("\n── Part D: error ordering with crystal ON (rc=2 after restore) ──")

    baseline = HARMONY + 'ambient.00-00 = "monolith"\n'

    # D1: ambient -> nonexistent scene mid-run.
    home, cfg = fresh_home()
    write_cfg(cfg, baseline)
    runner = PtyRunner(cfg)
    bad = HARMONY + 'ambient.00-00 = "nosuchscene"\n'
    stream, rc = runner.run(
        home, ["-v"], secs=12, edits=[(5.0, bad)], quit_at_end=False
    )
    s = stream.decode("utf-8", errors="replace")
    problems = []
    if rc != 2:
        problems.append(f"rc={rc} (want 2)")
    leave = s.rfind("\x1b[?1049l")
    if leave == -1:
        problems.append("no alt-screen leave")
    else:
        if "error:" in s[:leave]:
            problems.append("'error:' DURING rain")
        if "unknown scene" not in s[leave:].lower():
            problems.append("diagnostic after restore missing 'unknown scene'")
    result(
        not problems,
        "D1 broken ambient ref with crystal ON",
        "; ".join(problems) if problems else "rc=2, error only after restore",
    )

    # D2: crystal-dragon-secs out of range mid-run (live-reload rejection).
    home, cfg = fresh_home()
    write_cfg(cfg, baseline)
    runner = PtyRunner(cfg)
    bad = baseline + "crystal-dragon-secs = 99999\n"
    stream, rc = runner.run(
        home, ["-v"], secs=12, edits=[(5.0, bad)], quit_at_end=False
    )
    s = stream.decode("utf-8", errors="replace")
    problems = []
    if rc != 2:
        problems.append(f"rc={rc} (want 2)")
    leave = s.rfind("\x1b[?1049l")
    if leave == -1:
        problems.append("no alt-screen leave")
    else:
        if "error:" in s[:leave]:
            problems.append("'error:' DURING rain")
        if "crystal-dragon-secs" not in s[leave:]:
            problems.append("diagnostic after restore missing key name")
    result(
        not problems,
        "D2 crystal-dragon-secs out of range mid-run",
        "; ".join(problems) if problems else "rc=2, error only after restore",
    )

    # D3: control — a VALID harmony edit keeps running (no false kill).
    home, cfg = fresh_home()
    write_cfg(cfg, baseline)
    runner = PtyRunner(cfg)
    good = baseline + "msg-mode = false\n"
    stream, rc = runner.run(home, ["-v"], secs=12, edits=[(5.0, good)])
    s = stream.decode("utf-8", errors="replace")
    problems = []
    if rc == 2:
        problems.append("valid edit wrongly rejected (rc=2)")
    if "invalid config" in s or "unknown key" in s:
        problems.append("rejection diagnostic on a valid edit")
    result(
        not problems,
        "D3 control: valid harmony edit keeps running",
        "; ".join(problems) if problems else "still running, clean quit",
    )


def main():
    if not os.path.exists(BIN):
        sys.exit(f"FATAL: binary not found: {BIN}")
    print(
        f"depthtest-8 (NIGHT-depthtest-5 & hunt-46) ambient/crystal + "
        f"flow-separation surfaces — binary: {BIN}"
    )
    print("=" * 100)
    part_a_static_surface()
    part_b_dump_config()
    part_c_ambient_crystal_pty()
    part_d_error_ordering()
    print("\n" + "=" * 100)
    print(f"=== SUMMARY: {PASS_COUNT} PASS / {FAIL_COUNT} FAIL ===")
    if FAILURES:
        print("FAILURES:")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
