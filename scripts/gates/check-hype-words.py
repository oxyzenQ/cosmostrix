#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
# COSMOSTRIX HYPE-VOCABULARY CHECK (night-dragon-maintenance-5)
"""Check the banned hype-vocabulary rule (docs/BRANDING.md section 6).

Grandiose marketing claims fail the push in every tracked file -
source comments included. The owner's rationale: an unverifiable
superlative reads as bluster to every AI agent and human skeptic that
cannot self-verify it, and invites the toy-project dismissal instead
of the technical respect the measured properties earn. State the
property and the measurement; never the crown.

Single-source law: the banned token list lives ONLY in the fenced
block under the "Banned hype vocabulary" heading of docs/BRANDING.md.
This gate parses the fence at runtime and never carries its own copy,
so the list can be extended by editing the home doc alone (a gate with
a second copy of the list is the next drift waiting for its day).
`masterclass` and `curiosity` are owner-kept words and are not listed.

Exemptions, both documented in the home doc:
  - docs/archive/** and CHANGELOG.md - the frozen records are never
    rewritten (the same frozen-record law the disclaimer and docs-audit
    corpus rules honor), so their historical vocabulary stands;
  - docs/BRANDING.md whole-file - the law's home must be able to name
    what it bans (the Avoid anti-example quotes the pattern it
    forbids).

This file is exempt from itself too, and is written self-clean by
construction: the tokens are runtime data parsed from the home doc,
so no literal of the banned family appears in this source (the same
self-clean shape the name-case checker uses).

Matching is case-insensitive substring per line - the tokens are
word-shaped enough (hyphenated compounds, possessive phrases) that a
boundary grammar would be precision theater; a false positive is a
one-line wording fix, a missed claim is a shipped lie.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HOME_DOC = "docs/BRANDING.md"
HEADING_MARKER = "### banned hype vocabulary"
FENCE_OPEN = "```text"
FENCE_CLOSE = "```"
EXEMPT_PREFIXES = ("docs/archive/",)
EXEMPT_FILES = {
    "CHANGELOG.md",
    HOME_DOC,
    "scripts/gates/check-hype-words.py",
}
BINARY_SNIFF_BYTES = 8192
MAX_REPORTED = 50


def parse_banned_tokens() -> list[str]:
    """Read the banned-token fence from the home doc; fail closed.

    The fence must exist and carry at least one token - a home doc
    that lost its list, or a renamed heading, is a broken law, not a
    vacuous pass.
    """
    lines = (ROOT / HOME_DOC).read_text(encoding="utf-8").splitlines()
    heading = next(
        (
            i
            for i, ln in enumerate(lines)
            if ln.strip().lower().startswith(HEADING_MARKER)
        ),
        None,
    )
    if heading is None:
        print("cosmostrix hype-vocabulary check: FAIL")
        print(f"{HOME_DOC}: the '{HEADING_MARKER}' heading is missing -")
        print("the banned list has no home; restore it or fix the gate marker")
        raise SystemExit(1)
    open_idx = next(
        (i for i in range(heading + 1, len(lines)) if lines[i].strip() == FENCE_OPEN),
        None,
    )
    if open_idx is None:
        print("cosmostrix hype-vocabulary check: FAIL")
        print(f"{HOME_DOC}: no {FENCE_OPEN} fence under the banned heading")
        raise SystemExit(1)
    tokens: list[str] = []
    for line in lines[open_idx + 1 :]:
        if line.strip() == FENCE_CLOSE:
            break
        token = line.strip()
        if token and not token.startswith("#"):
            tokens.append(token.lower())
    if not tokens:
        print("cosmostrix hype-vocabulary check: FAIL")
        print(f"{HOME_DOC}: the banned-token fence is empty - a law that")
        print("bans nothing is not a law")
        raise SystemExit(1)
    return tokens


def tracked_paths() -> list[str]:
    """Every path in the index - nothing excluded from the walk."""
    out = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout
    return sorted(p.decode("utf-8", "surrogateescape") for p in out.split(b"\0") if p)


def is_binary(raw: bytes) -> bool:
    """Null-byte sniff over the file head - the cheap, honest test."""
    return b"\x00" in raw[:BINARY_SNIFF_BYTES]


def main() -> int:
    tokens = parse_banned_tokens()
    violations: list[tuple[str, int, str]] = []
    scanned = 0
    for path in tracked_paths():
        if path in EXEMPT_FILES or path.startswith(EXEMPT_PREFIXES):
            continue
        file_path = ROOT / path
        if not file_path.is_file():
            continue
        raw = file_path.read_bytes()
        if is_binary(raw):
            continue
        scanned += 1
        text = raw.decode("utf-8", errors="replace")
        for lineno, line in enumerate(text.splitlines(), start=1):
            lowered = line.lower()
            for token in tokens:
                if token in lowered:
                    violations.append((path, lineno, token))
    if violations:
        print("cosmostrix hype-vocabulary check: FAIL")
        shown = violations[:MAX_REPORTED]
        for path, lineno, token in shown:
            print(f"{path}:{lineno}: banned token '{token}'")
        hidden = len(violations) - len(shown)
        if hidden > 0:
            print(f"... and {hidden} more (cap {MAX_REPORTED})")
        print(f"{len(violations)} hit(s); state the property and the")
        print("measurement, not the crown (docs/BRANDING.md section 6)")
        return 1
    print(
        "cosmostrix hype-vocabulary check: PASS "
        f"({len(tokens)} banned tokens x {scanned} tracked text files)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
