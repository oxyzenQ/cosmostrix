#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
"""Repo-wide emoji sweep for cosmostrix (NIGHT-hunt-48 owner rule).

Scans EVERY git-tracked text file (*.md, *.rs, *.sh, *.py, *.yml,
*.toml, ... plus extensionless text files). A file counts as text when
it decodes as strict UTF-8; binaries (images, blobs) fail the decode
and are skipped. This replaces the old .md-only doc sweep: the owner
rule is repo-wide, so the detector is repo-wide.

Fail classes (mirror the forbidden classes of docs/RULES.md "Output
Glyph Policy" -- the symbol-only output gate bans the same blocks on
output surfaces; this sweep extends them to every text file):
  U+1F000..U+1FFFF  astral emoji (incl. regional indicator flags)
  U+2600..U+27BF    misc symbols + dingbats (check/cross, warning)
  U+2300..U+23FF    misc technical (clocks, media controls, key caps)
  U+2B00..U+2BFF    stars / pictographic thick arrows
  U+FE0F, U+FE0E    emoji variation selectors (16/15)
  U+200D            zero-width joiner

Allowed non-ASCII (typographic house style, never flagged): prose
arrows U+2190..21FF, box drawing + geometric U+2500..25FF, em/en dash,
ellipsis, bullet, math operators. None of those ranges intersect the
fail blocks, so the allowlist needs no carve-outs.

Exemptions (file-level, each justified -- keep this list SHORT):
  scripts/gates/check-symbol-only-output.sh  the denylist itself (literal glyphs)
  src/output/message.rs                sanitizer test INPUT: real emoji
                                       as data to verify replacement

Excluded paths (frozen or generated, never scanned):
  docs/archive/**                    frozen history (same exclusion as
                                     every other gate; see NIGHT-hunt-46)
  benchmark/bench-labs/sweep_*|PGO_AB_*|BOLT2_*  auto-generated artifacts

Strict by default: exit 1 on any hit, so gate-keepers.sh check 15 can
block a commit on emoji. --fix applies the replacement table (icons
map to the v80.0.0-beta.2 symbol vocabulary: check marks to OK, cross
marks to X), strips variation selectors, then re-scans; leftover hits
still exit 1.

Usage:
  python3 scripts/audit/emoji-audit.py          strict scan (gate mode)
  python3 scripts/audit/emoji-audit.py --fix    replace known emoji, re-scan
"""

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

# Emoji -> plain replacement, keyed by codepoint (robust against
# Python unicode-name coverage gaps on newer emoji blocks).
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
    0x2713: "OK",  # check mark (v80.0.0-beta.2: no longer "functional")
    0x2717: "X",  # ballot X (v80.0.0-beta.2: no longer "functional")
    0x26A0: "warning:",  # warning sign
    0x1F512: "",  # lock
    0x1F50E: "",  # magnifying glass
    0x1F6E1: "",  # shield
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

# Invisible emoji formatting selectors: always garbage in repo text.
STRIP_ALWAYS = {0x200D, 0xFE0F, 0xFE0E}

# Fail blocks: (low, high, class label) -- the RULES.md forbidden
# classes, extended from output surfaces to the whole tracked tree.
FAIL_BLOCKS = [
    (0x1F000, 0x1FFFF, "astral-emoji"),
    (0x2600, 0x27BF, "misc-symbols-dingbats"),
    (0x2300, 0x23FF, "misc-technical"),
    (0x2B00, 0x2BFF, "stars-pictographic"),
]

# File-level exemptions, each with a justification (keep SHORT).
EXEMPT_FILES = {
    "scripts/gates/check-symbol-only-output.sh": "embeds the denylist glyphs",
    "src/output/message.rs": "sanitizer test INPUT (emoji as data)",
}

# Frozen or generated paths, never scanned (archive is history by the
# NIGHT-hunt-46 directive; bench-labs sweeps are machine artifacts).
EXCLUDE_PATH_RE = re.compile(
    r"^(docs/archive/|benchmark/bench-labs/(sweep_|PGO_AB_|BOLT2_))"
)

MAX_CONTEXT_LINES = 5


def fail_class(ch: str) -> str | None:
    """Return the fail-class label for ch, or None when allowed."""
    cp = ord(ch)
    if cp in STRIP_ALWAYS:
        return "variation-selector"
    for lo, hi, name in FAIL_BLOCKS:
        if lo <= cp <= hi:
            return name
    return None


# The --fix pass only ever rewrites FAIL-CLASS characters: this table
# is the fail-class subset of REPLACEMENTS. Two entries from the old
# .md-only sweep were deliberately dropped for parity with the RULES.md
# classes: U+25B6 (play triangle, geometric block) and U+2139
# (information source, letterlike block) are ALLOWED glyphs --
# living_rain.rs uses U+25B6 in its doc-comment state diagram as
# rain-adjacent ART. Replacing allowed glyphs was the first draft's bug
# (caught by diffing the tree before commit, never shipped).
FIX_TABLE = {
    cp: repl for cp, repl in REPLACEMENTS.items() if fail_class(chr(cp)) is not None
}


def tracked_text_files() -> list[tuple[str, str]]:
    """Every tracked (or untracked non-ignored) file that is UTF-8 text."""
    out = subprocess.run(
        [
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.splitlines()
    files = []
    for rel in out:
        if EXCLUDE_PATH_RE.match(rel) or rel in EXEMPT_FILES:
            continue
        try:
            text = (REPO / rel).read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue  # binary or unreadable: not repo text
        files.append((rel, text))
    return files


# (lineno, line_text, [(char, fail_class), ...]) per flagged line.
LineHits = list[tuple[int, str, list[tuple[str, str]]]]


def scan(files: list[tuple[str, str]]) -> dict[str, LineHits]:
    """Return {rel: [(lineno, line_text, [(char, class), ...])]}."""
    hits: dict[str, LineHits] = {}
    for rel, text in files:
        per_file = []
        for lineno, line in enumerate(text.splitlines(), 1):
            found = [(ch, fail_class(ch)) for ch in line if fail_class(ch)]
            if found:
                per_file.append((lineno, line, found))
        if per_file:
            hits[rel] = per_file
    return hits


def apply_fixes(files: list[tuple[str, str]]) -> int:
    """Rewrite fail-class chars only; allowed glyphs stay untouched."""
    changed = 0
    for rel, text in files:
        new_text = text
        for cp, repl in FIX_TABLE.items():
            new_text = new_text.replace(chr(cp), repl)
        for cp in STRIP_ALWAYS:
            new_text = new_text.replace(chr(cp), "")
        if new_text != text:
            (REPO / rel).write_text(new_text, encoding="utf-8")
            changed += 1
    return changed


def main() -> None:
    fix = "--fix" in sys.argv
    files = tracked_text_files()
    hits = scan(files)
    if fix and hits:
        changed = apply_fixes(files)
        print(f"--fix: rewrote {changed} file(s)")
        files = tracked_text_files()
        hits = scan(files)

    total = sum(len(found) for lines in hits.values() for _, _, found in lines)
    print(f"scanned {len(files)} tracked text files")
    print(f"emoji violations: {total} across {len(hits)} file(s)")
    for rel, lines in sorted(hits.items()):
        print(f"\n{rel} ({sum(len(f) for _, _, f in lines)} hits):")
        for lineno, line, found in lines[:MAX_CONTEXT_LINES]:
            cps = " ".join(f"U+{ord(ch):04X}" for ch, _ in found)
            print(f"  [{cps}] L{lineno}: {line.strip()[:86]}")
        if len(lines) > MAX_CONTEXT_LINES:
            print(f"  ... (+{len(lines) - MAX_CONTEXT_LINES} more lines)")

    sys.exit(1 if hits else 0)


if __name__ == "__main__":
    main()
