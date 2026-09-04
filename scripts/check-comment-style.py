#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Comment-style gate: no decorative markdown emphasis in Rust comments.

Owner mandate 2026-09-04 (docs/COMMENT_STYLE.md section 2): raw source
comments must read as plain prose. `**bold**` and `*italic*` emphasis
markers are banned in every comment type (`//`, `///`, `//!`); the
2026-09-04 sweep removed all 378 of them. This gate keeps them out.

Functional rustdoc constructs are NOT flagged:
- inline code backticks (an asterisk inside backticks is code, not
  emphasis — e.g. `(channel * fi + 128)`);
- content inside doc-comment code fences (```text / ```json / doctests);
- arithmetic like `a * b` in prose (the italic pattern requires a
  non-space character immediately after the opening asterisk and
  immediately before the closing one).

Checked surface: every git-tracked *.rs file under src/ AND test/
(the mirrored test tree from NIGHT-hunter-1 — its files carry the
same comment-style contract as production source).

Usage: python3 scripts/check-comment-style.py
Exit code: 0 = clean, 1 = emphasis markers found (printed with location).
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

BOLD_RE = re.compile(r"\*\*([^*\n]+?)\*\*")
ITALIC_RE = re.compile(r"\*(\S(?:[^*\n]*?\S)?)\*")
# Inline code spans: emphasis markers inside backticks are code, not
# markdown. Strip them from the scanned text before matching.
BACKTICK_RE = re.compile(r"`[^`\n]*`")


def tracked_rs_files() -> list[Path]:
    out = subprocess.run(
        [
            "git",
            "ls-files",
            "src/**/*.rs",
            "src/*.rs",
            "test/**/*.rs",
            "test/*.rs",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    return [Path(p) for p in out.stdout.splitlines() if p]


def doc_fence_toggle(line: str) -> bool:
    """True when a doc-comment line opens/closes a code fence."""
    stripped = line.lstrip()
    if stripped.startswith(("///", "//!")):
        rest = stripped[3:].strip()
        if rest.startswith(("```", "~~~")):
            return True
    return False


def scan_file(path: Path) -> list[str]:
    hits: list[str] = []
    in_fence = False
    for lineno, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        if doc_fence_toggle(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        stripped = line.lstrip()
        if not stripped.startswith("//"):
            continue
        # Hide inline-code spans so asterisks inside them are not flags.
        scan_text = BACKTICK_RE.sub("", line)
        bold = BOLD_RE.search(scan_text)
        italic = ITALIC_RE.search(scan_text)
        if bold:
            hits.append(f"{path}:{lineno}: bold emphasis {bold.group(0)!r}")
        elif italic:
            hits.append(f"{path}:{lineno}: italic emphasis {italic.group(0)!r}")
    return hits


def main() -> int:
    files = tracked_rs_files()
    if not files:
        print("check-comment-style: no tracked .rs files found (skipping)")
        return 0
    all_hits: list[str] = []
    for path in files:
        all_hits.extend(scan_file(path))
    if all_hits:
        print("check-comment-style: decorative markdown emphasis found:")
        for hit in all_hits[:40]:
            print(f"  {hit}")
        if len(all_hits) > 40:
            print(f"  ... and {len(all_hits) - 40} more")
        print("  Fix: rewrite as plain prose (see docs/COMMENT_STYLE.md section 2).")
        return 1
    print(f"check-comment-style: clean ({len(files)} files, 0 emphasis markers)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
