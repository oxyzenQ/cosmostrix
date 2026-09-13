#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
"""NIGHT-hunt-46 (depthtest-6): CLI-flag typo coverage on the REAL
binary — the same error-stream contract NIGHT-hunt-44 pinned for
config.toml, applied to the other surface: argv.

Owner mandate: the config-key matrix (depthtest-5) closed the
"typo/duplicate/invalid" class for config.toml. This extends it to
CLI flags (same class, different surface) before LTS.

Contract under test, for EVERY error case:

  1. rc == 2 (usage error — never a panic, never 0, never 101)
  2. stdout is EMPTY (the cinematic-screen guarantee: no error text,
     no escape bytes, no partial frames may leak onto stdout)
  3. stderr carries the diagnostic (error: ...)
  4. where a suggestion/migration is contractually expected, stderr
     carries it (tip: ... / has been removed ...)

Parts:

  A. Typo sweep — every live long flag (derived from the clap Args
     struct in src/config/mod.rs, minus the prevalidation-blocked
     long forms): a deterministic single-character deletion must
     produce rc=2 + clean stdout + a "tip: a similar argument
     exists" pointing back at the flag. Single-deletion Jaro is
     mathematically >= 0.72 for name lengths >= 2, so the tip is
     guaranteed by the same math clap uses.
  B. Value-domain matrix — every value-taking flag's rejection
     paths (ranges, enums, pair formats, missing values, unknown
     names), all verified against the binary before encoding.
  C. Removed-registry migration sweep — every entry parsed live
     out of src/validation/mod.rs REMOVED_FLAGS: "has been removed"
     + migration guidance, rc=2, clean stdout. Includes the
     NIGHT-hunt-46 fix (--disable-effects rename drift).
  D. Case-rescue + shorthand + alias + -v interplay special cases.
  E. PTY alt-screen invariant — a representative startup error must
     die BEFORE any alt-screen enter is written to the terminal.

Usage: python3 scripts/depthtest6_cli_flag_error_streams.py [binary]
       (default binary: ./target/pro-native/cosmostrix)
"""

import os
import pty
import re
import select
import subprocess
import sys
import tempfile
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else "./target/pro-native/cosmostrix"
if not os.path.isabs(BIN):
    BIN = os.path.abspath(BIN)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ARGS_SRC = os.path.join(REPO, "src", "config", "mod.rs")
VALIDATION_SRC = os.path.join(REPO, "src", "validation", "mod.rs")

PASS_COUNT = 0
FAIL_COUNT = 0
FAILURES = []


def fresh_home():
    home = tempfile.mkdtemp(prefix="h46_home_")
    os.makedirs(os.path.join(home, ".config", "cosmostrix"), exist_ok=True)
    return home


def run(args, timeout=15):
    env = dict(os.environ)
    env["HOME"] = fresh_home()
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


def check(label, rc, out, err, must_contain=""):
    """The hunt-44 assertion, CLI edition: rc=2, stdout EMPTY, stderr
    diagnostic, and (optionally) a contractually-expected fragment."""
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


def collect_live_long_flags():
    """Long flags from the clap Args struct (single source of truth),
    minus names the prevalidation registry blocks as direct long
    forms (the -mb/-mfs expansion targets are Args-internal)."""
    src = open(ARGS_SRC, encoding="utf-8", errors="replace").read()
    m = re.search(r"pub struct Args \{.*?\n\}", src, re.DOTALL)
    if not m:
        sys.exit("FATAL: could not locate `pub struct Args` in src/config/mod.rs")
    body = m.group(0)
    names = set(re.findall(r'long = "([a-z0-9\-]+)"', body))
    names |= set(re.findall(r'alias = "([a-z0-9\-]+)"', body))
    reg = open(VALIDATION_SRC, encoding="utf-8", errors="replace").read()
    blocked = set(
        "--" + f for f in re.findall(r'^\s+"(--[a-z0-9\-]+)",\s*$', reg, re.MULTILINE)
    )
    return sorted("--" + n for n in names if ("--" + n) not in blocked)


def collect_removed_flags():
    reg = open(VALIDATION_SRC, encoding="utf-8", errors="replace").read()
    m = re.search(r"REMOVED_FLAGS[^=]*=\s*&\[", reg)
    if not m:
        sys.exit("FATAL: could not locate REMOVED_FLAGS registry")
    body = reg[m.end():]
    body = body[: body.find("];")]
    return sorted(set(re.findall(r'"(--[a-z0-9\-]+)"', body)))


def part_a_typo_sweep():
    print("\n── Part A: single-deletion typo sweep over every live long flag ──")
    flags = collect_live_long_flags()
    print(f"  derived {len(flags)} live long flags from the Args struct")
    # Only names long enough for a deterministic in-family typo.
    # Single deletion from an n>=3 name keeps Jaro >= 0.72 (clap's
    # threshold is > 0.7), so the tip contract is guaranteed.
    all_names = set(flags)
    swept = 0
    for flag in flags:
        name = flag[2:]
        if len(name) < 3:
            continue
        # drop a middle character (position len//2) — deterministic
        idx = len(name) // 2
        typo = "--" + name[:idx] + name[idx + 1:]
        if typo in all_names:
            print(f"  [SKIP] {flag}: generated typo {typo} collides with a live flag")
            continue
        rc, out, err = run([typo])
        # For names <= 3 chars the Jaro match window collapses to 0
        # and a longer flag can legitimately outscore the original
        # (verified: --fs -> jaro 0.611 vs --fps, 0.733 vs
        # --perf-stats — clap suggests --perf-stats). Short names
        # therefore only assert tip PRESENCE; names >= 4 chars keep
        # the strict point-back-at-the-original assertion.
        if len(name) <= 3:
            check(f"typo {typo} -> a tip exists", rc, out, err,
                  must_contain="tip: a similar argument exists:")
        else:
            check(
                f"typo {typo} -> tip {flag}",
                rc, out, err,
                must_contain=f"tip: a similar argument exists: '{flag}'",
            )
        swept += 1
    print(f"  swept {swept} flags")


def part_b_value_matrix():
    print("\n── Part B: value-domain rejection matrix (verified contracts) ──")
    cases = [
        # (label, argv, must_contain)
        ("fps below range", ["--fps", "0"], "1..=240"),
        ("fps above range", ["--fps", "241"], "1..=240"),
        ("fps non-numeric", ["--fps", "abc"], "1..=240"),
        ("speed below range", ["--speed", "0"], "1..=100"),
        ("speed above range", ["--speed", "101"], "1..=100"),
        ("density below range", ["--density", "0"], "0.01"),
        ("density above range", ["--density", "5.001"], "0.01"),
        ("bold above range", ["--bold", "3"], "0..=2"),
        ("shading-mode above range", ["--shading-mode", "2"], "0..=1"),
        ("color-mode invalid", ["--color-mode", "13"], "allowed"),
        ("bench-scene typo (README case)", ["--bench-scene", "leanax"], "possible values"),
        ("screen-size non-numeric", ["--screen-size", "abc"], "WxH"),
        ("screen-size missing x", ["--screen-size", "2"], "WxH"),
        ("screen-size empty height", ["--screen-size", "2x"], "height"),
        ("screen-size zero dim", ["--screen-size", "0x4"], "zero"),
        ("screen-size zero dim 2", ["--screen-size", "4x0"], "zero"),
        ("screen-size width over u16", ["--screen-size", "70000x2"], "width"),
        ("screen-size height over u16", ["--screen-size", "2x70000"], "height"),
        ("bench-duration below min", ["--bench-duration", "0"], "minimum"),
        ("bench-duration over 24h", ["--bench-duration", "90000"], "ceiling"),
        ("bench-duration garbage", ["--bench-duration", "abc"], "invalid format"),
        ("duration below min", ["--duration", "0.05"], "0.1"),
        ("duration over max", ["--duration", "90000"], "86400"),
        ("crystal-dragon-secs negative", ["--crystal-dragon-secs=-1"], "negative"),
        ("crystal-dragon-secs over max", ["--crystal-dragon-secs", "86401"], "86400"),
        ("crystal-dragon numeric hint", ["--crystal-dragon", "5"], "--crystal-dragon-secs"),
        ("async-mode bad bool", ["--async-mode", "truthy"], "boolean"),
        ("msg-mode bad bool", ["--msg-mode", "yep"], "boolean"),
        ("power-dragon bad bool", ["--power-dragon", "onoff"], "boolean"),
        ("glitch-ms low below min", ["-g", "0,400"], "range"),
        ("glitch-ms high above max", ["-g", "300,5001"], "1..=5000"),
        ("glitch-ms non-numeric", ["-g", "abc"], "NU"),
        ("glitch-ms missing pair", ["-g", "300"], "NU"),
        ("linger-ms high above max", ["-l", "1,60001"], "1..=60000"),
        ("color-tune over range", ["--color-tune", "sat=9"], "out of range"),
        ("color-tune unknown key", ["--color-tune", "wrong=1"], "unknown key"),
        ("color-tune non-numeric", ["--color-tune", "sat=abc"], "not a number"),
        ("intro bad enum", ["--intro", "wrongv"], "possible values"),
        ("monolith-size bad enum", ["--monolith-size", "huge"], "small"),
        ("glitch-level typo value", ["--glitch-level", "sutble"], "subtle"),
        ("color-bg bad enum", ["--color-bg", "blackest"], "black"),
        ("msg-fill-style bad enum", ["--msg-fill-style", "wrongstyle"], "possible values"),
        ("color unknown name", ["--color", "nonexistent-theme-xyz"], "--list-colors"),
        ("scene unknown name", ["--scene", "nosuchscene"], "--list-scenes"),
        ("scene-custom unknown", ["--scene-custom", "nosuch"], "unknown custom scene"),
        ("charset unknown name", ["--charset", "nosuchset"], "--list-charsets"),
        ("colors-custom unknown", ["--colors-custom", "nosuchpal"], "not found"),
        ("intro-color unknown", ["--intro-color", "nosuch"], "not a builtin"),
        ("show-scene unknown", ["--show-scene", "nosuch"], "unknown scene"),
        ("--color missing value", ["--color"], "value is required"),
        ("--fps missing value", ["--fps"], "value is required"),
        ("-m missing value", ["-m"], "value is required"),
        ("-b missing value", ["-b"], "value is required"),
        ("-g missing value", ["-g"], "value is required"),
        ("--config missing value", ["--config"], "value is required"),
    ]
    for label, argv, frag in cases:
        rc, out, err = run(argv)
        check(label, rc, out, err, must_contain=frag)


def part_c_removed_registry():
    print("\n── Part C: REMOVED_FLAGS registry migration sweep ──")
    removed = collect_removed_flags()
    print(f"  parsed {len(removed)} registry entries from src/validation/mod.rs")
    for flag in removed:
        rc, out, err = run([flag])
        check(
            f"removed {flag} -> migration hint",
            rc, out, err,
            must_contain="has been removed",
        )
    # The equals form must be intercepted too (registry contract).
    rc, out, err = run(["--disable-effects=true"])
    check(
        "removed --disable-effects=<v> equals form",
        rc, out, err,
        must_contain="has been removed",
    )


def part_d_specials():
    print("\n── Part D: case-rescue, shorthand typo, alias, -v interplay ──")
    cases = [
        ("--LIS case rescue", ["--LIS"], "tip: a similar argument exists: '--list-scenes'"),
        ("--HELPSS case rescue", ["--HELPSS"], "tip: a similar argument exists: '--help'"),
        ("-mfss shorthand typo", ["-mfss", "engrave"], "--msg-fill-style"),
        ("typo with -v present", ["--colr", "-v"], "'--color'"),
        ("-v before typo", ["-v", "--scne", "x"], "'--scene'"),
    ]
    for label, argv, frag in cases:
        rc, out, err = run(argv)
        check(label, rc, out, err, must_contain=frag)

    # Alias positive control: --charset-custom is a live clap alias —
    # it must RUN (bench output on stdout is expected for a success
    # path; only the error paths demand an empty stdout).
    rc, out, err = run(["--charset-custom", "binary", "--bench-frames", "3"], timeout=30)
    global PASS_COUNT, FAIL_COUNT
    ok = rc == 0 and "frames" in out.lower()
    status = "PASS" if ok else "FAIL"
    print(f"  [{status}] {'--charset-custom alias runs':56s} | rc={rc}")
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"--charset-custom alias runs | rc={rc}, out={out[:80]!r}, err={err[:80]!r}")


def part_e_pty_alt_screen():
    print("\n── Part E: PTY alt-screen invariant (startup errors die pre-ALT) ──")
    global PASS_COUNT, FAIL_COUNT
    home = fresh_home()
    env = dict(os.environ)
    env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
    pid, fd = pty.fork()
    if pid == 0:
        os.environ.clear()
        os.environ.update(env)
        os.execv(BIN, [BIN, "--colr"])
        os._exit(127)
    buf = b""
    rc = None
    deadline = time.time() + 10
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                chunk = os.read(fd, 4096)
            except OSError:
                break
            if not chunk:
                break
            buf += chunk
            continue
        p, st = os.waitpid(pid, os.WNOHANG)
        if p:
            rc = os.waitstatus_to_exitcode(st)
            # final drain after child death
            while True:
                r2, _, _ = select.select([fd], [], [], 0.1)
                if not r2:
                    break
                try:
                    chunk = os.read(fd, 4096)
                except OSError:
                    break
                if not chunk:
                    break
                buf += chunk
            break
    os.close(fd)
    if rc is None:
        # EOF before waitpid fired (child died fast): collect status.
        try:
            p, st = os.waitpid(pid, 0)
            rc = os.waitstatus_to_exitcode(st)
        except ChildProcessError:
            rc = "unknown"
    alt_enter = b"\x1b[?1049h"
    problems = []
    if rc != 2:
        problems.append(f"rc={rc}")
    if alt_enter in buf:
        problems.append("ALT-SCREEN ENTERED before dying")
    if b"error" not in buf.lower():
        problems.append("no diagnostic on the tty stream")
    ok = not problems
    status = "PASS" if ok else "FAIL"
    print(f"  [{status}] {'startup error dies before alt-screen':56s} | {'; '.join(problems) if problems else 'rc=2, no ESC[?1049h'}")
    if ok:
        PASS_COUNT += 1
    else:
        FAIL_COUNT += 1
        FAILURES.append(f"PTY alt-screen invariant | {'; '.join(problems)}")


def main():
    if not os.path.exists(BIN):
        sys.exit(f"FATAL: binary not found: {BIN}")
    print(f"depthtest-6 (NIGHT-hunt-46) CLI-flag error streams — binary: {BIN}")
    part_a_typo_sweep()
    part_b_value_matrix()
    part_c_removed_registry()
    part_d_specials()
    part_e_pty_alt_screen()
    print(
        f"\n=== SUMMARY: {PASS_COUNT} PASS / {FAIL_COUNT} FAIL ==="
    )
    if FAILURES:
        print("\nFAILURES:")
        for f in FAILURES:
            print(f"  - {f}")
    sys.exit(1 if FAIL_COUNT else 0)


if __name__ == "__main__":
    main()
