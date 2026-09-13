#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).

"""NIGHT-hunt-38-supermassive: strength repro for the testconf parser bug class.

Owner fatal report (commit 6198431, manual testing): four config typo
classes produced stale / inconsistent / garbage diagnostics:

1. msg-mode = truee  -> --testconf PASS (no bool validator); runtime
   printed a one-line error but KEPT RUNNING (inconsistent verdicts).
2. [charset-custom.test] with `set : "x"` -> two cascaded errors
   (malformed line + block incomplete) - noisy but at least rejected.
3. [charset-custom.test] with `set == "x"` -> SILENTLY ACCEPTED (0
   errors, PASS): the `==` typo stored `= "x"` as the charset content
   (garbage glyphs).
4. ambient.06-00 == "signal" -> rejected with a STALE "legacy
   multi-field format" migration essay for a format the user never
   used (the value reached the validator as `= "signal"`).

This script drives the REAL binary (default config surface, no unit-test
shims) against all four cases plus the runtime-verdict consistency check
and reports the actual behavior. Exit 0 = all expectations met
(post-fix), exit 1 = at least one expectation failed.

Usage: python3 night_h38_supermassive_testconf_repro.py [path-to-cosmostrix]
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


def run(binary, home, extra_args, timeout=30):
    """Run the binary with a sandboxed HOME; return (rc, stdout, stderr)."""
    env = dict(os.environ)
    env["HOME"] = home
    env.pop("XDG_CONFIG_HOME", None)
    env["TERM"] = "xterm-256color"
    try:
        p = subprocess.run(
            [binary] + extra_args,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", "", ""


def sandbox():
    """Fresh HOME with the stock template config already written."""
    home = tempfile.mkdtemp(prefix="h38_home_")
    cfgdir = os.path.join(home, ".config", "cosmostrix")
    os.makedirs(cfgdir, exist_ok=True)
    cfg = os.path.join(cfgdir, "config.toml")
    rc, _out, err = run(BIN, home, ["--dump-config", cfg, "--force"])
    assert os.path.exists(cfg), f"dump-config failed: rc={rc} err={err}"
    return home, cfg


def read(cfg):
    with open(cfg, "r", encoding="utf-8") as f:
        return f.read()


def write(cfg, text):
    with open(cfg, "w", encoding="utf-8") as f:
        f.write(text)


def check(name, cond, detail=""):
    status = "PASS" if cond else "FAIL"
    print(f"  [{status}] {name}" + (f" — {detail}" if detail and not cond else ""))
    if not cond:
        FAILURES.append(f"{name}: {detail}")


def case_msg_mode_truee():
    print("\n== case 1: msg-mode = truee (bool typo) ==")
    home, cfg = sandbox()
    text = read(cfg)
    write(cfg, text.replace("# msg-mode        = true", "msg-mode        = truee"))
    rc, out, err = run(BIN, home, ["-v", "-s", "--scene", "test", "--testconf"])
    print(f"  --testconf rc={rc}")
    print(f"  stdout tail: {out.strip().splitlines()[-1] if out.strip() else '(none)'}")
    passed = rc == 0
    check(
        "testconf must REJECT msg-mode = truee (bool vocabulary)",
        not passed,
        f"rc={rc}, out={out[-400:]}",
    )
    # Expect the error to NAME the key and the expected vocabulary.
    blob = out + err
    check(
        "error names msg-mode and expected true/false",
        passed or ("msg-mode" in blob and ("true" in blob and "false" in blob)),
        f"blob={blob[-400:]}",
    )


def case_set_colon():
    print('\n== case 2: [charset-custom.test] set : "x" (colon separator) ==')
    home, cfg = sandbox()
    text = read(cfg)
    block = '[charset-custom.test]\n set : "x"\n'
    write(cfg, text + "\n" + block)
    rc, out, err = run(BIN, home, ["-v", "-s", "--scene", "test", "--testconf"])
    print(f"  --testconf rc={rc}")
    blob = out + err
    check(
        "testconf must reject (colon is not a TOML separator)",
        rc != 0,
        f"rc={rc}, out={out[-400:]}",
    )
    check(
        "malformed-line diagnostic mentions '=' separator syntax",
        "malformed line" in blob or "key = value" in blob,
        blob[-400:],
    )


def case_set_double_equals():
    print('\n== case 3: [charset-custom.test] set == "x" (double equals) ==')
    home, cfg = sandbox()
    text = read(cfg)
    block = '[charset-custom.test]\n set == "x"\n'
    write(cfg, text + "\n" + block)
    rc, out, err = run(BIN, home, ["-v", "-s", "--scene", "test", "--testconf"])
    print(f"  --testconf rc={rc}")
    blob = out + err
    check(
        'testconf must REJECT set == "x" (silent accept = stale bug)',
        rc != 0,
        f"rc={rc} — silently accepted, out={out[-300:]}",
    )
    check(
        "diagnostic flags the double-'=' typo",
        "==" in blob or "double" in blob.lower(),
        blob[-400:],
    )


def case_ambient_single_name():
    print('\n== case 4: ambient.06-00 = "signal" (valid single-scene name) ==')
    home, cfg = sandbox()
    text = read(cfg)
    write(cfg, text.replace('# ambient.06-00 = "signal"', ' ambient.06-00 = "signal"'))
    rc, out, err = run(BIN, home, ["-v", "-s", "--scene", "test", "--testconf"])
    print(f"  --testconf rc={rc}")
    blob = out + err
    check(
        "testconf must ACCEPT the documented single-scene-name format",
        rc == 0,
        f"rc={rc}",
    )
    check(
        "no stale 'legacy multi-field' essay for a valid entry",
        "legacy multi-field" not in blob,
        blob[-500:] if "legacy" in blob else "",
    )
    # Also: the == typo on an ambient key should be a MALFORMED LINE error,
    # not a stale legacy-format migration essay.
    home2, cfg2 = sandbox()
    write(
        cfg2,
        read(cfg2).replace('# ambient.06-00 = "signal"', ' ambient.06-00 == "signal"'),
    )
    rc2, out2, err2 = run(BIN, home2, ["-v", "-s", "--scene", "test", "--testconf"])
    blob2 = out2 + err2
    print(f"  (== typo variant) rc={rc2}")
    check(
        "ambient == typo rejected as syntax error, not legacy-format essay",
        rc2 != 0 and "legacy multi-field" not in blob2,
        f"rc={rc2}, blob={blob2[-400:]}",
    )


def case_runtime_consistency():
    print("\n== case 5: runtime verdict matches testconf verdict (msg-mode) ==")
    home, cfg = sandbox()
    write(cfg, read(cfg).replace("# msg-mode        = true", "msg-mode        = truee"))
    # Startup validation path (no --testconf). -s + bench-ish flags keep it
    # non-interactive; startup validation runs before any render loop.
    rc, out, err = run(BIN, home, ["-v", "-s", "--scene", "test", "--duration", "1"])
    blob = out + err
    print(f"  runtime rc={rc}")
    check(
        "startup must REJECT msg-mode = truee (exit 2, uniform contract)",
        rc == 2,
        f"rc={rc}, blob={blob[-400:]}",
    )


def main():
    if not os.path.exists(BIN):
        print(f"binary not found: {BIN}")
        return 1
    print(f"binary: {BIN}")
    case_msg_mode_truee()
    case_set_colon()
    case_set_double_equals()
    case_ambient_single_name()
    case_runtime_consistency()
    print(f"\n{'=' * 60}")
    if FAILURES:
        print(f"{len(FAILURES)} expectation(s) FAILED:")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("ALL expectations met.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
