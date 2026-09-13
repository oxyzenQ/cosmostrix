#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).

"""NIGHT-hunt-40: end-to-end verification of the owner's min-1/max-24
entry policy for the four config namespaces (tightened from the
NIGHT-hunt-39 max-64 policy by the owner mandate 2026-09-13):

- [charset-custom.<name>] blocks   (CHARSET_CUSTOM_MAX_BLOCKS = 24)
- [colors-custom.<name>]  blocks   (COLORS_CUSTOM_MAX_BLOCKS   = 24)
- [scene-custom.<name>]   blocks   (SCENE_CUSTOM_MAX_BLOCKS    = 24)
- ambient.<HH-MM>         entries  (AMBIENT_MAX_ENTRIES        = 24)

Boundary contract: 24 entries PASS on every surface (--testconf,
startup, live-reload), 25 entries FAIL with an error that names the
count and the cap. The min-1 side is the NIGHT-hunt-37 completeness
contract (a block with zero field entries is a hard error) - pinned
here too, plus the ambient boundary (1 entry = a valid active
schedule).

Drives the REAL binary (template dump + edit + --testconf). Exit 0 =
all expectations met, exit 1 = at least one failure.

Usage: python3 night_h40_entry_budget_e2e.py [path-to-cosmostrix]
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


def run(home, extra_args, timeout=60):
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


def sandbox():
    home = tempfile.mkdtemp(prefix="h40_home_")
    cfgdir = os.path.join(home, ".config", "cosmostrix")
    os.makedirs(cfgdir, exist_ok=True)
    cfg = os.path.join(cfgdir, "config.toml")
    rc, _out, _err = run(home, ["--dump-config", cfg, "--force"])
    assert os.path.exists(cfg), f"dump-config failed: rc={rc}"
    with open(cfg, "r", encoding="utf-8") as f:
        base = f.read()
    return cfg, base


def check(name, cond, detail=""):
    print(
        f"  [{'PASS' if cond else 'FAIL'}] {name}"
        + (f" — {detail}" if detail and not cond else "")
    )
    if not cond:
        FAILURES.append(f"{name}: {detail}")


def testconf(cfg, text):
    with open(cfg, "w", encoding="utf-8") as f:
        f.write(text)
    return run(
        os.path.dirname(os.path.dirname(os.path.dirname(cfg))),
        ["-s", "--scene", "test", "--testconf"],
    )


CAP = 24


def charset_blocks(n):
    return "".join(f'\n[charset-custom.c{i}]\nset = "x{i}"\n' for i in range(n))


def colors_blocks(n):
    return "".join(
        f'\n[colors-custom.p{i}]\nbg = "#0a0a0a"\nrain = "#111111, #222222"\n'
        for i in range(n)
    )


def scene_blocks(n):
    return "".join(
        f'\n[scene-custom.s{i}]\nrain = "glyph"\ncolor = "green"\n'
        f'charset = "hacker"\nfps = 60\nspeed = 9\ndensity = 0.75\n'
        f'glitch-level = "subtle"\n'
        for i in range(n)
    )


def ambient_entries(n):
    # Valid HH-MM keys: hours 0..23, minutes cycling 0..4 (5/hour).
    return "".join(
        f'ambient.{i // 5:02d}-{i % 5:02d} = "cinematic"\n' for i in range(n)
    )


def main():
    if not os.path.exists(BIN):
        print(f"binary not found: {BIN}")
        return 1
    print(f"binary: {BIN}")

    cases = [
        ("charset-custom", charset_blocks),
        ("colors-custom", colors_blocks),
        ("scene-custom", scene_blocks),
    ]
    for name, gen in cases:
        print(f"\n== {name}: block-count boundary ==")
        cfg, base = sandbox()
        rc, out, err = testconf(cfg, base + gen(CAP))
        check(
            f"{CAP} blocks PASS",
            rc == 0,
            f"rc={rc}, out={out[-300:]}, err={err[-300:]}",
        )
        rc, out, err = testconf(cfg, base + gen(CAP + 1))
        blob = out + err
        check(f"{CAP + 1} blocks FAIL", rc == 2, f"rc={rc}")
        check(
            "error names count and cap",
            f"{CAP + 1} blocks" in blob and f"maximum is {CAP}" in blob,
            blob[-300:],
        )

    print("\n== ambient: entry-count boundary ==")
    cfg, base = sandbox()
    rc, out, err = testconf(cfg, base + "\n" + ambient_entries(CAP))
    check(f"{CAP} entries PASS", rc == 0, f"rc={rc}, out={out[-300:]}")
    rc, out, err = testconf(cfg, base + "\n" + ambient_entries(CAP + 1))
    blob = out + err
    check(f"{CAP + 1} entries FAIL", rc == 2, f"rc={rc}")
    check(
        "error names count and cap",
        f"{CAP + 1} entries" in blob and f"maximum is {CAP}" in blob,
        blob[-300:],
    )
    # min-1 side: a single entry is a valid active schedule.
    rc, out, err = testconf(cfg, base + '\nambient.06-00 = "signal"\n')
    check("1 ambient entry PASS (min-1 when active)", rc == 0, f"rc={rc}")

    print("\n== min-1 side: header-only (zero-entry) blocks are hard errors ==")
    scene_fields = (
        'rain = "glyph"\ncolor = "green"\ncharset = "hacker"\n'
        'fps = 60\nspeed = 9\ndensity = 0.75\nglitch-level = "subtle"'
    )
    for header, field in [
        ("charset-custom.test", 'set = "x"'),
        ("colors-custom.test", 'bg = "#0a0a0a"\nrain = "#111111, #222222"'),
        ("scene-custom.test", scene_fields),
    ]:
        cfg, base = sandbox()
        # Complete block: PASS.
        rc, out, err = testconf(cfg, base + f"\n[{header}]\n{field}\n")
        check(f"complete [{header}] PASS", rc == 0, f"rc={rc}, {out[-300:]}")
        # Zero-entry block: FAIL (completeness = the min-1 rule).
        rc, out, err = testconf(cfg, base + f"\n[{header}]\n")
        blob = out + err
        check(
            f"header-only [{header}] FAIL (min 1 entry)",
            rc == 2 and "incomplete" in blob,
            f"rc={rc}, {blob[-300:]}",
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
