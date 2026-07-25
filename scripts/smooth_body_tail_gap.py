#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""
Task 3: Smooth the body-tail luminance gap.

At high speed/density, droplets are short, so the body-tail transition
(stop 2 → stop 3) is sharp. The luminance gap between stop 2 (tail) and
stop 3 (body peak) is typically +250-275 in fast themes (storm, hacker,
neon), creating a visible "line" or gap in the rain.

This script smooths the transition by pulling stop 2 ~30% toward stop 3:
  new_stop2 = lerp(stop2, stop3, 0.3)

This reduces the gap by 30% without dramatically changing the tail's
hue identity. The body peak (stop 3) and head stops are untouched.

Applied to ALL stop-based themes (7-stop and 9-stop) since the gap is
inherent to the original stop values, not specific to fast themes.
"""

import re
import sys
from pathlib import Path


def lerp(a, b, t):
    """Linear interpolation between two RGB tuples at parameter t in [0, 1]."""
    return (
        round(a[0] + (b[0] - a[0]) * t),
        round(a[1] + (b[1] - a[1]) * t),
        round(a[2] + (b[2] - a[2]) * t),
    )


def smooth_file(path):
    """Smooth the body-tail gap (stop 2 → stop 3) in all stop-based themes."""
    content = path.read_text()
    stats = {'smoothed_7': 0, 'smoothed_9': 0, 'skipped_other': 0}

    # Find each stops array. Strip comments before extracting tuples.
    pattern = re.compile(
        r'(stops:\s*&\[)\s*(?P<inner>(?:\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*\)\s*,?\s*)+)\]',
        re.DOTALL
    )

    def replacer(m):
        prefix = m.group(1)
        inner = m.group('inner')
        # Strip comments before extracting tuples
        inner_clean = re.sub(r'//[^\n]*', '', inner)
        tuples = re.findall(r'\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)', inner_clean)
        if not tuples:
            stats['skipped_other'] += 1
            return m.group(0)
        tuples = [(int(r), int(g), int(b)) for r, g, b in tuples]

        if len(tuples) < 4:
            # Need at least 4 stops (0,1,2,3) to smooth the 2→3 gap
            stats['skipped_other'] += 1
            return m.group(0)

        # Smooth stop 2 toward stop 3 by 30%.
        # This reduces the tail→body luminance gap by 30%.
        old_stop2 = tuples[2]
        stop3 = tuples[3]
        new_stop2 = lerp(old_stop2, stop3, 0.3)
        tuples[2] = new_stop2

        if len(tuples) == 7:
            stats['smoothed_7'] += 1
        elif len(tuples) == 9:
            stats['smoothed_9'] += 1
        else:
            stats['skipped_other'] += 1
            return m.group(0)

        # Reconstruct with 16-space indent
        lines = [f"                ({r}, {g}, {b})," for r, g, b in tuples]
        return f"{prefix}\n" + "\n".join(lines) + "\n            ]"

    new_content = pattern.sub(replacer, content)
    if new_content != content:
        path.write_text(new_content)
    return stats


def main():
    target = Path('/home/z/my-project/cosmostrix/src/central_colors.rs')
    if not target.exists():
        print(f"error: {target} not found", file=sys.stderr)
        return 1

    print(f"Smoothing body-tail gap in {target}...")
    print("  Transformation: new_stop2 = lerp(old_stop2, stop3, 0.3)")
    print()
    stats = smooth_file(target)
    print(f"  7-stop themes smoothed: {stats['smoothed_7']}")
    print(f"  9-stop themes smoothed: {stats['smoothed_9']}")
    print(f"  Skipped (other): {stats['skipped_other']}")
    return 0


if __name__ == '__main__':
    sys.exit(main())
