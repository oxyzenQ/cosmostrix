#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Emoji / strange-symbol sweep for cosmostrix docs (owner cold/zen directive).

Scans every git-tracked .md file (excluding docs/archive/** and
auto-generated bench-labs artifacts). Hits are printed with codepoint +
line context for mechanical triage. Functional glyphs (arrows, box
drawing, check/cross marks used as terminal-status semantics) are kept;
decorative pictographs are flagged.

Usage: python3 scripts/emoji-audit.py [--fix]
"""

import bisect
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# Decorative emoji -> plain replacement, keyed by codepoint (robust
# against Python unicode-name coverage gaps on newer emoji blocks).
REPLACEMENTS = {
    0x1F525: "",  # fire
    0x2728: "",  # sparkles
    0x1F680: "",  # rocket
    0x1F389: "",  # party popper
    0x1F3C6: "",  # trophy
    0x1F432: "",  # dragon face
    0x1F48E: "",  # gem stone
    0x1F480: "",  # skull
    0x1F4AA: "",  # flexed biceps
    0x1F44F: "",  # clapping
    0x1F44D: "OK",  # thumbs up
    0x2705: "OK",  # white heavy check mark
    0x274C: "X",  # cross mark
    0x26A0: "warning:",  # warning sign
    0x1F512: "",  # lock
    0x1F50E: "",  # magnifying glass
    0x1F6E1: "",  # shield
    0x2139: "note:",  # information source
    0x1F4E6: "",  # package
    0x23F0: "",  # alarm clock
    0x1F4A1: "",  # bulb
    0x1F4DA: "",  # books
    0x2699: "",  # gear
    0x1F527: "",  # wrench
    0x1F4C8: "",  # chart increasing
    0x1F41B: "",  # bug
    0x1F4A9: "",  # poo
    0x1F440: "",  # eyes
    0x1F4AF: "100%",  # hundred points
    0x1F197: "OK",  # OK button
    0x1F388: "",  # balloon
    0x1F382: "",  # birthday cake
    0x1F602: "",  # tears of joy
    0x1F600: "",  # grinning
    0x1F60A: "",  # smiling eyes
    0x1F914: "",  # thinking face
    0x26A1: "",  # high voltage
    0x2B50: "",  # star
    0x1F31F: "",  # glowing star
    0x2744: "",  # snowflake
    0x1F308: "",  # rainbow
    0x2601: "",  # cloud
    0x25B6: ">",  # play triangle
    0x2714: "OK",  # heavy check mark
    0x2716: "X",  # heavy multiplication x
    0x1F5D1: "",  # wastebasket
    0x1F5C3: "",  # card file box
    0x1F9E0: "",  # brain
    0x1F4CB: "",  # clipboard
    0x1F50D: "",  # left magnifying glass
    0x1F6A9: "",  # triangular flag
    0x1F3AF: "",  # target
    0x1F9F8: "",  # teddy
}

# Emoji ZWJ sequences and variation selectors are always garbage in text.
STRIP_ALWAYS = {0x200D, 0xFE0F, 0xFE0E}

# Suspect codepoint ranges (broad pictograph blocks).
SUSPECT_RANGES = [
    (0x1F000, 0x1FAFF),
    (0x2600, 0x27BF),
    (0x2B00, 0x2BFF),
    (0x1F1E6, 0x1F1FF),
    (0x2190, 0x21FF),  # arrows: flagged for review, most are functional
]

# Functional glyphs kept (arrows, box-drawing, typography, math).
KEEP = set(
    list(range(0x2500, 0x25FF + 1))  # box drawing + geometric
    + [0x2192, 0x2190, 0x2191, 0x2193, 0x2194, 0x21D2]  # common arrows
    + [0x2713, 0x2717, 0x00D7, 0x2014, 0x2013, 0x2026]
    + [0x2265, 0x2264, 0x2248, 0x2260, 0x00B1, 0x00B7, 0x00B0]
    + [0x2500, 0x2502, 0x250C, 0x2514, 0x2518, 0x2510]
    + [0x2550, 0x2551, 0x256D, 0x256E, 0x256F, 0x2570]
    + [0x2591, 0x2592, 0x2593]  # shade blocks (ASCII-art fallbacks)
)


def tracked_md_files():
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "*.md"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.splitlines()
    return [
        REPO / f
        for f in out
        if not f.startswith("docs/archive/")
        and not re.match(r"benchmark/bench-labs/(sweep_|PGO_AB_|BOLT2_)", f)
    ]


def is_suspect(ch: str) -> bool:
    cp = ord(ch)
    if cp in STRIP_ALWAYS or cp in KEEP:
        return cp in STRIP_ALWAYS
    return any(lo <= cp <= hi for lo, hi in SUSPECT_RANGES)


def main():
    fix = "--fix" in sys.argv
    files = tracked_md_files()
    total = 0
    per_file = {}
    for f in files:
        text = f.read_text(errors="replace")
        hits = [(i, ch) for i, ch in enumerate(text) if is_suspect(ch)]
        if hits:
            per_file[str(f.relative_to(REPO))] = hits
            total += len(hits)
            if fix:
                new_text = text
                for cp, repl in REPLACEMENTS.items():
                    new_text = new_text.replace(chr(cp), repl)
                new_text = re.sub("[\u200d\ufe0f\ufe0e]", "", new_text)
                if new_text != text:
                    f.write_text(new_text)

    print(f"scanned {len(files)} non-archive .md files")
    print(f"suspect occurrences: {total} across {len(per_file)} files")
    for rel, hits in sorted(per_file.items()):
        lines = (REPO / rel).read_text(errors="replace").splitlines()
        line_starts = []
        pos = 0
        for ln in lines:
            line_starts.append(pos)
            pos += len(ln) + 1
        print(f"\n{rel} ({len(hits)} hits):")
        shown = 0
        last_line = -1
        for i, ch in hits:
            lineno = bisect.bisect_right(line_starts, i) - 1
            if lineno == last_line:
                continue
            last_line = lineno
            ctx = lines[lineno].strip()[:86] if 0 <= lineno < len(lines) else "?"
            print(f"  U+{ord(ch):05X} {ch!r} L{lineno + 1}: {ctx}")
            shown += 1
            if shown >= 5:
                print("  ...")
                break


if __name__ == "__main__":
    main()
