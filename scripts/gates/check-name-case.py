#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux/macOS). Requires git (ls-files) and
# python3 - both are gate-keepers.sh runtime staples.
#
# COSMOSTRIX NAME-CASE CHECK (NIGHT-dinner-2)
#
"""Check the lowercase project-name rule (docs/BRANDING.md section 2).

The project name is lowercase `cosmostrix` in every context - prose,
titles, headings, code, CLI output, comments, commit subjects, and
file paths - the nginx/curl convention the owner mandated. This gate
scans EVERY tracked file and EVERY tracked path (git ls-files: the
.cargo/ and .github/ hidden trees included - nothing excluded, the
archive included) and fails on any casing outside the legal families
BRANDING section 2 codifies:

  1. lowercase - the name itself, always;
  2. the identifier family - the `COSMOSTRIX_*` environment
     variables and the `COSMOSTRIX-DISCLAIMER` / `COSMOSTRIX-*`
     markers (attached with `_` or `-`);
  3. the banner family - the all-caps banner comment titles heading
     the scripts/ gate files (a comment line opening with the
     all-caps name);
  4. the display-banner family - the all-caps display titles the
     binary prints and the docs quote: the name followed by an
     all-caps word (`COSMOSTRIX BENCHMARK`, `COSMOSTRIX DIAGNOSTICS
     REPORT`, `COSMOSTRIX PERFORMANCE REPORT`) or by an em-dash hero
     separator (`COSMOSTRIX — The ...`).

Every other casing - a capitalized first letter, internal capitals,
or all-caps as prose - fails with file:line:token. The checker is
self-clean by construction: the all-caps form is built at runtime
from the lowercase name, so this file carries no literal it would
flag itself on.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NAME = "cosmostrix"
UPPER = NAME.upper()
IDENTIFIER_CHARS = ("_", "-")
BANNER_SUFFIXES = {".sh", ".py"}
BANNER_RE = re.compile(r"^\s*#\s*" + UPPER + r"(\s|$)")
MAX_REPORTED = 50


def tracked_paths() -> list[str]:
    """Every path in the index - nothing excluded."""
    try:
        out = subprocess.run(
            ["git", "ls-files", "-z"],
            cwd=ROOT,
            check=True,
            capture_output=True,
        ).stdout
    except subprocess.CalledProcessError as exc:
        print("cosmostrix name-case check: FAIL")
        print(f"git ls-files failed (exit {exc.returncode}) - run from a repo checkout")
        raise SystemExit(1) from exc
    return sorted(p.decode("utf-8", "surrogateescape") for p in out.split(b"\0") if p)


def banner_title(path: str, line: str) -> bool:
    """True when the line is a banner title heading a scripts/ gate file."""
    if not path.startswith("scripts/"):
        return False
    if Path(path).suffix not in BANNER_SUFFIXES:
        return False
    return BANNER_RE.match(line) is not None


def display_banner(rest: str) -> bool:
    """True when the text after the all-caps name opens a display title.

    `rest` is the line content from the end of the token onward. A
    display banner is the name as the first word of an all-caps title
    (`COSMOSTRIX BENCHMARK...`) or a hero line (`COSMOSTRIX — The...`
    with an em-dash separator). Anything else - lowercase prose, a
    sentence, a punctuation terminator - is all-caps used as a word
    and fails.
    """
    return re.match(r"^\s+(—|[A-Z]{2,}(?:-[A-Z]+)*)", rest) is not None


def violation_reason(
    path: str, line: str, token: str, prev: str, nxt: str, rest: str
) -> str | None:
    """None when the occurrence is legal, else the failure reason."""
    if token == NAME:
        return None
    if token == UPPER:
        if nxt in IDENTIFIER_CHARS or prev in IDENTIFIER_CHARS:
            return None
        if banner_title(path, line):
            return None
        if display_banner(rest):
            return None
        return "all-caps outside the identifier/banner/display families (docs/BRANDING.md section 2)"
    return "the name is lowercase in every context (docs/BRANDING.md section 2)"


def main() -> int:
    failures: list[str] = []
    files = 0
    tokens = 0
    lowercase = 0
    family = 0

    for path in tracked_paths():
        files += 1

        lowered = path.lower()
        pos = lowered.find(NAME)
        while pos != -1:
            end = pos + len(NAME)
            if path[pos:end] != NAME:
                bad = path[pos:end]
                failures.append(
                    f"FAIL PATH   {path} '{bad}' - path tokens must be lowercase"
                )
            pos = lowered.find(NAME, end)

        target = ROOT / path
        if not target.is_file():
            continue
        try:
            text = target.read_bytes().decode("utf-8", errors="replace")
        except OSError:
            continue
        for lineno, line in enumerate(text.splitlines(), 1):
            lowered = line.lower()
            pos = lowered.find(NAME)
            while pos != -1:
                end = pos + len(NAME)
                token = line[pos:end]
                prev = line[pos - 1] if pos > 0 else ""
                nxt = line[end] if end < len(line) else ""
                rest = line[end:]
                tokens += 1
                reason = violation_reason(path, line, token, prev, nxt, rest)
                if reason is None:
                    if token == NAME:
                        lowercase += 1
                    else:
                        family += 1
                else:
                    failures.append(f"FAIL CASE   {path}:{lineno} '{token}' - {reason}")
                pos = lowered.find(NAME, end)

    print("cosmostrix name-case check: " + ("FAIL" if failures else "PASS"))
    for failure in failures[:MAX_REPORTED]:
        print(failure)
    if len(failures) > MAX_REPORTED:
        print(f"... and {len(failures) - MAX_REPORTED} more violation(s) not listed")
    print(
        f"Checked {files} tracked file(s) and path(s), {tokens} name token(s): "
        f"{lowercase} lowercase, {family} identifier/banner/display family, "
        f"{len(failures)} violation(s)."
    )
    if failures:
        print("The rule (docs/BRANDING.md section 2): the name is lowercase in")
        print("every context; the COSMOSTRIX_* / COSMOSTRIX-* identifiers, the")
        print("scripts/ banner titles, and the all-caps display banners are the")
        print("only uppercase survivors.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
