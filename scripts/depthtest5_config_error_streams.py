#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL
#   or Git Bash on Windows).

"""NIGHT-hunt-44: verify + audit the config error surfaces pre-LTS.

Owner brief (2026-09-13): hunts 38/40/41 were marked done, but the
owner still doubts the class is closed — "when some function error on
config like wrong input typo, duplicate, etc problems the error output
is still on runtime not really exit so that is garbage on cinematic
screen. not just this function msg-mode should all existing function
on config need depth audit. before LTS landed."

This harness closes the three gaps the existing suites leave open:

  GAP 1 — stream separation. depth-test-config.py merges stdout+stderr
  into one blob, so it CANNOT see the owner's actual complaint: error
  text reaching the SCREEN (stdout) instead of stderr. Every error
  case here asserts rc=2 AND stdout == "" AND the diagnostic on
  stderr. A startup that prints its error to stdout would garble the
  cinematic handoff exactly the way the owner describes.

  GAP 2 — full-registry sweep. The owner said "not just msg-mode". The
  22 top-level USER_CONFIG_KEYS each get: key-typo, invalid-value
  (per type), duplicate-key — on the --doctor startup surface, plus a
  -v (verbose) re-check for a representative subset (the owner's own
  invocation style). Custom namespaces get field-typo probes
  (scene-custom.<n>.<field>, colors-custom.<n>.<field>, charset-custom
  .<n>.set, color.tune.<field>, ambient.<HH-MM> formats).

  GAP 3 — runtime ordering (PTY). A real terminal run with a mid-run
  bad-config edit must never paint error text over the rain: the PTY
  byte stream is checked so that every "error:"/"invalid config"
  occurrence lands AFTER the alternate-screen leave sequence
  (ESC[?1049l), i.e. printed after terminal restoration on exit —
  never during rendering. A clean-edit control asserts the watcher
  still live-reloads a VALID edit without exiting.

  Plus hunt-40 entry-budget BOUNDARIES (24 pass / 25 fail for all
  four custom namespaces, rain stops 9 pass / 10 fail, message
  length 200 pass / 201 fail) — the "at most" edges that an
  over-cap-only test can miss (an off-by-one that rejects 24, or a
  cap that silently tolerates 25, both pass an over-cap-only probe).

Exit 0 = all expectations met, exit 1 = at least one failure.

Usage: python3 depthtest5_config_error_streams.py [path-to-cosmostrix]
Default binary: ./target/pro-native/cosmostrix
"""

import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else "./target/pro-native/cosmostrix"
if not os.path.isabs(BIN):
    BIN = os.path.abspath(BIN)

TERM_ROWS, TERM_COLS = 40, 120
ALT_LEAVE = b"\x1b[?1049l"  # crossterm LeaveAlternateScreen

PASS_COUNT = 0
FAIL_COUNT = 0
FAILURES = []

# The full top-level key registry (configfile.rs USER_CONFIG_KEYS) with
# a per-key VALID example and an INVALID example of the most likely
# user mistake class for that key's type.
KEYS = [
    # (key, valid_value, invalid_value, invalid_class)
    ("scene", '"cinematic"', '"cinematics"', "bad enum"),
    ("color", '"energy-zen"', '"energy-zenn"', "bad enum"),
    ("charset", '"zen"', '"zenn"', "bad enum"),
    ("fps", "60", "99999", "over range"),
    ("speed", "9", "1000", "over range"),
    ("density", "0.75", "99.0", "over range"),
    ("monolith-size", '"normal"', '"normous"', "bad enum"),
    ("glitch-level", '"subtle"', '"subtls"', "bad enum"),
    ("bold", "1", "3", "over range"),
    ("shading-mode", "1", "5", "over range"),
    ("color-bg", '"black"', '"blacks"', "bad enum"),
    ("crystal-dragon", "false", "truee", "bad bool"),
    ("crystal-dragon-secs", "60", "99999", "over range"),
    ("power-dragon", "true", "truee", "bad bool"),
    ("async-mode", "true", "truee", "bad bool"),
    ("intro", '"logo"', '"logos"', "bad enum"),
    ("intro-color", '"energy-zen"', '"energy-zenn"', "bad enum"),
    ("message", '"ok"', '"' + "x" * 201 + '"', "over 200"),
    ("message-border", '"ok"', '"' + "y" * 201 + '"', "over 200"),
    ("msg-mode", "true", "truee", "bad bool"),
    ("msg-fill-style", '"engrave"', '"engrav"', "bad enum"),
    ("ambient-snapback-secs", "30", "99999", "over range"),
]

# Verify no typo'd key accidentally matches a real key.
_REAL_KEYS = {k for k, _, _, _ in KEYS}
for k, _, _, _ in KEYS:
    assert k + "y" not in _REAL_KEYS, f"typo of {k} collides with real key"


def fresh_home():
    """Fresh HOME with an (initially absent) config.toml."""
    home = tempfile.mkdtemp(prefix="h44_home_")
    cfgdir = os.path.join(home, ".config", "cosmostrix")
    os.makedirs(cfgdir, exist_ok=True)
    return home, os.path.join(cfgdir, "config.toml")


def run(home, extra_args, timeout=15, verbose=False):
    env = dict(os.environ)
    env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
    argv = [BIN] + (["-v"] if verbose else []) + extra_args
    try:
        p = subprocess.run(
            argv, env=env, capture_output=True, text=True,
            timeout=timeout, check=False,
        )
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", "", ""


def check(label, rc, out, err, must_contain=""):
    """The GAP-1 assertion: rc=2, stdout EMPTY, diagnostic on stderr."""
    global PASS_COUNT, FAIL_COUNT
    problems = []
    if rc != 2:
        problems.append(f"rc={rc}")
    if out.strip():
        problems.append(f"STDOUT NOT CLEAN: {out.strip()[:120]!r}")
    if not err.strip():
        problems.append("stderr empty")
    if must_contain and must_contain.lower() not in err.lower():
        problems.append(f"stderr missing '{must_contain}'")
    ok = not problems
    detail = "; ".join(problems) if problems else f"rc=2, stdout clean"
    status = "PASS" if ok else "FAIL"
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"{label} | {detail}")
    print(f"  [{status}] {label:58s} | {detail}")


def write_cfg(path, text):
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def part_a_startup_matrix():
    """GAP 1 + GAP 2: every key, every error class, stream-separated."""
    global PASS_COUNT, FAIL_COUNT
    print("=" * 100)
    print("PART A — full-registry startup error matrix (stdout must stay clean)")
    print("=" * 100)

    print("\n== A1. Key typos (all 22 top-level keys) ==")
    for key, _valid, _inv, _cls in KEYS:
        home, cfg = fresh_home()
        write_cfg(cfg, f"{key}y = xyz\n")
        rc, out, err = run(home, ["--doctor"])
        check(f"{key}y = ... (key typo)", rc, out, err, key if key != "color" else "color")

    print("\n== A2. Invalid values (per type) ==")
    for key, _valid, invalid, cls in KEYS:
        home, cfg = fresh_home()
        write_cfg(cfg, f"{key} = {invalid}\n")
        rc, out, err = run(home, ["--doctor"])
        check(f"{key} = {invalid[:24]} ({cls})", rc, out, err, key)

    print("\n== A3. Duplicate keys (all 22) ==")
    for key, valid, _inv, _cls in KEYS:
        home, cfg = fresh_home()
        write_cfg(cfg, f"{key} = {valid}\n{key} = {valid}\n")
        rc, out, err = run(home, ["--doctor"])
        check(f"{key} twice (duplicate)", rc, out, err, "duplicate")

    print("\n== A4. Owner's -v invocation style (verbose stays on stderr) ==")
    # The owner runs `cosmostrix -v`; verbose lines must also never
    # reach stdout, and the error must still exit 2.
    for key, valid, invalid, cls in [
        ("msg-mode", "true", "truee", "bad bool"),
        ("fps", "60", "99999", "over range"),
        ("scene", '"cinematic"', '"cinematics"', "bad enum"),
        ("density", "0.75", "99.0", "over range"),
    ]:
        home, cfg = fresh_home()
        write_cfg(cfg, f"{key} = {invalid}\n")
        rc, out, err = run(home, ["--doctor"], verbose=True)
        check(f"-v {key} = {invalid[:20]} ({cls})", rc, out, err, key)

    print("\n== A5. Valid values sanity (all 22 keys at once) ==")
    home, cfg = fresh_home()
    lines = [f"{key} = {valid}" for key, valid, _i, _c in KEYS]
    write_cfg(cfg, "\n".join(lines) + "\n")
    rc, out, err = run(home, ["--doctor"])
    ok = rc == 0
    status = "PASS" if ok else "FAIL"
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"all-22-valid baseline | rc={rc} err={err[:150]}")
    print(f"  [{status}] {'all 22 keys valid at once':58s} | rc={rc}")

    print("\n== A6. Custom-namespace field typos ==")
    ns_cases = [
        ("scene-custom.p.rains (field typo)",
         '[scene-custom.p]\nrains = "glyph"\ncolor = "green"\ncharset = "zen"\n'
         "fps = 60\nspeed = 9\ndensity = 0.8\nglitch-level = \"none\"\n",
         "rains"),
        ("colors-custom.p.background (field typo)",
         '[colors-custom.p]\nbackground = "#0a0a0a"\nrain = ["#111111", "#222222"]\n',
         "background"),
        ("charset-custom.p.sett (field typo)",
         '[charset-custom.p]\nsett = "ABC"\n',
         "sett"),
        ("color.tune.brightnes (field typo)",
         "[color.tune]\nbrightnes = 1.0\n",
         "brightnes"),
        ("ambient.25-00 (hour over 23)",
         'ambient.25-00 = "cinematic"\n',
         "ambient"),
        ("ambient.06-61 (minute over 59)",
         'ambient.06-61 = "cinematic"\n',
         "ambient"),
        ("ambient.6-00 (non 2-digit hour format)",
         'ambient.6-00 = "cinematic"\n',
         "ambient"),
        ("colors-custom rain 10 stops (max 9)",
         '[colors-custom.p]\nbg = "#0a0a0a"\n'
         'rain = ["#111111", "#222222", "#333333", "#444444", "#555555", '
         '"#666666", "#777777", "#888888", "#999999", "#aaaaaa"]\n',
         "rain"),
        ("scene-custom missing fields (incomplete block)",
         '[scene-custom.p]\nrain = "glyph"\n',
         "scene-custom"),
    ]
    for label, text, needle in ns_cases:
        home, cfg = fresh_home()
        write_cfg(cfg, text)
        rc, out, err = run(home, ["--doctor"])
        check(label, rc, out, err, needle)


def part_b_boundaries():
    """Hunt-40 entry-budget edges: 24 must PASS, 25 must FAIL."""
    print("\n" + "=" * 100)
    print("PART B — hunt-40 entry-budget boundaries (24 pass / 25 fail)")
    print("=" * 100)

    def blocks(n, header_fmt, body_fmt, name_fmt="s{:02d}"):
        out = []
        for i in range(n):
            out.append(header_fmt.format(name_fmt.format(i)))
            out.append(body_fmt)
        return "\n".join(out) + "\n"

    # scene-custom: 7 required fields each.
    scene_body = (
        'rain = "glyph"\ncolor = "green"\ncharset = "zen"\n'
        "fps = 60\nspeed = 9\ndensity = 0.8\nglitch-level = \"none\"\n"
    )

    cases = [
        ("scene-custom 24 blocks (at cap)", blocks(24, "[scene-custom.{}]", scene_body), 0),
        ("scene-custom 25 blocks (over cap)", blocks(25, "[scene-custom.{}]", scene_body), 2),
        ("colors-custom 24 blocks (at cap)",
         blocks(24, "[colors-custom.{}]", 'bg = "#0a0a0a"\nrain = ["#111111", "#222222"]\n'), 0),
        ("colors-custom 25 blocks (over cap)",
         blocks(25, "[colors-custom.{}]", 'bg = "#0a0a0a"\nrain = ["#111111", "#222222"]\n'), 2),
        ("charset-custom 24 blocks (at cap)",
         blocks(24, "[charset-custom.{}]", 'set = "ABC"\n'), 0),
        ("charset-custom 25 blocks (over cap)",
         blocks(25, "[charset-custom.{}]", 'set = "ABC"\n'), 2),
        ("ambient 24 entries (at cap)",
         "".join(f'ambient.{h:02d}-30 = "cinematic"\n' for h in range(24)), 0),
        ("ambient 25 entries (over cap)",
         "".join(f'ambient.{h:02d}-30 = "cinematic"\n' for h in range(24))
         + 'ambient.00-45 = "cinematic"\n', 2),
        ("colors-custom rain 9 stops (at cap)",
         '[colors-custom.p]\nbg = "#0a0a0a"\nrain = ['
         + ", ".join(f'"#{i:06x}"' for i in range(0x111111, 0x999999, 0x111111)) + "]\n", 0),
        ("message 200 chars (at cap)", 'message = "' + "m" * 200 + '"\n', 0),
        ("message 201 chars (over cap)", 'message = "' + "m" * 201 + '"\n', 2),
    ]

    for label, text, want in cases:
        home, cfg = fresh_home()
        write_cfg(cfg, text)
        rc, out, err = run(home, ["--doctor"])
        global PASS_COUNT, FAIL_COUNT
        if want == 0:
            ok = rc == 0
            detail = f"rc={rc}"
        else:
            ok = rc == 2 and not out.strip()
            detail = f"rc={rc}" + ("" if not out.strip() else " + STDOUT DIRTY")
        status = "PASS" if ok else "FAIL"
        if ok:
            PASS_COUNT += 1
        else:
            FAIL_COUNT += 1
            FAILURES.append(f"{label} | want rc={want}, {detail}; err={err[:120]}")
        print(f"  [{status}] {label:58s} | want rc={want}, {detail}")


def run_pty(argv, secs, edits, home):
    """Run in a PTY; edits = [(t, cfg_text)]. Returns (bytes, rc).

    home -- the sandbox HOME (the config must live at the DEFAULT
    path inside it: a --config /tmp/... override is rejected by the
    safepath allowlist, which would fail the run for the wrong
    reason).
    """
    master, slave = pty.openpty()
    fcntl.ioctl(master, termios.TIOCSWINSZ,
                struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0))
    env = dict(os.environ)
    env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
    env["TERM_PROGRAM"] = "alacritty"
    env["COLORTERM"] = "truecolor"
    proc = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave, env=env)
    os.close(slave)
    buf = bytearray()
    t0 = time.time()
    edits = list(edits)
    exited = None
    while time.time() - t0 < secs:
        r, _, _ = select.select([master], [], [], 0.05)
        if r:
            try:
                c = os.read(master, 65536)
            except OSError:
                break
            if not c:
                break
            buf.extend(c)
        now = time.time() - t0
        while edits and now >= edits[0][0]:
            _, text = edits.pop(0)
            write_cfg(run_pty._cfg, text)
        if proc.poll() is not None:
            exited = proc.returncode
            # Drain briefly.
            deadline = time.time() + 0.5
            while time.time() < deadline:
                r, _, _ = select.select([master], [], [], 0.05)
                if r:
                    try:
                        c = os.read(master, 65536)
                    except OSError:
                        break
                    if not c:
                        break
                    buf.extend(c)
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


def part_c_runtime_pty():
    """GAP 3: bad-config edit mid-run — error only AFTER screen restore.

    Two windows (NIGHT-hunt-44): during the intro (the watcher now
    spawns BEFORE the intro, so intro-window edits are caught — they
    used to be silently baked into the baseline) and after the intro
    (the steady-state contract). Plus valid-edit controls for both.
    """
    print("\n" + "=" * 100)
    print("PART C — runtime PTY: error ordering + clean-exit contract")
    print("=" * 100)
    global PASS_COUNT, FAIL_COUNT

    good = "msg-mode = true\nfps = 60\n"
    bad = "msg-modey = true\nfps = 60\n"

    def run_case(label, edit_at, text_after, want_rc, want_still_running, secs):
        global PASS_COUNT, FAIL_COUNT
        home, cfg = fresh_home()
        write_cfg(cfg, good)
        run_pty._cfg = cfg
        stream, rc = run_pty([BIN], secs=secs, edits=[(edit_at, text_after)], home=home)
        s = stream.decode("utf-8", errors="replace")
        problems = []
        leave = s.rfind("\x1b[?1049l")
        if want_still_running:
            if rc == 2:
                problems.append("valid edit wrongly rejected (rc=2)")
            if "invalid config" in s or "unknown key" in s:
                problems.append("rejection diagnostic on a valid edit")
        else:
            if rc != want_rc:
                problems.append(f"rc={rc} (want {want_rc})")
            if leave == -1:
                problems.append("no alt-screen leave sequence in stream")
            else:
                head = s[:leave]
                for needle in ("error:", "invalid config", "unknown key"):
                    if needle in head:
                        problems.append(f"'{needle}' printed DURING rain (before restore)")
            after = s[leave:] if leave != -1 else ""
            if "invalid config" not in after and "unknown key" not in after:
                problems.append("no rejection diagnostic after restore")
        if problems:
            FAIL_COUNT += 1
            FAILURES.append(f"{label} | {'; '.join(problems)}")
            print(f"  [FAIL] {label:58s} | {'; '.join(problems)}")
        else:
            PASS_COUNT += 1
            note = "still running" if want_still_running else f"rc={want_rc}, error after restore"
            print(f"  [PASS] {label:58s} | {note}")

    # C1: bad edit DURING the intro window (~t=2s; the logo intro plays
    # for several seconds). Pre-hunt-44 this edit was silently missed;
    # now the watcher (spawned pre-intro) catches it and the loop
    # breaks on the first post-intro drain.
    run_case("C1: bad edit during intro -> clean exit", 2.0, bad, 2, False, 18)
    # C2: bad edit AFTER the intro (steady state).
    run_case("C2: bad edit after intro -> clean exit", 9.0, bad, 2, False, 20)
    # C3: valid edit during the intro — must keep running (no exit).
    run_case("C3: valid edit during intro keeps running", 2.0,
             "msg-mode = true\nfps = 45\n", 0, True, 16)
    # C4: valid edit after the intro — the classic live-reload control.
    run_case("C4: valid edit after intro keeps running", 9.0,
             "msg-mode = true\nfps = 45\n", 0, True, 16)


def main():
    if not os.path.exists(BIN):
        print(f"binary not found: {BIN}")
        return 1
    print(f"binary: {BIN}")
    print(f"harness: depthtest5_config_error_streams.py — NIGHT-hunt-44 verify + audit")
    part_a_startup_matrix()
    part_b_boundaries()
    part_c_runtime_pty()
    print("\n" + "=" * 100)
    print(f"depthtest5 results: {PASS_COUNT} PASS, {FAIL_COUNT} FAIL")
    print("=" * 100)
    if FAILURES:
        print(f"\nFAILURES ({len(FAILURES)}):")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("\nALL expectations met.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
