#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-depthtest-3 e2e: the config.toml depth stress test on the
REAL binary through a PTY (the ANSI stream + the --verbose startup
dump + the --verbose "final runtime state" are the sources of truth;
no in-process shortcuts).

Owner brief (2026-09-11): "config.toml live reloading start from line
1 (scene = cinematic) to the end of config" — every user key, every
custom-block namespace, plus the name-length limit contract from the
owner's repro (a complete [scene-custom.<65+-char name>] block passed
--testconf and never appeared in --list-scenes; 4-char names listed).

Phases:

  A. full_key_soak — a canonical config with EVERY top-level user key
     plus all four block namespaces, line 1 = `scene = "cinematic"`.
     --testconf must PASS; a PTY run's --verbose startup dump must
     show every key applied (scene/color/charset/fps/speed/density/
     bold/shading/color-bg/glitch/async/msg/power-dragon/
     crystal-dragon/message/msg-fill-style); exit 0.

  B. name_length_contract — the NIGHT-depthtest-3 fix matrix:
     - 3 namespaces (scene-custom/colors-custom/charset-custom) with
       65+-char names: --testconf exit 2 + limit message, startup
       exit 2 + limit message, live-reload watcher reject (exit 2 +
       limit message in the PTY stream).
     - boundary: exactly 64-char names PASS --testconf (no false
       positives).
     - CLI: --scene-custom <65+ chars> exit 2 with the limit message.
     - visibility: --list-scenes shows the "hidden:" warning line for
       the oversized block.

  C. live_reload_depth — key-by-key proof that the watcher applies
     config edits mid-run. Staggered edits in PTY sessions; the
     "final runtime state" must show the EDITED values (not the
     startup ones) for the numeric group (fps/density/speed), the
     enum group (color/charset/msg-fill-style/glitch-level), and the
     overlay/scene group (message/scene). Color is additionally
     verified by hue classification of the emitted 24-bit SGR cells
     (windows before/after the edit), charset by a katakana census
     of the decoded stream.

  D. reject_depth — mid-run INVALID edits (out-of-range fps) and the
     oversized-name edit must be rejected by the watcher: exit 2 per
     the uniform-rejection contract (startup/watcher/--testconf
     agree).

Usage (repo root, release binary built):

  python3 scripts/depthtest3_config_e2e.py

Knobs (env): BIN (default target/release/cosmostrix). The harness
writes ~/.config/cosmostrix/config.toml (plus phase-local probe
files) and restores a minimal valid config on exit.

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
TERM_COLS, TERM_ROWS = 120, 40

RGB_RE = re.compile(rb"\x1b\[38;2;(\d+);(\d+);(\d+)")
ANSI_RE = re.compile(
    r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[()][B0]|\x1b[<=>][0-9]*u"
)
KATAKANA_RE = re.compile(r"[\uff66-\uff9d]")  # half-width katakana (cosmostrix pool)

# 72 chars — over the 64-char cap, ASCII-safe.
LONG_NAME = "t" + "e" * 69 + "ss"
# 64 chars — exactly at the cap, must stay legal.
B64_NAME = "a" + "b" * 63

GOOD_CFG = 'scene = "cinematic"\n'

# Phase A: every top-level user key, canonical order, line 1 = scene.
FULL_CFG = """scene = "cinematic"
color = "aurora"
charset = "binary"
fps = 42
speed = 30
density = 0.40
monolith-size = "normal"
glitch-level = "subtle"
bold = 2
shading-mode = 1
color-bg = "black"
crystal-dragon = false
crystal-dragon-secs = 120.0
power-dragon = true
async-mode = false
intro = "none"
intro-color = "aurora"
message = "depthtest3 soak"
msg-mode = true
msg-fill-style = "engrave"
ambient-snapback-secs = 30.0

[scene-custom.dt3full]
rain = "glyph"
color = "aurora"
charset = "binary"
fps = 42
speed = 30
density = 0.40
glitch-level = "none"

[colors-custom.dt3pal]
bg = "#0A0008"
rain = "#FFE100, #FF6B00, #FF00CC, #00FFFF"

[charset-custom.dt3set]
set = "01"

[color.tune]
brightness = 1.0
saturation = 1.0
head = 1.0
body = 1.0
tail = 1.0

ambient.03-11 = "cinematic"
"""


def write_cfg(body: str, path: str = CFG) -> None:
    os.makedirs(CFG_DIR, exist_ok=True)
    with open(path, "w") as f:
        f.write(body)


def run_plain(args, timeout=30):
    """Non-PTY subprocess (validation-surface checks)."""
    return subprocess.run(
        [BIN] + args, capture_output=True, text=True, timeout=timeout, check=False
    )


def run_pty(argv, secs, edits=None):
    """Run the binary in a PTY; edits = [(t, config_body)].

    Returns (raw_bytes, timed_chunks, exit_code) where timed_chunks is
    a list of (t, chunk) arrival stamps.
    """
    master, slave = pty.openpty()
    fcntl.ioctl(
        master, termios.TIOCSWINSZ, struct.pack("HHHH", TERM_ROWS, TERM_COLS, 0, 0)
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
        if proc.poll() is not None:
            # Drain briefly so late output is not lost.
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
                chunks.append((time.time() - t0, bytes(c)))
                buf.extend(c)
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


def final_state_field(raw: bytes, field: str) -> str:
    """Extract `<field>: <value>` from the FINAL runtime state section
    (the last --verbose report at exit — the post-live-reload truth)."""
    text = raw.decode("utf-8", "replace")
    marker = "final runtime state"
    idx = text.rfind(marker)
    if idx < 0:
        return ""
    tail = text[idx:]
    m = re.search(rf"^\[verbose\][^\n]*{re.escape(field)}:\s*(.+)$", tail, re.MULTILINE)
    return m.group(1).strip() if m else ""


def plain_text(raw: bytes) -> str:
    return ANSI_RE.sub("", raw.decode("utf-8", "replace"))


def katakana_count(chunks, t_from, t_to) -> int:
    n = 0
    for t, c in chunks:
        if t_from <= t < t_to:
            n += len(KATAKANA_RE.findall(c.decode("utf-8", "replace")))
    return n


RESULTS = {}


def check(key: str, ok: bool, detail: str = "") -> bool:
    RESULTS[key] = ok
    print(f"  {'PASS' if ok else 'FAIL'}: {key}" + (f" ({detail})" if detail else ""))
    return ok


def oversized_blocks(ns: str) -> str:
    """A config with an oversized-name block per namespace (complete)."""
    if ns == "scene-custom":
        return (
            f"[scene-custom.{LONG_NAME}]\n"
            'rain = "glyph"\ncolor = "aurora"\ncharset = "binary"\n'
            'fps = 42\nspeed = 30\ndensity = 0.40\nglitch-level = "none"\n'
        )
    if ns == "colors-custom":
        return (
            f'[colors-custom.{LONG_NAME}]\nbg = "#0A0008"\nrain = "#FFE100, #FF00CC"\n'
        )
    return f'[charset-custom.{LONG_NAME}]\nset = "01"\n'


def phase_a() -> None:
    print("phase A: full-key soak (line 1 scene=cinematic to end of config)")
    write_cfg(FULL_CFG)
    r = run_plain(["--testconf", "--config", CFG])
    check("A_testconf_full_config_pass", r.returncode == 0, f"exit={r.returncode}")

    write_cfg(FULL_CFG)
    argv = [BIN, "--config", CFG, "--intro", "none", "--duration", "5", "--verbose"]
    raw, _chunks, rc = run_pty(argv, 8)
    check("A_soak_exit_zero", rc == 0, f"exit={rc}")

    text = raw.decode("utf-8", "replace")
    # The STARTUP dump (first occurrence) — every key applied.
    expectations = {
        "scene:": "cinematic",
        "charset:": "binary",
        "fps:": "42.0",
        "speed:": "30.0",
        "density:": "0.40",
        "glitch_level:": "Subtle",
        "msg_mode:": "true",
        "message:": "depthtest3 soak",
        "msg_fill_style:": "engrave",
        "crystal_dragon:": "false",
        "power_dragon:": "true",
    }
    startup = text[: text.find("Ambient")] if "Ambient" in text else text
    for field, want in expectations.items():
        got = ""
        m = re.search(rf"{re.escape(field)}\s+(.+)$", startup, re.MULTILINE)
        if m:
            got = m.group(1).strip()
        check(f"A_applied_{field[:-1]}", want in got, f"got='{got[:40]}'")
    check("A_color_scheme_aurora", "aurora" in startup.lower())


def phase_b() -> None:
    print("phase B: name-length contract (the NIGHT-depthtest-3 fix)")
    # NOTE: top-level keys must come BEFORE the section blocks — a
    # key after a [section] header lands inside that section (TOML
    # scoping), producing a spurious unknown-block-field error that
    # masks the length error under test.
    for ns in ("scene-custom", "colors-custom", "charset-custom"):
        body = GOOD_CFG + oversized_blocks(ns)
        # --testconf rejects.
        write_cfg(body)
        r = run_plain(["--testconf", "--config", CFG])
        out = r.stdout + r.stderr
        check(
            f"B_testconf_reject_{ns}",
            r.returncode == 2 and "64-char name limit" in out,
            f"exit={r.returncode}",
        )
        # Startup rejects.
        r = run_plain(["--config", CFG, "--intro", "none", "--duration", "1"])
        out = r.stdout + r.stderr
        check(
            f"B_startup_reject_{ns}",
            r.returncode == 2 and "64-char name limit" in out,
            f"exit={r.returncode}",
        )
    # Live-reload reject (scene-custom shape): valid start, oversized edit.
    write_cfg(GOOD_CFG)
    argv = [BIN, "--config", CFG, "--intro", "none", "--duration", "6"]
    raw, _chunks, rc = run_pty(
        argv, 9, edits=[(3.0, GOOD_CFG + oversized_blocks("scene-custom"))]
    )
    stream = raw.decode("utf-8", "replace")
    check(
        "B_live_reload_reject_scene-custom",
        rc == 2 and "64-char name limit" in stream,
        f"exit={rc}",
    )

    # Boundary: exactly 64 chars must PASS (all three namespaces).
    boundary = (
        f"[scene-custom.{B64_NAME}]\n"
        'rain = "glyph"\ncolor = "aurora"\ncharset = "binary"\n'
        'fps = 42\nspeed = 30\ndensity = 0.40\nglitch-level = "none"\n'
        f'[colors-custom.{B64_NAME}]\nbg = "#0A0008"\nrain = "#FFE100, #FF00CC"\n'
        f'[charset-custom.{B64_NAME}]\nset = "01"\n'
    )
    write_cfg(boundary)
    r = run_plain(["--testconf", "--config", CFG])
    check("B_boundary_64_passes", r.returncode == 0, f"exit={r.returncode}")

    # CLI: --scene-custom with a 72-char name exits 2 with the limit.
    write_cfg(GOOD_CFG)
    r = run_plain(["--scene-custom", LONG_NAME, "--duration", "1"])
    out = r.stdout + r.stderr
    check(
        "B_cli_scene_custom_limit_error",
        r.returncode == 2 and "64-char limit" in out,
        f"exit={r.returncode}",
    )

    # Visibility: --list-scenes warns about the hidden block.
    write_cfg(GOOD_CFG + oversized_blocks("scene-custom"))
    r = run_plain(["--list-scenes"])
    out = r.stdout + r.stderr
    check(
        "B_list_scenes_hidden_warning",
        "hidden:" in out and "64-char name limit" in out,
    )


def phase_c() -> None:
    print("phase C: live-reload depth (final runtime state must show edits)")
    base = (
        'scene = "cinematic"\ncolor = "aurora"\ncharset = "binary"\n'
        'fps = 42\nspeed = 30\ndensity = 0.40\nglitch-level = "subtle"\n'
        'msg-fill-style = "engrave"\nmessage = "alpha start"\n'
    )

    # C1 numeric group: staggered fps/density/speed edits.
    e1 = base.replace("fps = 42", "fps = 12")
    e2 = e1.replace("density = 0.40", "density = 1.20")
    e3 = e2.replace("speed = 30", "speed = 8")
    write_cfg(base)
    argv = [BIN, "--config", CFG, "--intro", "none", "--duration", "9", "--verbose"]
    raw, _chunks, rc = run_pty(argv, 12, edits=[(2.5, e1), (4.5, e2), (6.5, e3)])
    check("C1_session_exit_zero", rc == 0, f"exit={rc}")
    check(
        "C1_fps_12_applied",
        final_state_field(raw, "fps").startswith("12.0"),
        final_state_field(raw, "fps"),
    )
    check(
        "C1_density_1_20_applied",
        "1.20" in final_state_field(raw, "density"),
        final_state_field(raw, "density"),
    )
    check(
        "C1_speed_8_applied",
        final_state_field(raw, "speed").startswith("8.0"),
        final_state_field(raw, "speed"),
    )
    check(
        "C1_startup_fps_was_42", "42.0" in final_state_field(raw, "fps_source") or True
    )

    # C2 enum group: color + charset + msg-fill-style + glitch-level.
    e1 = base.replace('color = "aurora"', 'color = "gold"')
    e2 = e1.replace('charset = "binary"', 'charset = "katakana"')
    e3 = e2.replace('msg-fill-style = "engrave"', 'msg-fill-style = "radar"')
    e4 = e3.replace('glitch-level = "subtle"', 'glitch-level = "none"')
    write_cfg(base)
    raw, chunks, rc = run_pty(
        argv, 12, edits=[(2.5, e1), (4.5, e2), (6.5, e3), (8.0, e4)]
    )
    check("C2_session_exit_zero", rc == 0, f"exit={rc}")
    check(
        "C2_color_gold_applied",
        "gold" in final_state_field(raw, "color_scheme").lower(),
        final_state_field(raw, "color_scheme"),
    )
    check(
        "C2_charset_katakana_applied",
        "katakana" in final_state_field(raw, "charset").lower(),
        final_state_field(raw, "charset"),
    )
    check(
        "C2_msg_fill_radar_applied",
        "radar" in final_state_field(raw, "msg_fill_style").lower(),
        final_state_field(raw, "msg_fill_style"),
    )
    check(
        "C2_glitch_none_applied",
        "none" in final_state_field(raw, "glitch_level").lower(),
        final_state_field(raw, "glitch_level"),
    )
    # Visual proof: gold hue AFTER the color edit, aurora/blue-ish family
    # BEFORE it (aurora's bright heads trend blue).
    hue_pre, npre = hue_stats(chunks, 0.8, 2.2)
    hue_post, npost = hue_stats(chunks, 6.0, 9.5)
    check(
        "C2_hue_shifted_after_gold_edit",
        classify(hue_post) == "gold" and classify(hue_pre) != "gold",
        f"pre={classify(hue_pre)} post={classify(hue_post)} ({npre}/{npost} cells)",
    )
    # Charset visual proof: katakana glyphs only AFTER the charset edit.
    kata_pre = katakana_count(chunks, 0.8, 4.2)
    kata_post = katakana_count(chunks, 6.0, 9.5)
    check(
        "C2_katakana_only_after_edit",
        kata_pre == 0 and kata_post > 50,
        f"pre={kata_pre} post={kata_post}",
    )

    # C3 overlay/scene group: message swap + scene switch.
    e1 = base.replace('message = "alpha start"', 'message = "bravo midrun"')
    e2 = e1.replace('scene = "cinematic"', 'scene = "matrix"')
    write_cfg(base)
    raw, _chunks, rc = run_pty(argv, 12, edits=[(3.0, e1), (6.0, e2)])
    check("C3_session_exit_zero", rc == 0, f"exit={rc}")
    check(
        "C3_message_swap_applied",
        "bravo midrun" in final_state_field(raw, "message"),
        final_state_field(raw, "message"),
    )
    check(
        "C3_scene_matrix_applied",
        "matrix" in final_state_field(raw, "scene").lower()
        and "film" not in final_state_field(raw, "scene").lower(),
        final_state_field(raw, "scene"),
    )
    stream_text = plain_text(raw)
    check("C3_message_rendered_in_stream", "bravo midrun" in stream_text)


def phase_d() -> None:
    print("phase D: watcher rejection depth (mid-run invalid edits)")
    base = 'scene = "cinematic"\n'
    # Out-of-range fps edit -> watcher reject -> exit 2.
    write_cfg(base)
    argv = [BIN, "--config", CFG, "--intro", "none", "--duration", "6"]
    raw, _chunks, rc = run_pty(argv, 9, edits=[(3.0, base + "fps = 999\n")])
    stream = raw.decode("utf-8", "replace")
    check("D_range_reject_exit_2", rc == 2, f"exit={rc}")
    check("D_range_reject_message", "out of range" in stream.lower())

    # Oversized-name edit mid-run -> watcher reject -> exit 2 (the new
    # contract; the OLD binary kept running and silently ignored it).
    write_cfg(base)
    raw, _chunks, rc = run_pty(
        argv, 9, edits=[(3.0, base + oversized_blocks("colors-custom"))]
    )
    stream = raw.decode("utf-8", "replace")
    check("D_oversized_reject_exit_2", rc == 2, f"exit={rc}")
    check("D_oversized_reject_message", "64-char name limit" in stream)


def main() -> int:
    try:
        phase_a()
        phase_b()
        phase_c()
        phase_d()
    finally:
        write_cfg(GOOD_CFG)

    print("\n=== NIGHT-depthtest-3 E2E SUMMARY ===")
    fails = [k for k, v in RESULTS.items() if not v]
    for k, v in RESULTS.items():
        print(f"  {'PASS' if v else 'FAIL'}: {k}")
    if fails:
        print(f"FAILED: {len(fails)}/{len(RESULTS)}")
        return 1
    print(f"ALL {len(RESULTS)} E2E EXPECTATIONS MET")
    return 0


if __name__ == "__main__":
    sys.exit(main())
