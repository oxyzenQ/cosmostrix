#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""NIGHT-hunt-34: all-style stuck-cell audit (thirteen structured styles).

Owner directive (2026-09-12): the glyph-family stuck-cell bug was fixed in
commit 7247862 (NIGHT-hunter-17: sweep gate + interval + all-cells check).
Before LTS, the thirteen structured styles (monolith, vortex, flux, lorenz,
dragon, physarum, black_hole, aeolian, solar_flare, dna_helix, murmuration,
quasar, neural) must be audited for the same bug class: cells that freeze
on screen for a long time and only clear when the style's own motion
happens to pass through them (a vacated-cell cleanup gap in the family's
draw pass — the monolith diff-cleanup contract).

Method: per style, run the scene for FORM_SECS (structures form), snapshot,
keep running for WINDOW_SECS with NO input (the style's own motion must
clean up everything it vacates), snapshot again, and report every non-space
cell identical across the window. Structured styles legitimately hold
STATIC cells (the black hole ball annulus, the DNA ladder rungs, the quasar
disk core) — those are live (redrawn each frame, identical content) and are
expected findings; the report separates them from scattered outliers via
position clustering, and the final classification is a manual review
decision (see the audit table in the commit message).

Usage (repo root, release binary built):

  python3 scripts/night_h34_style_sweep.py             # all 13 styles
  python3 scripts/night_h34_style_sweep.py vortex flux # subset

Exit 0 = sweep completed and report written (this is an audit tool, not a
pass/fail gate: the frozen set is expected to contain each style's static
structure cells).
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from night_cbg34_e2e import Run, SyncScreen, TERM_COLS, TERM_ROWS, ANALYSIS_ROWS

CFG_DIR = os.path.expanduser("~/.config/cosmostrix/h34sweep")
CFG = os.path.join(CFG_DIR, "config.toml")
BIN = os.environ.get("BIN", "target/release/cosmostrix")

FORM_SECS = 4.0
WINDOW_SECS = 12.0

# (scene name, RainStyle label) — the scene catalog pins each style.
STYLES = [
    ("monolith", "monolith"),
    ("vortex", "vortex"),
    ("flux", "flux"),
    ("lorenz", "lorenz"),
    ("cosmic_dragon", "dragon"),
    ("physarum", "physarum"),
    ("sorgonemous_intrascals", "black_hole"),
    ("aeolian", "aeolian"),
    ("solar_flare", "solar_flare"),
    ("dna_helix", "dna_helix"),
    ("murmuration", "murmuration"),
    ("quasar", "quasar"),
    ("neural", "neural"),
]


def frozen_cells(snaps, labels, rows):
    """Non-space cells IDENTICAL across ALL snapshots (multi-checkpoint:
    kills snapshot-coincidence false positives — a vortex mote back at the
    same orbital position, or a lorenz mote revisiting a dense attractor
    region, matches TWO snapshots by chance but not five)."""
    grids = [snaps[l] for l in labels]
    row_cap = min(rows, *(len(g) for g in grids))
    stuck = []
    for y in range(row_cap):
        for x in range(len(grids[0][y])):
            c0 = grids[0][y][x]
            if c0[0] in (" ", ""):
                continue
            if all(g[y][x] == c0 for g in grids[1:]):
                stuck.append((x, y, c0))
    return stuck


def cluster_positions(stuck):
    """Group frozen cells into contiguous clusters (static structures show
    as large clusters; scattered residue shows as 1-3 cell clusters)."""
    cells = {(x, y) for x, y, _ in stuck}
    clusters = []
    while cells:
        seed = next(iter(cells))
        stack = [seed]
        cells.discard(seed)
        comp = [seed]
        while stack:
            cx, cy = stack.pop()
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    n = (cx + dx, cy + dy)
                    if n in cells:
                        cells.discard(n)
                        stack.append(n)
                        comp.append(n)
        clusters.append(sorted(comp))
    clusters.sort(key=len, reverse=True)
    return clusters


def main() -> int:
    which = sys.argv[1:] or [s for s, _ in STYLES]
    os.makedirs(CFG_DIR, exist_ok=True)
    with open(CFG, "w") as f:
        f.write('color-bg = "default-background"\n')

    table = []
    for scene, style in STYLES:
        if scene not in which:
            continue
        print(f"[{style}] scene={scene} ({FORM_SECS}s form + {WINDOW_SECS}s window)")
        argv = [BIN, "--config", CFG, "--scene", scene, "--intro", "none"]
        # Five checkpoints across the window: a cell must hold identical
        # content at every one to count as frozen.
        n_check = 5
        actions = []
        labels = []
        for i in range(n_check):
            t = FORM_SECS + WINDOW_SECS * i / (n_check - 1)
            actions.append((round(t, 2), "snap", f"c{i}"))
            labels.append(f"c{i}")
        snaps = Run(argv, CFG, actions, run_secs=FORM_SECS + WINDOW_SECS + 0.5).run()
        stuck = frozen_cells(snaps, labels, ANALYSIS_ROWS)
        clusters = cluster_positions(stuck)
        big = [c for c in clusters if len(c) >= 6]
        small = [c for c in clusters if len(c) < 6]
        print(f"    frozen: {len(stuck)} cells, {len(clusters)} clusters")
        for c in clusters[:4]:
            xs = [x for x, _ in c]
            ys = [y for _, y in c]
            print(
                f"      cluster {len(c):3d} cells  x[{min(xs)}..{max(xs)}] y[{min(ys)}..{max(ys)}]"
            )
        if small:
            print(f"    SCATTERED small clusters (<=5 cells): {len(small)}")
            for c in small[:8]:
                print(f"      {c[:6]}")
        table.append((style, len(stuck), len(big), len(small)))
    print("\n=== audit table ===")
    print(f"{'style':<12} {'frozen':>7} {'big':>4} {'scattered':>9}")
    for style, frozen, big, small in table:
        print(f"{style:<12} {frozen:>7} {big:>4} {small:>9}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
