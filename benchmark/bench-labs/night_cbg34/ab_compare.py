#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Compare NIGHT-hunter-34 A/B bench JSONs (2 runs per side, per scene).

Extracts the owner-contract metrics: avg_fps, frame_entropy_bits,
density_gini, dirty_cells_per_frame. Prints a markdown table.
"""

import json
import sys
from pathlib import Path

LAB = Path(__file__).parent


def load(label, scene, run):
    p = LAB / f"{label}_{scene}_run{run}.json"
    with p.open() as f:
        d = json.load(f)
    return (
        d["performance"]["avg_fps"],
        d["visual_objective"]["frame_entropy_bits"],
        d["visual_objective"]["density_gini"],
        d["cell_efficiency"]["dirty_cells_per_frame"],
    )


def mean(xs):
    return sum(xs) / len(xs)


def main():
    rows = []
    for scene in ("cinematic", "monolith"):
        a = [load("A", scene, r) for r in (1, 2)]
        b = [load("B", scene, r) for r in (1, 2)]
        for i, name in enumerate(
            ("avg fps", "entropy bits", "density gini", "dirty cells/frame")
        ):
            am, bm = mean([x[i] for x in a]), mean([x[i] for x in b])
            delta = (bm - am) / am * 100.0 if am else 0.0
            rows.append((scene, name, am, bm, delta))
    for scene, name, am, bm, delta in rows:
        print(f"| {scene} | {name} | {am:,.2f} | {bm:,.2f} | {delta:+.2f} % |")


if __name__ == "__main__":
    print("| scene | metric | baseline | after | delta |")
    print("|---|---|---|---|---|")
    main()
    sys.exit(0)
