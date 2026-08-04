#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: Cross-platform Python 3 (Linux, macOS, BSD, Windows).
#   Pure-Python stdlib only (re, pathlib). Safe to run anywhere Python 3.7+.
# ─────────────────────────────────────────────────────────────────────────────
"""
Task 1: Head stop luminance audit + normalization.

Scans all stop-based themes in src/central_colors.rs. For each theme,
finds the LAST stop (head) and computes its RGB sum. If the sum exceeds
the NEON_GREEN_BENCHMARK (655), scales down all three channels
proportionally to bring the sum to exactly 655, preserving the hue.

Algorithm:
  - scale = 655 / sum
  - new_vals = [round(v * scale) for v in (R, G, B)]
  - adjust the largest channel by (655 - new_sum) to hit 655 exactly
    (compensates for rounding drift)

Themes with sum <= 655 are skipped (already compliant).
"""

import re
import sys
from pathlib import Path

NEON_GREEN_BENCHMARK = 655


def scale_to_benchmark(r, g, b, target=NEON_GREEN_BENCHMARK):
    """Scale (r, g, b) proportionally so the sum equals `target`.
    Preserves hue by multiplying all channels by the same factor.
    Adjusts the largest channel to compensate for rounding drift.
    """
    s = r + g + b
    if s <= target:
        return (r, g, b)
    scale = target / s
    nr = round(r * scale)
    ng = round(g * scale)
    nb = round(b * scale)
    ns = nr + ng + nb
    # Compensate rounding drift: adjust the largest channel.
    if ns != target:
        diff = target - ns
        # Pick the largest channel to absorb the diff.
        vals = [(nr, "r"), (ng, "g"), (nb, "b")]
        vals.sort(reverse=True)
        largest = vals[0][1]
        if largest == "r":
            nr = max(0, min(255, nr + diff))
        elif largest == "g":
            ng = max(0, min(255, ng + diff))
        else:
            nb = max(0, min(255, nb + diff))
    return (nr, ng, nb)


def transform_file(path):
    """Apply head-stop normalization to all stop-based themes."""
    content = path.read_text()
    stats = {"normalized": 0, "skipped_ok": 0, "skipped_other": 0}

    # Find each stops array. The head stop is the LAST tuple in the array.
    # Pattern: stops: &[ ... (r, g, b), (r, g, b), ..., (HEAD_R, HEAD_G, HEAD_B) ]
    pattern = re.compile(
        r"(stops:\s*&\[)\s*(?P<inner>(?:\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*\)\s*,?\s*)+)\]",
        re.DOTALL,
    )

    def replacer(m):
        prefix = m.group(1)
        inner = m.group("inner")
        # Parse all tuples
        tuples = re.findall(r"\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)", inner)
        if not tuples:
            stats["skipped_other"] += 1
            return m.group(0)

        # Convert to ints
        tuples = [(int(r), int(g), int(b)) for r, g, b in tuples]
        head = tuples[-1]
        head_sum = sum(head)
        if head_sum <= NEON_GREEN_BENCHMARK:
            stats["skipped_ok"] += 1
            return m.group(0)

        # Scale head to benchmark
        new_head = scale_to_benchmark(*head)
        tuples[-1] = new_head
        stats["normalized"] += 1

        # Reconstruct the stops array with 16-space indent
        lines = [f"                ({r}, {g}, {b})," for r, g, b in tuples]
        return f"{prefix}\n" + "\n".join(lines) + "\n            ]"

    new_content = pattern.sub(replacer, content)
    if new_content != content:
        path.write_text(new_content)
    return stats


def main():
    REPO_ROOT = Path(__file__).resolve().parent.parent
    target = REPO_ROOT / "src" / "central_colors.rs"
    if not target.exists():
        print(f"error: {target} not found", file=sys.stderr)
        return 1

    print(f"Normalizing head stops in {target}...")
    print(f"Benchmark: NeonGreen head sum = {NEON_GREEN_BENCHMARK}")
    print()
    stats = transform_file(target)
    print(
        f"  Themes normalized (head sum > {NEON_GREEN_BENCHMARK}): {stats['normalized']}"
    )
    print(
        f"  Themes already compliant (head sum <= {NEON_GREEN_BENCHMARK}): {stats['skipped_ok']}"
    )
    print(f"  Skipped (empty/other): {stats['skipped_other']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
