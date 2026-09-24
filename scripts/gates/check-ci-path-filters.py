#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
"""CI path-filter hygiene gate: directory globs only, no hardcoded filenames.

Owner mandate 2026-09-24 (NIGHT-boost-1): workflow path filters gate
on directories, not on individual files. A filter entry that points
inside a directory must be a glob (`scripts/**`, `src/**`,
`docs/**`); hardcoding a specific filename (`scripts/example.sh`)
rots the moment the file is renamed or deleted — the workflow then
silently stops triggering while the filter still looks alive. The
2026-09-13 incident in ci.yml (`test/**` missing after commit 5553174:
jobs never ran for a test-only change) is the same failure class.

Rule (mechanical, no YAML parser needed):
- Only `paths:` and `paths-ignore:` blocks in .github/workflows/*.yml
  are scanned. Branch/tag filters are out of scope.
- An entry containing `/` (points inside a directory) MUST contain a
  glob metacharacter (`*`). `scripts/**` passes, `scripts/foo.sh`
  fails.
- Root-level entries (`Cargo.toml`, `deny.toml`, `*.sh`) carry no
  slash and are exempt: a root build file cannot be globbed more
  generally without matching unrelated files, and root-level
  pre-emptive entries (codeql.yml's future-language set) are
  deliberate.

Usage: python3 scripts/gates/check-ci-path-filters.py
Exit code: 0 = all filters glob-clean, 1 = hardcoded filename entries.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

WORKFLOW_DIR = Path(".github/workflows")
# A `paths:` / `paths-ignore:` block header at any indent.
BLOCK_KEY_RE = re.compile(r"^(\s*)(paths|paths-ignore):\s*$")
# A list entry inside the block: `- 'entry'` or `- entry`.
ENTRY_RE = re.compile(r"^\s*-\s+(?:'([^']+)'|\"([^\"]+)\"|(\S+))\s*$")


def scan_workflow(path: Path) -> list[str]:
    """Return violation strings for one workflow file."""
    hits: list[str] = []
    block_indent: int | None = None
    for lineno, raw in enumerate(
        path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        stripped = raw.strip()
        # Comments and blank lines never end a block and are never
        # entries; skip them before any indent math.
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(raw) - len(raw.lstrip(" "))
        if BLOCK_KEY_RE.match(raw):
            block_indent = indent
            continue
        if block_indent is None:
            continue
        if indent <= block_indent:
            # First shallower-or-equal line after the header closes the
            # paths block (list entries sit one level deeper).
            block_indent = None
            continue
        entry_match = ENTRY_RE.match(raw)
        if entry_match is None:
            continue
        entry = next(g for g in entry_match.groups() if g is not None)
        # The anti-pattern: names a file inside a directory without a
        # glob. Directory scope must be expressed as a directory glob.
        if "/" in entry and "*" not in entry:
            hits.append(
                f"{path}:{lineno}: hardcoded filename in path filter: "
                f"{entry!r} — use a directory glob "
                f"(e.g. {entry.rsplit('/', 1)[0] + '/**'})"
            )
    return hits


def main() -> int:
    if not WORKFLOW_DIR.is_dir():
        print("check-ci-path-filters: .github/workflows not found (skipping)")
        return 0
    workflows = sorted(WORKFLOW_DIR.glob("*.yml"))
    if not workflows:
        print("check-ci-path-filters: no workflow files found (skipping)")
        return 0
    all_hits: list[str] = []
    for path in workflows:
        all_hits.extend(scan_workflow(path))
    if all_hits:
        print("check-ci-path-filters: hardcoded filename entries found:")
        for hit in all_hits:
            print(f"  {hit}")
        print(
            "  Fix: gate directories with globs ('scripts/**'), not "
            "filenames ('scripts/example.sh'). Root-level files "
            "(Cargo.toml) are exempt."
        )
        return 1
    print(
        f"check-ci-path-filters: clean ({len(workflows)} workflows, "
        "directory entries are all globs)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
