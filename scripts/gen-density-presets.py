#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky
#
# Cosmostrix Density-Map Preset Generator
#
# Generates CSV density-map strings for scene-custom monolith pillar sculpting.
# Each preset is a comma-separated list of f64 weights (0.0..1.0), one per
# terminal column. 0.0 = never spawn, 1.0 = always spawn.
#
# Usage:
#   python3 scripts/gen-density-presets.py <preset> [columns]
#
# Presets:
#   twin-towers  Two dense clusters at cols 20-35 and 85-100, sparse canyon between
#   cascade      Linear gradient: dense left (1.0) to sparse right (0.05)
#   throne       Central pillar (cols 50-70) ringed by sparse court
#   list         Print all available preset names
#
# Examples:
#   python3 scripts/gen-density-presets.py twin-towers
#   python3 scripts/gen-density-presets.py cascade 200
#   python3 scripts/gen-density-presets.py throne 80
#
# Output: single line of comma-separated f64 weights, ready to paste into
# config.toml as: scene-custom.<name>.density-map = <output>

import sys

DEFAULT_COLS = 120


def twin_towers(cols: int) -> list[float]:
    """Two dense pillar clusters at 17% and 71% of width, sparse between."""
    w = []
    c1_start, c1_end = int(cols * 0.17), int(cols * 0.29)
    c2_start, c2_end = int(cols * 0.71), int(cols * 0.83)
    for c in range(cols):
        if c1_start <= c <= c1_end or c2_start <= c <= c2_end:
            w.append(1.0)
        elif c1_start - 2 <= c <= c1_end + 2 or c2_start - 2 <= c <= c2_end + 2:
            w.append(0.7)
        else:
            w.append(0.08)
    return w


def cascade(cols: int) -> list[float]:
    """Smooth linear gradient from 1.0 (left) to 0.05 (right)."""
    w = []
    for c in range(cols):
        t = c / max(cols - 1, 1)
        w.append(round(1.0 - 0.95 * t, 3))
    return w


def throne(cols: int) -> list[float]:
    """Central pillar ringed by concentric sparse courts."""
    w = []
    center = cols // 2
    core_r = max(int(cols * 0.08), 8)
    edge_r = core_r + 3
    inner_r = edge_r + 5
    outer_r = inner_r + int(cols * 0.1)
    for c in range(cols):
        dist = abs(c - center)
        if dist <= core_r:
            w.append(1.0)
        elif dist <= edge_r:
            w.append(0.8)
        elif dist <= inner_r:
            w.append(0.3)
        elif dist <= outer_r:
            w.append(0.12)
        else:
            w.append(0.05)
    return w


PRESETS = {
    "twin-towers": twin_towers,
    "cascade": cascade,
    "throne": throne,
}


def main():
    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help", "help"):
        print(__doc__)
        sys.exit(0)

    name = sys.argv[1].lower()

    if name == "list":
        print("Available presets:")
        for p in PRESETS:
            print(f"  {p}")
        sys.exit(0)

    if name not in PRESETS:
        print(f"error: unknown preset '{name}'", file=sys.stderr)
        print(f"available: {', '.join(PRESETS.keys())}", file=sys.stderr)
        sys.exit(1)

    cols = DEFAULT_COLS
    if len(sys.argv) >= 3:
        try:
            cols = int(sys.argv[2])
            if cols < 10 or cols > 500:
                raise ValueError
        except ValueError:
            print(f"error: columns must be 10-500, got '{sys.argv[2]}'", file=sys.stderr)
            sys.exit(1)

    weights = PRESETS[name](cols)
    print(",".join(str(w) for w in weights))


if __name__ == "__main__":
    main()
