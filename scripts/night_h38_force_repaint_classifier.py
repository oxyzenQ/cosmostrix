#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).

"""NIGHT-hunt-38: the force-repaint classifier — a standing audit tool.

Owner approval (2026-09-13, NIGHT-depth-hunt-1 follow-up): the
force-repaint frame-state membership probe — previously a one-off
method recorded in
docs/audits/NIGHT_DEPTH_HUNT_1_AUDIT_2026-09-13.md (Part 4) — is
promoted into scripts/ as a standing audit tool.

The question this tool answers, per style: are the "frozen" cells the
style sweep reports real cleanup bugs, or live static structure?

Method (the classifier contract):

1. Run the style in a PTY, form for FORM_SECS, then five checkpoints
   across WINDOW_SECS. A cell that holds IDENTICAL content (glyph
   AND color) at every checkpoint is "frozen" (same multi-checkpoint
   rule as night_h34_style_sweep.py — kills snapshot-coincidence
   false positives).

2. Fire the HUD toggle ON then OFF ('i' twice). Toggling OFF runs
   force_draw_everything(), which resyncs the PHYSICAL screen from
   the frame state: every cell the frame believes blank is emitted
   blank again (the NIGHT-hunter-16/27 immediate-clear contract).

3. Snapshot after SETTLE_SECS and classify every frozen cell:
   - same glyph + color  -> LIVE STATIC (the frame owns this cell
     and keeps it identical: the black hole annulus, the DNA ladder
     rungs, the quasar disk core, dormant integrate-and-fire
     neurons). Benign by design.
   - now a space         -> ORPHAN: the frame does NOT own this cell
     (physical residue with no frame-state membership). A real
     cleanup bug — the sweep's "frozen" reading was correct.
   - different content   -> ADVANCING (actively re-rendered live
     content, e.g. charge accumulation on a neuron). Benign.

Verdict rule: any ORPHAN cell -> exit 1 (a real bug; file it). All
frozen cells static or advancing -> exit 0.

Usage (repo root, release binary built):

  python3 scripts/night_h38_force_repaint_classifier.py            # all 13 structured styles
  python3 scripts/night_h38_force_repaint_classifier.py neural     # subset by scene name
  BIN=target/release/cosmostrix python3 scripts/night_h38_force_repaint_classifier.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from night_cbg34_e2e import ANALYSIS_ROWS, Run
from night_h34_style_sweep import STYLES, frozen_cells

CFG_DIR = os.path.expanduser("~/.config/cosmostrix/h38classifier")
CFG = os.path.join(CFG_DIR, "config.toml")
BIN = os.environ.get("BIN", "target/release/cosmostrix")

FORM_SECS = 4.0
WINDOW_SECS = 12.0
# HUD on -> off, then let the post-repaint frames flow before the
# classification snapshot.
HUD_ON_AT = 0.5
HUD_OFF_AT = 2.0
SETTLE_SECS = 1.5
N_CHECK = 5


def classify(frozen, post):
    """Split the frozen set by post-repaint membership.

    Returns (static, advancing, erased) position lists. `post` is the
    post-repaint snapshot grid; cells are pyte Char tuples so the
    identical check covers glyph AND color (a brightness-advancing
    cell classifies as advancing, not static).
    """
    static, advancing, erased = [], [], []
    for x, y, cell in frozen:
        after = post[y][x]
        if after == cell:
            static.append((x, y))
        elif after[0] in (" ", ""):
            erased.append((x, y))
        else:
            advancing.append((x, y))
    return static, advancing, erased


def classify_style(scene, style):
    argv = [BIN, "--config", CFG, "--scene", scene, "--intro", "none"]
    actions = []
    labels = []
    for i in range(N_CHECK):
        t = FORM_SECS + WINDOW_SECS * i / (N_CHECK - 1)
        actions.append((round(t, 2), "snap", f"c{i}"))
        labels.append(f"c{i}")
    # The force-repaint: HUD on, then off (force_draw_everything
    # resyncs the physical screen from the frame state on the off
    # toggle; the on toggle redraws the overlay region which the
    # analysis window excludes).
    actions.append((round(FORM_SECS + WINDOW_SECS + HUD_ON_AT, 2), "key", "i"))
    actions.append((round(FORM_SECS + WINDOW_SECS + HUD_OFF_AT, 2), "key", "i"))
    actions.append(
        (round(FORM_SECS + WINDOW_SECS + HUD_OFF_AT + SETTLE_SECS, 2), "snap", "post")
    )
    total = FORM_SECS + WINDOW_SECS + HUD_OFF_AT + SETTLE_SECS + 1.0
    snaps = Run(argv, CFG, actions, run_secs=total).run()
    frozen = frozen_cells(snaps, labels, ANALYSIS_ROWS)
    post = snaps["post"]
    return classify(frozen, post), len(frozen)


def main() -> int:
    which = sys.argv[1:] or [s for s, _ in STYLES]
    os.makedirs(CFG_DIR, exist_ok=True)
    with open(CFG, "w") as f:
        f.write('color-bg = "default-background"\n')

    table = []
    orphans_total = 0
    for scene, style in STYLES:
        if scene not in which:
            continue
        print(
            f"[{style}] scene={scene} ({FORM_SECS:.0f}s form + {WINDOW_SECS:.0f}s window + force-repaint)"
        )
        (static, advancing, erased), frozen_n = classify_style(scene, style)
        verdict = "CLEAN" if not erased else "ORPHANS FOUND (real cleanup bug)"
        print(
            f"    frozen: {frozen_n}  ->  static: {len(static)}  advancing: {len(advancing)}  erased(orphan): {len(erased)}"
        )
        for x, y in erased[:8]:
            print(
                f"      ORPHAN cell x={x} y={y} (space after force-repaint — frame does not own it)"
            )
        if erased:
            print(f"    verdict: {verdict}")
        table.append((style, frozen_n, len(static), len(advancing), len(erased)))
        orphans_total += len(erased)

    print("\n=== force-repaint classifier table ===")
    print(f"{'style':<12} {'frozen':>7} {'static':>7} {'advancing':>10} {'orphan':>7}")
    for style, frozen_n, static_n, advancing_n, erased_n in table:
        print(
            f"{style:<12} {frozen_n:>7} {static_n:>7} {advancing_n:>10} {erased_n:>7}"
        )
    if orphans_total:
        print(
            f"\nFAIL: {orphans_total} orphan cell(s) erased by force-repaint — real cleanup bug(s)."
        )
        return 1
    print(
        "\nPASS: every frozen cell is frame-state content (static or advancing) — no orphans."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
