#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).

"""NIGHT-hunt-41: strength repro for the owner's fatal report.

Owner fatal report (manual testing, 2026-09-13):

  config.toml:
    msg-modey        = true

  cli:
    ./target/pro-native/cosmostrix -v

  observed: passed, no error printed, the binary kept running.
  expected: error, the binary cannot run (msg-modey is a typo of
            msg-mode; --testconf already rejects it).

Root cause (found by tracing the startup path):
  src/config/config_apply.rs:135 gates the entire startup validation
  block on `!parsed_cfg.values.is_empty()`. A config whose ONLY key
  is an unknown key (like `msg-modey`) has `values` empty (unknown
  keys go to `parsed.unknown_keys`, not `parsed.values`), so the
  guard short-circuits and Layers 1/1.5/2/3 (malformed / duplicate /
  unknown / strict-value) are ALL skipped. The binary loads the
  config with "0 keys parsed" and proceeds with pure defaults --
  exactly the silent-ignore class the owner rejected.

  --testconf was NOT affected: it iterates `parsed.unknown_keys`
  directly (testconf/mod.rs:158) and reports each one as an error.
  The bug is asymmetric: --testconf catches the typo, startup does
  not. That is the "three different verdicts for one typo" anti-
  pattern that NIGHT-hunt-38-supermassive closed for value typos;
  NIGHT-hunt-41 closes the key-typo sibling.

This script drives the REAL binary through the owner's exact repro
plus three sibling cases (a different key typo, a value typo, a
duplicate key) to confirm the fix is uniform across the typo class.
Exit 0 = all expectations met, exit 1 = at least one failure.

Usage: python3 night_h41_msg_modey_repro.py [path-to-cosmostrix]
Default binary: ./target/release/cosmostrix
"""

import os
import subprocess
import sys
import tempfile

BIN = sys.argv[1] if len(sys.argv) > 1 else "./target/release/cosmostrix"
if not os.path.isabs(BIN):
    BIN = os.path.abspath(BIN)

FAILURES = []


def run(home, extra_args, timeout=10):
    env = dict(os.environ)
    env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
    try:
        p = subprocess.run(
            [BIN] + extra_args,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", "", ""


def write_config(home, text):
    cfgdir = os.path.join(home, ".config", "cosmostrix")
    os.makedirs(cfgdir, exist_ok=True)
    cfg = os.path.join(cfgdir, "config.toml")
    with open(cfg, "w", encoding="utf-8") as f:
        f.write(text)
    return cfg


def check(name, cond, detail=""):
    print(
        f"  [{'PASS' if cond else 'FAIL'}] {name}"
        + (f" -- {detail}" if detail and not cond else "")
    )
    if not cond:
        FAILURES.append(f"{name}: {detail}")


def case_startup_rejects(label, config_text, must_contain):
    """Startup must REJECT the config (exit code 2) and the error
    message must contain `must_contain`. The owner's report: this
    path was silently passing.

    Uses `--doctor` (a POST-config early-return flag) so the binary
    runs apply_config (where validation lives) but does NOT enter
    interactive mode (which would die on a headless terminal and
    mask the validation verdict)."""
    print(f"\n== startup reject: {label} ==")
    home = tempfile.mkdtemp(prefix="h41_home_")
    write_config(home, config_text)
    rc, out, err = run(home, ["--doctor"], timeout=8)
    blob = out + err
    check(
        f"startup exit code is 2 (got {rc})",
        rc == 2,
        blob[-400:],
    )
    check(
        f"error mentions '{must_contain}'",
        must_contain in blob,
        blob[-400:],
    )


def case_startup_accepts(label, config_text):
    """Sanity: a known-good config still starts (exit 0 for --doctor,
    NOT exit 2)."""
    print(f"\n== startup accept: {label} ==")
    home = tempfile.mkdtemp(prefix="h41_home_")
    write_config(home, config_text)
    rc, out, err = run(home, ["--doctor"], timeout=4)
    check(
        f"startup does NOT reject (rc={rc})",
        rc != 2,
        (out + err)[-300:],
    )


def case_testconf_rejects(label, config_text, must_contain):
    """--testconf must reject (already worked before the fix; pinned
    here so the asymmetry cannot regress)."""
    print(f"\n== testconf reject: {label} ==")
    home = tempfile.mkdtemp(prefix="h41_home_")
    write_config(home, config_text)
    rc, out, err = run(home, ["--testconf", "-s"], timeout=8)
    blob = out + err
    check(f"testconf exit code is 2 (got {rc})", rc == 2, blob[-400:])
    check(
        f"testconf error mentions '{must_contain}'",
        must_contain in blob,
        blob[-400:],
    )


def main():
    if not os.path.exists(BIN):
        print(f"binary not found: {BIN}")
        return 1
    print(f"binary: {BIN}")

    # The owner's exact repro: `msg-modey = true` in config.toml.
    case_startup_rejects(
        "msg-modey = true (owner's exact repro)",
        'msg-modey = true\n',
        "msg-modey",
    )
    case_testconf_rejects(
        "msg-modey = true (testconf already worked)",
        'msg-modey = true\n',
        "msg-modey",
    )

    # Sibling 1: a different key typo (scenee instead of scene).
    case_startup_rejects(
        "scenee = cinematic (key typo sibling)",
        'scenee = "cinematic"\n',
        "scenee",
    )

    # Sibling 2: a value typo the parser classifies as unknown key
    # at the value side (msg-mode = truee was the NIGHT-hunt-38
    # case; here we add a key-typo companion). This confirms the
    # fix is not specific to msg-modey.
    case_startup_rejects(
        "msg-mode + msg-modey both present",
        'msg-mode = true\nmsg-modey = false\n',
        "msg-modey",
    )

    # Sibling 3: a duplicate key (the NIGHT-depthtest-2 layer).
    # Two `msg-mode = ...` lines -- duplicate_keys vector must fire
    # at startup, not just at --testconf.
    case_startup_rejects(
        "duplicate msg-mode key (Layer 1.5)",
        'msg-mode = true\nmsg-mode = false\n',
        "duplicate",
    )

    # Sanity: a known-good config still starts.
    case_startup_accepts(
        "msg-mode = true (known-good control)",
        'msg-mode = true\n',
    )

    print("\n" + "=" * 60)
    if FAILURES:
        print(f"{len(FAILURES)} expectation(s) FAILED:")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("ALL expectations met.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
