#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
"""NIGHT-hunt-47 (depthtest-7): the [color.tune] + scene-custom
REFERENCE GRAPH, depth-audited on the real binary across all three
error surfaces (startup, --testconf, live-reload watcher).

Owner-approved extension of the hunt-44/46 error-stream matrices:
the config is a graph, not a flat list — values REFERENCE other
blocks (scene-custom.t.color -> [colors-custom.y], ambient.HH-MM ->
[scene-custom.z], scene-custom.t.colors-custom -> [colors-custom.y]).
A dangling reference is the same class as a typo'd key: it must fail
fast on every surface, never silently fall back to defaults.

Graph nodes under test:

  top-level color/charset ......... builtin | [colors-custom.*] / [charset-custom.*]
  [scene-custom.x].color .......... builtin | [colors-custom.*]
  [scene-custom.x].colors-custom .. [colors-custom.*]
  [scene-custom.x].charset ........ builtin | [charset-custom.*]
  [scene-custom.x].charset-custom . [charset-custom.*]
  ambient.HH-MM .................... builtin scene | [scene-custom.*]
  color.tune.<field> .............. float in [0.0, 3.0] (long forms only)
  transitive ...................... scene-custom -> colors-custom -> hex validity

Parts:

  A. Dangling-reference static matrix (startup + --testconf): every
     edge type with a nonexistent target must exit 2 with EMPTY
     stdout and a diagnostic that NAMES the block and field.
  B. Positive controls: the fully-wired graph (custom scene ->
     custom palette -> custom charset -> ambient schedule) runs.
  C. Live-reload PTY matrix (the hunt-44 C-window contract): a
     reference-breaking edit mid-run must exit 2 cleanly with the
     diagnostic printed only AFTER the alt-screen restore — never
     garbage on the cinematic screen. Includes the deletion edge:
     removing a [colors-custom.*] block that a scene-custom still
     references must reject, not silently fall back.

Usage: python3 scripts/depthtest7_reference_graph.py [binary]
       (default binary: ./target/pro-native/cosmostrix)
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
ALT_LEAVE = b"\x1b[?1049l"

PASS_COUNT = 0
FAIL_COUNT = 0
FAILURES = []


def fresh_home():
    home = tempfile.mkdtemp(prefix="h47_home_")
    os.makedirs(os.path.join(home, ".config", "cosmostrix"), exist_ok=True)
    return home, os.path.join(home, ".config", "cosmostrix", "config.toml")


def write_cfg(path, text):
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def run(home, args, timeout=15):
    env = dict(os.environ)
    env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
    try:
        p = subprocess.run(
            [BIN] + args, env=env, capture_output=True, text=True,
            timeout=timeout, check=False,
        )
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", "", ""


# A complete, VALID scene-custom block (the graph baseline).
SC_T = """[scene-custom.t]
rain = "glyph"
color = "cosmos"
charset = "binary"
fps = 60
speed = 20
density = 0.75
glitch-level = "subtle"
"""


def check(label, rc, out, err, must_contain=""):
    """hunt-44 assertion (STARTUP runs): rc=2, stdout EMPTY, stderr diagnostic."""
    global PASS_COUNT, FAIL_COUNT
    problems = []
    if rc != 2:
        problems.append(f"rc={rc}")
    if out.strip():
        problems.append(f"STDOUT NOT CLEAN: {out.strip()[:100]!r}")
    if not err.strip():
        problems.append("stderr empty")
    if must_contain and must_contain.lower() not in err.lower():
        problems.append(f"stderr missing '{must_contain}'")
    ok = not problems
    detail = "; ".join(problems) if problems else "rc=2, stdout clean"
    status = "PASS" if ok else "FAIL"
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"{label} | {detail}")
    print(f"  [{status}] {label:56s} | {detail}")


def check_testconf(label, rc, out, err, must_contain=""):
    """--testconf variant: stdout legitimately carries the 'checking
    <path>' / 'file-sha512' progress lines (that is the tool's purpose);
    the contract is rc=2 + the diagnostic on STDERR + no 'error:' text
    on stdout (depthtest-5 convention for the testconf surface)."""
    global PASS_COUNT, FAIL_COUNT
    problems = []
    if rc != 2:
        problems.append(f"rc={rc}")
    if "error:" in out.lower():
        problems.append(f"error text on STDOUT: {out[:100]!r}")
    if not err.strip():
        problems.append("stderr empty")
    if must_contain and must_contain.lower() not in err.lower():
        problems.append(f"stderr missing '{must_contain}'")
    ok = not problems
    detail = "; ".join(problems) if problems else "rc=2, errors on stderr"
    status = "PASS" if ok else "FAIL"
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"{label} | {detail}")
    print(f"  [{status}] {label:56s} | {detail}")


def part_a_static_matrix():
    print("\n── Part A: dangling-reference static matrix (startup + --testconf) ──")
    global PASS_COUNT, FAIL_COUNT

    def both(label, cfg, frag):
        """Same config through both static surfaces."""
        home, cfg_path = fresh_home()
        write_cfg(cfg_path, cfg)
        rc, out, err = run(home, ["--testconf"])
        check_testconf(f"[testconf] {label}", rc, out, err, must_contain=frag)
        rc, out, err = run(home, ["--bench-frames", "3"])
        check(f"[startup]  {label}", rc, out, err, must_contain=frag)

    def block_with(field, value):
        """SC_T with ONE reference field replaced by a dangling target."""
        lines = SC_T.strip().splitlines()
        key = field.split("|")[0]
        out_lines = [
            ln for ln in lines
            if not ln.startswith(key + " ") and not ln.startswith(field.split("|")[-1] + " ")
        ]
        out_lines.append(f'{field.split("|")[-1] if "|" in field else field} = "{value}"')
        return "\n".join(out_lines) + "\n"

    # The four scene-custom reference edges, dangling.
    both("sc.color -> nonexistent builtin",
         block_with("color", "nonexistentpal"), "unknown color")
    both("sc.colors-custom -> undefined block",
         block_with("colors-custom", "nosuchblock"), "unknown colors-custom block")
    both("sc.charset -> nonexistent charset",
         block_with("charset", "nosuchset"), "unknown charset")
    both("sc.charset-custom -> undefined block",
         block_with("charset-custom", "nosuchblock"), "unknown charset-custom block")

    # ambient edges.
    both("ambient -> nonexistent scene",
         'ambient.03-00 = "nosuchscene"\n', "unknown scene")
    both("ambient hour out of range (25-00)",
         'ambient.25-00 = "cinematic"\n', "unknown key")
    both("ambient hour not zero-padded (3-00)",
         'ambient.3-00 = "cinematic"\n', "unknown key")

    # transitive: the referenced palette itself is broken.
    both("transitive: referenced palette bad bg",
         '[colors-custom.bad]\nbg = "nothex"\nrain = ["#112233", "#445566"]\n\n'
         + SC_T.replace('color = "cosmos"', 'colors-custom = "bad"'),
         "colors-custom")
    both("transitive: referenced palette too few stops",
         '[colors-custom.bad]\nbg = "#0a0a0a"\nrain = ["#112233"]\n\n'
         + SC_T.replace('color = "cosmos"', 'colors-custom = "bad"'),
         "colors-custom")

    # color.tune value edges (config long forms only — short forms are
    # CLI-only vocabulary).
    both("color.tune.saturation over range", 'color.tune.saturation = 9\n', "out of range")
    both("color.tune.saturation negative", 'color.tune.saturation = -0.5\n', "out of range")
    both("color.tune.brightness non-numeric", 'color.tune.brightness = "abc"\n', "expected number")
    both("color.tune unknown field (typo)", 'color.tune.satt = 1.5\n', "unknown key")
    both("color.tune short form (CLI vocab)", 'color.tune.sat = 1.5\n', "unknown key")


def part_b_positive_controls():
    print("\n── Part B: positive controls (the wired graph runs) ──")
    global PASS_COUNT, FAIL_COUNT

    graph = (
        '[colors-custom.pal]\n'
        'bg = "#0a0a0a"\n'
        'rain = ["#1a0033", "#4d0080", "#9933ff"]\n\n'
        '[charset-custom.zen]\n'
        'set = "|+"\n\n'
        '[scene-custom.t]\n'
        'rain = "glyph"\n'
        'colors-custom = "pal"\n'
        'charset-custom = "zen"\n'
        'fps = 60\n'
        'speed = 20\n'
        'density = 0.75\n'
        'glitch-level = "subtle"\n\n'
        'ambient.03-00 = "t"\n'
        'ambient.22-00 = "cinematic"\n'
        'color.tune.saturation = 1.2\n'
    )
    home, cfg_path = fresh_home()
    write_cfg(cfg_path, graph)

    # Startup: the fully-wired graph must run (bench = headless exit).
    rc, out, err = run(home, ["--bench-frames", "3"], timeout=30)
    ok = rc == 0 and "frames" in out.lower()
    status = "PASS" if ok else "FAIL"
    print(f"  [{status}] {'full graph runs (palette+charset+ambient+tune)':56s} | rc={rc}")
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"full graph runs | rc={rc}, err={err[:120]!r}")

    # CLI activation of the custom scene.
    rc, out, err = run(home, ["--scene-custom", "t", "--bench-frames", "3"], timeout=30)
    ok = rc == 0 and "frames" in out.lower()
    status = "PASS" if ok else "FAIL"
    print(f"  [{status}] {'--scene-custom t activates the graph':56s} | rc={rc}")
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"--scene-custom t | rc={rc}, err={err[:120]!r}")

    # NIGHT-hunt-47 finding lock: --bench-frames emits the text BENCH:
    # format; --json is honored only by --benchmark/--bench-all. The
    # silent ignore was hidden behavior — now a stderr warning fires
    # (src/bench/bench_helpers.rs collect_bench_noop_warnings, pinned by
    # bench_frames_with_json_warns_noop in the Rust suite).
    rc, out, err = run(home, ["--bench-frames", "3", "--json"], timeout=30)
    ok = rc == 0 and "BENCH" in out and "--json ignored" in err
    status = "PASS" if ok else "FAIL"
    print(f"  [{status}] {'--json + --bench-frames warns (no silent ignore)':56s} | rc={rc}")
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"json bench-frames warning | rc={rc}, err={err[:120]!r}")

    # Deterministic conflict handling: with BOTH pair members present
    # the block is complete (has = primary || alt), --show-scene dumps
    # BOTH fields honestly, and the apply-time winner (color beats
    # colors-custom) is pinned by the Rust suite
    # (test/config/live_config/tests_cli_priority.rs ::
    #  rebuild_scene_custom_block_color_beats_colors_custom — Z1-2).
    conflict = (
        '[colors-custom.pal]\n'
        'bg = "#0a0a0a"\n'
        'rain = ["#1a0033", "#4d0080"]\n\n'
        + SC_T.replace('color = "cosmos"', 'colors-custom = "pal"\ncolor = "cosmos"')
    )
    home2, cfg2 = fresh_home()
    write_cfg(cfg2, conflict)
    rc, out, err = run(home2, ["--show-scene", "t"])
    ok = (
        rc == 0
        and "color" in out
        and "colors-custom" in out
        and "pal" in out
    )
    status = "PASS" if ok else "FAIL"
    print(f"  [{status}] {'both-pairs present: dumped honestly, winner pinned':56s} | rc={rc}")
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"both-pairs display | rc={rc}, out={out[:160]!r}")


def run_pty(argv, secs, edits, home):
    """hunt-44 PTY harness: run in a PTY, apply config edits at t."""
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


def part_c_live_reload_pty():
    print("\n── Part C: live-reload reference-graph rejection (PTY) ──")
    global PASS_COUNT, FAIL_COUNT

    # The running baseline: a fully-wired, valid graph.
    baseline = (
        '[colors-custom.pal]\n'
        'bg = "#0a0a0a"\n'
        'rain = ["#1a0033", "#4d0080", "#9933ff"]\n\n'
        '[charset-custom.zen]\n'
        'set = "|+"\n\n'
        '[scene-custom.t]\n'
        'rain = "glyph"\n'
        'colors-custom = "pal"\n'
        'charset-custom = "zen"\n'
        'fps = 60\n'
        'speed = 20\n'
        'density = 0.75\n'
        'glitch-level = "subtle"\n'
    )

    def breaker(mutate):
        """Rebuild the baseline with a reference-breaking mutation."""
        return mutate(baseline)

    cases = [
        ("C1: sc.colors-custom -> nonexistent mid-run",
         breaker(lambda b: b.replace('colors-custom = "pal"', 'colors-custom = "nosuch"')),
         "unknown colors-custom block", 11.0),
        ("C2: referenced palette hex broken mid-run",
         breaker(lambda b: b.replace('bg = "#0a0a0a"', 'bg = "nothex"')),
         "colors-custom", 11.0),
        ("C3: ambient -> nonexistent scene mid-run",
         breaker(lambda b: b + 'ambient.03-00 = "nosuchscene"\n'),
         "unknown scene", 11.0),
        ("C4: color.tune out of range mid-run",
         breaker(lambda b: b + 'color.tune.saturation = 9\n'),
         "out of range", 11.0),
        ("C5: DELETE referenced palette block mid-run",
         breaker(lambda b: b.replace('[colors-custom.pal]\nbg = "#0a0a0a"\nrain = ["#1a0033", "#4d0080", "#9933ff"]\n\n', '')),
         "unknown colors-custom block", 11.0),
    ]

    for label, bad_cfg, frag, edit_at in cases:
        home, cfg = fresh_home()
        write_cfg(cfg, baseline)
        run_pty._cfg = cfg
        stream, rc = run_pty([BIN], secs=22, edits=[(edit_at, bad_cfg)], home=home)
        s = stream.decode("utf-8", errors="replace")
        problems = []
        if rc != 2:
            problems.append(f"rc={rc} (want 2)")
        leave = s.rfind("\x1b[?1049l")
        if leave == -1:
            problems.append("no alt-screen leave in stream")
        else:
            head = s[:leave]
            for needle in ("error:", "invalid config", "unknown key"):
                if needle in head:
                    problems.append(f"'{needle}' DURING rain (before restore)")
        after = s[leave:] if leave != -1 else ""
        if frag.lower() not in after.lower():
            problems.append(f"diagnostic after restore missing '{frag}'")
        ok = not problems
        status = "PASS" if ok else "FAIL"
        detail = "; ".join(problems) if problems else "rc=2, error only after restore"
        if ok:
            PASS_COUNT += 1
        else:
            FAIL_COUNT += 1
            FAILURES.append(f"{label} | {detail}")
        print(f"  [{status}] {label:56s} | {detail}")

    # Control: a VALID reference-graph edit must keep the run alive.
    home, cfg = fresh_home()
    write_cfg(cfg, baseline)
    run_pty._cfg = cfg
    good_edit = baseline.replace('density = 0.75', 'density = 0.80')
    stream, rc = run_pty([BIN], secs=22, edits=[(11.0, good_edit)], home=home)
    s = stream.decode("utf-8", errors="replace")
    problems = []
    if rc == 2:
        problems.append("valid edit wrongly rejected (rc=2)")
    if "invalid config" in s or "unknown key" in s:
        problems.append("rejection diagnostic on a valid edit")
    ok = not problems
    status = "PASS" if ok else "FAIL"
    detail = "; ".join(problems) if problems else "still running, no rejection"
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"control: valid graph edit | {detail}")
    print(f"  [{status}] {'control: valid graph edit keeps running':56s} | {detail}")


def main():
    if not os.path.exists(BIN):
        sys.exit(f"FATAL: binary not found: {BIN}")
    print(f"depthtest-7 (NIGHT-hunt-47) reference graph — binary: {BIN}")
    part_a_static_matrix()
    part_b_positive_controls()
    part_c_live_reload_pty()
    print(f"\n=== SUMMARY: {PASS_COUNT} PASS / {FAIL_COUNT} FAIL ===")
    if FAILURES:
        print("\nFAILURES:")
        for f in FAILURES:
            print(f"  - {f}")
    sys.exit(1 if FAIL_COUNT else 0)


if __name__ == "__main__":
    main()
