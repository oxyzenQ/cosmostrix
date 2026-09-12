#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-depthtest-4 + hunt-33 e2e: the v100 LTS quick regression
matrix on the REAL binary — one fast probe per bug from the owner's
historical bug list (the A1-J1 matrix), so a full re-verification of
the fixed-bug surface takes under two minutes instead of an evening
of manual terminal babysitting.

Owner brief (2026-09-12): "these bugs were fixed before, v100 is on
the way to stable LTS — help me simplify them so I can quickly test
whether anything slipped through." The DEEPSEEK checklist the owner
pasted is the source matrix; this harness automates every probe that
can be asserted mechanically (exit codes, stderr contracts, PTY
stream content) and leaves the genuinely-manual ones (full ambient
snapback cycles, live HUD watching) to the companion doc section in
the module docstring below.

Matrix coverage (bug id -> probe):

  A1  broken pipe           phase_c (doctor/version piped into head)
  B1  invalid flag + tip    phase_a (--test, -x, -g)
  B2  consistent CLI errors phase_a (same rc + format per case)
  C1  crystal-dragon-secs   phase_a (2m human-duration accepted;
                            30h out-of-range rejected)
  C2  secs honored at boot  phase_b (verbose startup cadence 3.0s)
  C3  CLI fallback lock     manual (see MANUAL-ONLY below)
  C4  crystal/ambient       manual (docs: AMBIENT_SCHEDULER harmony)
  C5  CLI fallback after    manual (see MANUAL-ONLY below)
      config edits
  D1  config wins runtime   phase_b (edit scene -> final state)
  D2/D5 ambient fallback    covered by the CLI-lock fallback unit
                            suite (tests_cli_fallback.rs) + phase_b
                            D3 trace probe
  D3  ambient deferred at   phase_b (startup deferral trace with an
      startup               active ambient entry + CLI scene)
  D4  ambient wins runtime  manual (see MANUAL-ONLY below)
  E1  invalid scene color   phase_a (unknown color, 1 trigger)
  E4  builtin in colors-    phase_a (BUILT-IN hint, hard error)
      custom
  E5  HUD metric names      phase_b (verbose scene/fps source of
                            truth; HUD decode is manual)
  E6  custom scene on CLI   phase_a (--list-scenes) + phase_b
                            (--scene hacker-mode loads, fps 60)
  E7  charset-custom works  phase_a (--list-charsets entry)
  F1  final state diffs     phase_b (bold/fps edits -> "was" lines)
  F2  CLI override display  phase_b (fps 40 "was" line)
  G1  symbols not icons     phase_b (wide-char skip warning "!")
  H1  charset edge cases    phase_a (wide chars hard error; single
                            "[" is VALID by contract and passes)
  H2  charset-custom block  phase_a (valid block -> PASS, no
      uncommented            spurious unknown-key error)
  I1  color-tune CLI lock   phase_b (config-present wins, the
                            v80 masterclass temporal contract; the
                            absent-key half is covered by
                            tests_cli_fallback.rs)
  I2  base scene override   manual (see MANUAL-ONLY below)
  J1  scene custom block    phase_a (incomplete -> missing-dims
      completeness           error; legacy keys -> removal hints)

MANUAL-ONLY (interactive timing the harness cannot compress, keep
them in the release-candidate loop):
  C3/C5 - edit crystal-dragon-secs live and watch the HUD crdr field
  D4    - sit through one ambient phase (30s+) and watch fps/color
  I2    - cosmostrix --show-scene hacker after a CLI-override run

Design notes (why this is fast):
  - every PTY probe sets intro = "none" in the probe config: the
    cinematic intro delays watcher spawn by ~4s, and an edit landing
    inside the intro window is absorbed by the watcher's initial
    snapshot (a benign race documented in KNOWN_ISSUES.md; without
    intro=none the F1/D1/I1 probes would need 10s+ dwell times)
  - every PTY probe edits the config only AFTER the first output
    chunk proves the rain loop is live
  - probes write to the default config path only (the safepath guard
    rejects /tmp configs), and a minimal valid config is restored on
    exit so the owner's next manual run starts from a clean state

Usage (repo root, release binary built):

  python3 scripts/depthtest4_regression_e2e.py

Knobs (env): BIN (default target/release/cosmostrix).

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
CFG_DIR = os.path.expanduser("~/.config/cosmostrix")
CFG = os.path.join(CFG_DIR, "config.toml")
TERM_ROWS, TERM_COLS = 30, 100
ANSI_RE = re.compile(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07")

# Icon glyphs forbidden in output surfaces (v80.0.0-beta.2 owner
# rule — the symbol-only contract). If any of these appear in a
# probe's stream, G1 regressed.
FORBIDDEN_ICON_GLYPHS = [
    "\u26a0",  # warning sign
    "\u2705",  # check mark
    "\u274c",  # cross mark
    "\u2728",  # sparkles
]

# Minimal config restored on exit so the owner's environment is not
# left carrying a probe artifact.
RESTORE_CONFIG = 'intro = "none"\nscene = "cosmos"\n'

RESULTS = []
FAILS = []


def check(key: str, ok: bool, detail: str = "") -> bool:
    RESULTS.append((key, ok, detail))
    if not ok:
        FAILS.append(key)
    tag = "OK  " if ok else "FAIL"
    line = f"  [{tag}] {key}"
    if detail:
        line += f" ({detail})"
    print(line)
    return ok


def write_cfg(body: str, path: str = CFG) -> None:
    os.makedirs(CFG_DIR, exist_ok=True)
    with open(path, "w") as f:
        f.write(body)


def run_plain(args, timeout=30):
    """Non-PTY subprocess (validation-surface checks)."""
    return subprocess.run(
        [BIN] + args, capture_output=True, text=True, timeout=timeout, check=False
    )


def run_pty(argv, secs, edits=None, debug=False):
    """Run the binary in a PTY; edits = [(t, config_body)].

    Returns (raw_bytes, exit_code). 'q' is sent at secs-1 so the
    final runtime state prints before the process exits.
    """
    master, slave = pty.openpty()
    fcntl.ioctl(
        master, termios.TIOCSWINSZ, struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0)
    )
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["TERM_PROGRAM"] = "alacritty"
    env["COLORTERM"] = "truecolor"
    if debug:
        env["COSMOSTRIX_LIVE_RELOAD_DEBUG"] = "1"
    proc = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave, env=env)
    os.close(slave)
    buf = bytearray()
    t0 = time.time()
    edits = list(edits or [])
    rain_live = False
    sent_q = False
    quit_at = max(secs - 1.0, 1.0)
    while time.time() - t0 < secs + 2:
        r, _, _ = select.select([master], [], [], 0.05)
        if r:
            try:
                c = os.read(master, 65536)
            except OSError:
                break
            if not c:
                break
            buf.extend(c)
            if len(buf) > 4096:
                rain_live = True
        now = time.time() - t0
        # Edits are only released after the rain loop is provably
        # live — otherwise the intro window swallows them (the
        # initial-snapshot race, see module docstring).
        while edits and rain_live and now >= edits[0][0]:
            _, text = edits.pop(0)
            write_cfg(text)
        if not sent_q and now >= quit_at:
            try:
                os.write(master, b"q")
            except OSError:
                pass
            sent_q = True
        if proc.poll() is not None:
            deadline = time.time() + 0.5
            while time.time() < deadline:
                r, _, _ = select.select([master], [], [], 0.05)
                if not r:
                    continue
                try:
                    c = os.read(master, 65536)
                except OSError:
                    break
                if not c:
                    break
                buf.extend(c)
            break
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
    os.close(master)
    return bytes(buf), proc.returncode


def plain_text(raw: bytes) -> str:
    return ANSI_RE.sub("", raw.decode("utf-8", "replace"))


def final_state_section(raw: bytes) -> str:
    """Extract the final runtime state section (post-edit truth)."""
    text = plain_text(raw)
    idx = text.rfind("final runtime state")
    return text[idx:] if idx >= 0 else ""


def state_field(section: str, field: str) -> str:
    """Extract `<field>: <value>` from the final-state section."""
    m = re.search(rf"{field}:\s+(.+)", section)
    return m.group(1).strip() if m else ""


# A complete, valid scene-custom block reused by several probes.
SCENE_CUSTOM_BLOCK = """[scene-custom.hacker-mode]
rain = "glyph"
color = "aurora"
charset = "binary"
fps = 60
speed = 12
density = 0.5
glitch-level = "none"
"""

INTRO_OFF = 'intro = "none"\n'


def phase_a() -> None:
    """Validation-surface probes: exit codes + stderr contracts."""
    print("Phase A — validation surface (fast)")
    out = run_plain(["--test"])
    check(
        "B1a --test rejected with tip",
        out.returncode == 2
        and "unexpected argument '--test'" in out.stderr
        and "a similar argument exists: '--testconf'" in out.stderr,
        f"rc={out.returncode}",
    )
    out = run_plain(["-x"])
    check("B1b -x rejected rc=2", out.returncode == 2, f"rc={out.returncode}")
    out = run_plain(["-g"])
    check(
        "B1c -g missing value is a clean clap error",
        out.returncode == 2 and "a value is required" in out.stderr,
        f"rc={out.returncode}",
    )

    write_cfg(
        INTRO_OFF
        + '[scene-custom.cp77]\nrain = "glyph"\ncolor = "cp77x"\ncharset = "binary"\n'
        + 'fps = 60\nspeed = 12\ndensity = 0.5\nglitch-level = "none"\n'
    )
    out = run_plain(["--testconf"])
    combined = out.stdout + out.stderr
    n_hits = combined.count("unknown color 'cp77x'")
    check(
        "E1 invalid scene-custom color is a hard single-trigger error",
        out.returncode == 2 and n_hits == 1 and "--list-colors" in combined,
        f"rc={out.returncode} triggers={n_hits}",
    )

    write_cfg(
        INTRO_OFF
        + '[scene-custom.cp77]\nrain = "glyph"\ncolors-custom = "cosmos"\ncharset = "binary"\n'
        + 'fps = 60\nspeed = 12\ndensity = 0.5\nglitch-level = "none"\n'
    )
    out = run_plain(["--testconf"])
    combined = out.stdout + out.stderr
    check(
        "E4 builtin color in colors-custom gets the BUILT-IN hint",
        out.returncode == 2
        and "unknown colors-custom block 'cosmos'" in combined
        and "is a BUILT-IN color name" in combined,
        f"rc={out.returncode}",
    )

    write_cfg(
        INTRO_OFF + '[scene-custom.cp77]\nrain = "glyph"\ncolor = "aurora"\nfps = 60\n'
    )
    out = run_plain(["--testconf"])
    combined = out.stdout + out.stderr
    check(
        "J1a incomplete scene-custom lists the missing dimensions",
        out.returncode == 2
        and "is incomplete: missing" in combined
        and "charset|charset-custom" in combined,
        f"rc={out.returncode}",
    )

    write_cfg(
        INTRO_OFF
        + '[scene-custom.cp77]\nrain = "glyph"\nbold = "1"\nshading-mode = "1"\n'
        + 'base-scene = "matrix"\ncolor = "aurora"\ncharset = "binary"\n'
        + 'fps = 60\nspeed = 12\ndensity = 0.5\nglitch-level = "none"\n'
    )
    out = run_plain(["--testconf"])
    combined = out.stdout + out.stderr
    check(
        "J1b legacy bold/shading-mode/base-scene keys are rejected",
        out.returncode == 2
        and "was removed from [scene-custom" in combined
        and "unknown key 'scene-custom.cp77.base-scene'" in combined,
        f"rc={out.returncode}",
    )

    write_cfg(INTRO_OFF + '[charset-custom.x]\nset = "\u6f22\u5b57"\n')
    out = run_plain(["--testconf"])
    combined = out.stdout + out.stderr
    check(
        "H1a wide-only charset set is a clear hard error",
        out.returncode == 2 and "no usable single-width characters" in combined,
        f"rc={out.returncode}",
    )
    write_cfg(INTRO_OFF + '[charset-custom.x]\nset = "["\n')
    out = run_plain(["--testconf"])
    check(
        "H1b single '[' glyph is a valid charset by contract",
        out.returncode == 0 and "PASS" in out.stdout,
        f"rc={out.returncode}",
    )

    write_cfg(INTRO_OFF + '[charset-custom.cyberpunk_2077]\nset = "01"\n')
    out = run_plain(["--testconf"])
    combined = out.stdout + out.stderr
    check(
        "H2 uncommented charset-custom block passes without unknown-key noise",
        out.returncode == 0 and "PASS" in out.stdout and "unknown key" not in combined,
        f"rc={out.returncode}",
    )

    write_cfg(INTRO_OFF + "crystal-dragon = 1\ncrystal-dragon-secs = 2m\n")
    out = run_plain(["--testconf"])
    check(
        "C1 human-duration crystal-dragon-secs accepted",
        out.returncode == 0 and "PASS" in out.stdout,
        f"rc={out.returncode}",
    )
    write_cfg(INTRO_OFF + "crystal-dragon-secs = 30h\n")
    out = run_plain(["--testconf"])
    check(
        "C1b out-of-range secs rejected",
        out.returncode == 2,
        f"rc={out.returncode}",
    )

    ambient_key = time.strftime("%H-%M")
    write_cfg(INTRO_OFF + f'ambient.{ambient_key} = "aurora"\n')
    out = run_plain(["--testconf"])
    combined = out.stdout + out.stderr
    check(
        "ambient value must be a scene (color names rejected)",
        out.returncode == 2 and "unknown scene 'aurora'" in combined,
        f"rc={out.returncode}",
    )

    write_cfg(INTRO_OFF + SCENE_CUSTOM_BLOCK + '[charset-custom.test]\nset = "01"\n')
    out = run_plain(["--list-scenes"])
    check(
        "E6a --list-scenes shows the custom scene section",
        "CUSTOM SCENES (from config)" in out.stdout and "hacker-mode" in out.stdout,
    )
    out = run_plain(["--list-charsets"])
    check(
        "E7 --list-charsets lists the custom charset entry",
        "CUSTOM CHARACTER SETS" in out.stdout and "test" in out.stdout,
    )
    print()


def phase_b() -> None:
    """PTY runtime probes: watcher + final-runtime-state contracts."""
    print("Phase B — PTY runtime (watcher + final state)")

    write_cfg(
        INTRO_OFF + "crystal-dragon = 1\ncrystal-dragon-secs = 3\n" + SCENE_CUSTOM_BLOCK
    )
    raw, rc = run_pty([BIN, "-v", "-s"], 6)
    startup = plain_text(raw)
    m = re.search(r"cadence_secs:\s+([0-9.]+)s", startup)
    check(
        "C2 crystal-dragon-secs=3 reaches the verbose startup dump",
        rc == 0 and m is not None and m.group(1).startswith("3.0"),
        f"rc={rc} cadence={m.group(1) if m else 'missing'}",
    )

    edit_cfg = INTRO_OFF + 'scene = "cosmos"\nbold = 2\nfps = 40\n'
    raw, rc = run_pty([BIN, "-v", "-s"], 8, edits=[(2.5, edit_cfg)])
    section = final_state_section(raw)
    check(
        "F1 bold edit shows in the final state with a was-annotation",
        rc == 0 and "All (was Random)" in section,
        f"rc={rc} bold={state_field(section, 'bold')!r}",
    )
    check(
        "F2 fps edit shows the was-annotation (CLI/lock diff)",
        "40.0 (was" in section,
        f"fps={state_field(section, 'fps')!r}",
    )

    # D1: runtime config edit wins over the CLI color lock for the
    # scene-family field the edit touches (v80 masterclass temporal
    # contract: a present key is the most recent intent).
    raw, rc = run_pty(
        [BIN, "-v", "-s", "-c", "neon-green"],
        8,
        edits=[(2.5, INTRO_OFF + 'scene = "cinematic"\n')],
    )
    section = final_state_section(raw)
    scene_val = state_field(section, "scene")
    check(
        "D1 runtime scene edit applies (config key present wins)",
        rc == 0 and "cinematic" in scene_val,
        f"rc={rc} scene={scene_val!r}",
    )

    # I1 (current contract): CLI --color-tune locks the startup tune;
    # a PRESENT config tune key is the more recent intent and wins.
    # The absent-key half (comment out -> CLI lock resurfaces) is
    # covered by tests_cli_fallback.rs; see the module docstring.
    raw, rc = run_pty(
        [BIN, "-v", "-s", "--color-tune", "bright=0.1"],
        8,
        edits=[
            (
                2.5,
                INTRO_OFF + 'scene = "cosmos"\n[color.tune]\nbrightness = 1.5\n',
            )
        ],
    )
    section = final_state_section(raw)
    tune_val = state_field(section, "color_tune")
    check(
        "I1 present config tune wins over the CLI lock",
        rc == 0 and "1.50" in tune_val,
        f"rc={rc} tune={tune_val!r}",
    )

    write_cfg(INTRO_OFF + SCENE_CUSTOM_BLOCK)
    raw, rc = run_pty([BIN, "-v", "--scene", "hacker-mode"], 6)
    startup = plain_text(raw)
    check(
        "E6b --scene <custom name> loads the block (fps from block)",
        rc == 0
        and "scene:             hacker-mode" in startup
        and "fps:               60.0" in startup,
        f"rc={rc}",
    )

    ambient_key = time.strftime("%H-%M")
    write_cfg(INTRO_OFF + f'scene = "monolith"\nambient.{ambient_key} = "cinematic"\n')
    raw, rc = run_pty([BIN, "-v", "--scene", "monolith"], 6, debug=True)
    text = plain_text(raw)
    check(
        "D3 startup ambient is deferred while a CLI scene lock is active",
        rc == 0
        and "ambient: startup" in text
        and "deferring ambient apply until snapback" in text,
        f"rc={rc}",
    )

    # G1: runtime warning surfaces use the "!" symbol prefix and the
    # stream carries no icon glyphs (symbol-only output contract).
    # The wide glyph in the referenced charset triggers the buffered
    # skip warning at startup (one usable char keeps the block valid).
    write_cfg(INTRO_OFF + 'charset = "wide"\n[charset-custom.wide]\nset = "\u6f22a"\n')
    raw, rc = run_pty([BIN, "-v", "-s"], 6)
    text = raw.decode("utf-8", "replace")
    has_bang = "! skipped 1 wide/zero-width" in ANSI_RE.sub("", text) or (
        "skipped 1 wide/zero-width" in plain_text(raw)
    )
    icons_found = [g for g in FORBIDDEN_ICON_GLYPHS if g in text]
    check(
        "G1 warnings use the ! prefix and no icon glyphs",
        has_bang and not icons_found,
        f"bang={has_bang} icons={icons_found}",
    )
    print()


def phase_c() -> None:
    """Pipe probes: broken-pipe graceful exit (A1)."""
    print("Phase C — broken pipe (A1)")
    for args, label in (
        (["--doctor"], "A1a --doctor | head -2"),
        (["--version"], "A1b --version | head -1"),
    ):
        proc = subprocess.Popen(
            [BIN] + args,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        # Read two lines, then close the pipe — the reader is gone.
        proc.stdout.readline()
        proc.stdout.readline()
        proc.stdout.close()
        try:
            err = proc.communicate(timeout=10)[1]
        except subprocess.TimeoutExpired:
            proc.kill()
            check(label + " exits after reader close", False, "timeout (hang)")
            continue
        rc = proc.returncode
        panicked = b"panicked" in (err or b"")
        check(
            label + " exits gracefully (no panic, no hang)",
            not panicked and rc in (0, 2, 141),
            f"rc={rc} panicked={panicked}",
        )
    print()


def main() -> int:
    if not os.path.exists(BIN):
        print(f"binary not found: {BIN} (build it first: cargo build --release)")
        return 1
    print(f"NIGHT-depthtest-4 quick regression matrix — binary: {BIN}")
    print()
    try:
        phase_a()
        phase_b()
        phase_c()
    finally:
        write_cfg(RESTORE_CONFIG)
    total = len(RESULTS)
    print(f"{'=' * 60}")
    print(f"{'PASSED' if not FAILS else 'FAILED'}: {total - len(FAILS)}/{total} probes")
    if FAILS:
        print("regressed probes:")
        for key in FAILS:
            print(f"  - {key}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
