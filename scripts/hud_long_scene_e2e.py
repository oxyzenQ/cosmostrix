#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""E2E long-scene-name HUD verification (NIGHT-hunter-20, owner mandate).

Owner bug report 2026-09-08: loading a scene with a long name hard-cut
the `scn:` HUD metric line at the old 14-char setter truncation (the
budget designed around the pre-hunt 24-col HUD_MAX_WIDTH) —
`scn: example_1234_test_this_long` rendered as `scn: example_1234_t`,
with the chroma border column sitting exactly at the cut, reading as
"hardcut by border". The owner mandated a minimum usable HUD width of
64 characters.

This script spawns cosmostrix in a PTY with a custom scene named
`example_1234_test_this_long` (27 chars — the owner's exact example),
toggles the HUD with 'i', waits for the 1 Hz metric tick, reconstructs
the virtual terminal screen from the ANSI stream (shared
`ansi_screen.py` mini-emulator — see hud_order_e2e.py for why the raw
stream is not a valid assertion surface), and asserts the full scene
name is on SCREEN row 8 with the chroma border column past the text
(never mid-text).
"""

import os
import pty
import re
import select
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ansi_screen import reconstruct_screen

BIN = "./target/pro/cosmostrix"
COLS, ROWS = 100, 40

SCENE_NAME = "example_1234_test_this_long"

CONFIG_TOML = f"""# NIGHT-hunter-20 e2e config — owner's exact long scene name.
[scene-custom.{SCENE_NAME}]
rain = "glyph"
color = "energy-zen"
charset = "zen"
fps = 60
speed = 9
density = 0.75
glitch-level = "none"
"""


def main():
    # The config path is safepath-sandboxed to ~/.config/cosmostrix/
    # (strict whitelist — see src/safepath.rs). The e2e config is written
    # to the standard location and removed on exit when no prior config
    # existed (restore-when-present otherwise).
    import pathlib

    config_dir = pathlib.Path.home() / ".config" / "cosmostrix"
    config_dir.mkdir(parents=True, exist_ok=True)
    config_path = config_dir / "config.toml"
    prior_config = None
    if config_path.exists():
        prior_config = config_path.read_text()
    config_path.write_text(CONFIG_TOML)

    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(BIN, [
            BIN,
            "--config", config_path,
            "--scene-custom", SCENE_NAME,
            "--fps", "60",
        ])

    # Set the PTY window size BEFORE the app reads it (mirrors
    # hud_order_e2e.py — a fresh PTY is 1x1 and the app measures at
    # startup).
    import fcntl
    import struct
    import termios

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    stop = threading.Event()
    buf = bytearray()
    lock = threading.Lock()

    def reader():
        while not stop.is_set():
            try:
                r, _, _ = select.select([fd], [], [], 0.2)
                if not r:
                    continue
                chunk = os.read(fd, 65536)
                if not chunk:
                    break
                with lock:
                    buf.extend(chunk)
            except OSError:
                break

    t = threading.Thread(target=reader, daemon=True)
    t.start()

    # Skip the intro (6 s brand delay) then let it run a bit.
    time.sleep(7.0)
    os.write(fd, b"i")
    time.sleep(3.0)  # HUD on + at least one 1 Hz metric tick
    os.write(fd, b"q")
    time.sleep(1.0)
    stop.set()
    t.join(timeout=1.0)

    try:
        os.close(fd)
    except OSError:
        pass
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass

    with lock:
        text = bytes(buf).decode("utf-8", errors="replace")

    # Assert on the RECONSTRUCTED SCREEN (shared `ansi_screen.py`
    # mini-emulator, same methodology as hud_order_e2e.py after its
    # NIGHT-hunter-20 fix): the raw stream's emission order follows the
    # dirty-cell flush population, not the visual layout, so screen-row
    # reading is the only robust assertion. Row 8 is the `scn:` line.
    screen = reconstruct_screen(text, COLS, ROWS)

    full_line = f"scn: {SCENE_NAME}"
    scn_row = screen[8].rstrip() if len(screen) > 8 else ""
    on_screen = full_line in scn_row
    truncated = scn_row.startswith(" scn: example_1234_t") and not on_screen

    # Border contract: the chroma right-edge border column must sit at
    # or past the end of the scn text (never mid-text — the owner's
    # "hardcut by border" symptom).
    border_col = scn_row.find("│")
    text_end = len(full_line)

    # Restore the prior config state (remove the e2e config when none
    # existed before; restore the original content otherwise).
    if prior_config is None:
        config_path.unlink(missing_ok=True)
    else:
        config_path.write_text(prior_config)

    print(f"scn screen row:   {scn_row[:45]!r}")
    print(f"full line sought: {full_line!r}")
    print(f"border col: {border_col}, scn text end: {text_end}")

    if on_screen and (border_col == -1 or border_col >= text_end):
        print("PASS: long scene name renders in full, border past the text")
        return 0
    if truncated:
        print("FAIL: scn line still hard-cut at 14 chars (regression!)")
        return 1
    print("FAIL: scn row does not carry the full scene name")
    return 1


if __name__ == "__main__":
    sys.exit(main())
