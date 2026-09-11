#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-depthtest-2 e2e: the priority + duplicate-name contracts on
the REAL binary through a PTY (the ANSI stream's 24-bit color values
are the source of truth; no in-process shortcuts).

Contract under test (docs/LIVE_RELOAD_BEHAVIOR.md S-master-LOGIC-3):

  Startup:  CLI flags > config.toml
  Runtime:  user shortkeys > config keys > CLI locks
  Duplicates: duplicate [charset-custom.x] / [colors-custom.x] /
  [scene-custom.x] headers reject on all three surfaces (--testconf
  exit 2, startup exit 2, live-reload watcher reject + exit 2).

Scenarios:

  1. cli_wins_startup:    config color=blue, CLI -c red -> RED.
  2. config_wins_runtime: mid-run edit color=gold -> GOLD (beats the
                          locked CLI red).
  3. shortkey_wins_runtime: 'C' press cycles the scheme at runtime
                          (blue -> warm family). 'C' (backwards) is
                          used because blue's forward neighbor (cyan)
                          shares the blue channel signature; the
                          backward hop is an unambiguous family swap.
  4. duplicates_e2e:      the three custom namespaces x the three
                          validation surfaces.

The hue classifier averages BRIGHT emitted fg colors (head-level
cells, R+G+B > 250) per time window; the shading ladder blends
between palette stops, so channel RATIOS (not absolute values) carry
the family signature: red G ~ B, gold G >> B, blue B > G > R.

Usage (repo root, release binary built):

  python3 scripts/depthtest2_cli_config_e2e.py

Knobs (env): BIN (default target/release/cosmostrix). The harness
writes ~/.config/cosmostrix/config.toml and restores a valid config
on exit.

Exit 0 = all expectations met; exit 1 = at least one regressed.
"""

import fcntl
import os
import pty
import re
import select
import struct
import subprocess
import sys
import termios
import time

BIN = os.environ.get("BIN", "target/release/cosmostrix")
CFG = os.path.expanduser("~/.config/cosmostrix/config.toml")
TERM_COLS, TERM_ROWS = 120, 40

GOOD_CFG = 'color = "blue"\ncharset = "katakana"\nfps = 60\n'

RGB_RE = re.compile(rb"\x1b\[38;2;(\d+);(\d+);(\d+)")


def write_cfg(body: str) -> None:
    with open(CFG, "w") as f:
        f.write(body)


def run_pty(argv, secs, edits=None, keys=None):
    """Run the binary in a PTY; edits = [(t, text)], keys = [(t, byte)].

    Returns (raw_bytes, timed_chunks, exit_code) where timed_chunks is
    a list of (t, chunk) arrival stamps.
    """
    master, slave = pty.openpty()
    fcntl.ioctl(
        master,
        termios.TIOCSWINSZ,
        struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0),
    )
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["TERM_PROGRAM"] = "alacritty"
    env["COLORTERM"] = "truecolor"
    proc = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave, env=env)
    os.close(slave)
    buf = bytearray()
    chunks = []
    t0 = time.time()
    edits = list(edits or [])
    keys = list(keys or [])
    while time.time() - t0 < secs + 2:
        r, _, _ = select.select([master], [], [], 0.05)
        if r:
            try:
                c = os.read(master, 65536)
            except OSError:
                break
            if not c:
                break
            chunks.append((time.time() - t0, bytes(c)))
            buf.extend(c)
        now = time.time() - t0
        while edits and now >= edits[0][0]:
            _, text = edits.pop(0)
            write_cfg(text)
        while keys and now >= keys[0][0]:
            _, k = keys.pop(0)
            try:
                os.write(master, k)
            except OSError:
                pass
        if proc.poll() is not None:
            break
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
    os.close(master)
    return bytes(buf), chunks, proc.returncode


def hue_stats(chunks, t_from, t_to):
    """Average R,G,B over BRIGHT emitted fg colors in [t_from, t_to)."""
    rs = gs = bs = n = 0
    for t, c in chunks:
        if t_from <= t < t_to:
            for m in RGB_RE.finditer(c):
                r, g, b = int(m.group(1)), int(m.group(2)), int(m.group(3))
                if r + g + b > 250:
                    rs += r
                    gs += g
                    bs += b
                    n += 1
    if n == 0:
        return None, 0
    return (rs / n, gs / n, bs / n), n


def classify(rgb):
    """Classify a bright-cell average into a scheme family."""
    if rgb is None:
        return "none"
    r, g, b = rgb
    if b > r and b > g:
        return "blue"
    if r > g and r > b:
        if g / max(b, 1) > 1.2:
            return "gold"
        return "red"
    return f"other({r:.0f},{g:.0f},{b:.0f})"


def hue_label(rgb):
    if rgb is None:
        return "none"
    r, g, b = rgb
    return f"({r:.0f},{g:.0f},{b:.0f})"


def run_testconf(body: str, label: str) -> bool:
    write_cfg(body)
    r = subprocess.run(
        [BIN, "--config", CFG, "--testconf"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    out = r.stdout + r.stderr
    ok = r.returncode == 2 and "duplicate" in out.lower()
    print(f"scenario4 testconf duplicate [{label}]: exit={r.returncode} ok={ok}")
    return ok


def run_startup(body: str, label: str) -> bool:
    write_cfg(body)
    r = subprocess.run(
        [BIN, "--config", CFG, "--intro", "none", "--duration", "1"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    out = r.stdout + r.stderr
    ok = r.returncode == 2 and "duplicate" in out.lower()
    print(f"scenario4 startup duplicate [{label}]: exit={r.returncode} ok={ok}")
    return ok


def main() -> int:
    results = {}

    # Scenario 1 + 2: one session, startup priority then runtime
    # config priority.
    write_cfg(GOOD_CFG)
    argv = [BIN, "--config", CFG, "--intro", "none", "--duration", "7", "-c", "red"]
    _raw, chunks, rc = run_pty(
        argv, 9, edits=[(4.0, 'color = "gold"\ncharset = "katakana"\nfps = 60\n')]
    )
    rgb1, n1 = hue_stats(chunks, 1.5, 3.5)
    rgb2, n2 = hue_stats(chunks, 5.5, 7.5)
    h1, h2 = classify(rgb1), classify(rgb2)
    print(
        f"scenario1 CLI-wins-startup: hue@1.5-3.5s = {h1} {hue_label(rgb1)} ({n1} cells)"
    )
    print(
        f"scenario2 config-wins-runtime: hue@5.5-7.5s = {h2} {hue_label(rgb2)} ({n2} cells)"
    )
    results["cli_wins_startup_red"] = h1 == "red"
    results["config_wins_runtime_gold"] = h2 == "gold"
    results["session_exit_zero"] = rc == 0

    # Scenario 3: shortkey at runtime beats the config's blue.
    write_cfg(GOOD_CFG)
    argv = [BIN, "--config", CFG, "--intro", "none", "--duration", "6"]
    _raw, chunks, rc = run_pty(argv, 8, keys=[(3.0, b"C")])
    rgb_a, _ = hue_stats(chunks, 1.0, 2.8)
    rgb_b, _ = hue_stats(chunks, 4.2, 6.0)
    h_a, h_b = classify(rgb_a), classify(rgb_b)
    print(
        f"scenario3 shortkey-wins-runtime: before={h_a} after={h_b} {hue_label(rgb_b)}"
    )
    results["shortkey_before_blue"] = h_a == "blue"
    results["shortkey_changed_scheme"] = h_a != h_b and h_b in ("red", "gold")
    results["shortkey_session_exit_zero"] = rc == 0

    # Scenario 4: duplicate-name rejection, three namespaces x three
    # surfaces.
    dups = {
        "charset-custom": '[charset-custom.dup]\nset = "01"\n[charset-custom.dup]\nset = "10"\n',
        "colors-custom": (
            '[colors-custom.dup]\nbg = "#000000"\nrain = "#ffffff"\n'
            '[colors-custom.dup]\nbg = "#101010"\nrain = "#f0f0f0"\n'
        ),
        "scene-custom": (
            '[scene-custom.dup]\nrain = "glyph"\n[scene-custom.dup]\nrain = "vortex"\n'
        ),
    }
    for ns, body in dups.items():
        results[f"testconf_dup_{ns}"] = run_testconf(body + GOOD_CFG, f"{ns}.dup")
        results[f"startup_dup_{ns}"] = run_startup(body + GOOD_CFG, f"{ns}.dup")

    # Live-reload duplicate edit: valid start, duplicate mid-run.
    write_cfg(GOOD_CFG)
    argv = [BIN, "--config", CFG, "--intro", "none", "--duration", "7"]
    _raw, _chunks, rc = run_pty(argv, 9, edits=[(3.0, dups["scene-custom"] + GOOD_CFG)])
    results["live_reload_dup_rejected"] = rc == 2
    print(
        f"scenario4 live-reload duplicate edit: exit={rc} (2 = reject+exit per contract)"
    )

    print("\n=== NIGHT-depthtest-2 E2E SUMMARY ===")
    fails = [k for k, v in results.items() if not v]
    for k, v in results.items():
        print(f"  {'PASS' if v else 'FAIL'}: {k}")
    write_cfg(GOOD_CFG)
    if fails:
        print(f"FAILED: {len(fails)}")
        return 1
    print("ALL E2E EXPECTATIONS MET")
    return 0


if __name__ == "__main__":
    sys.exit(main())
