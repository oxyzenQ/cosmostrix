#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Pure-English language audit for cosmostrix (NIGHT-depthtest-2).

Owner rule 2026-09-11: cosmostrix is a pure-English project — commit
messages, comments, strings, docs, and diagnostics must be English.
This script detects non-English HUMAN LANGUAGE content in every
git-tracked text file and fails (exit 1) on any finding, so the rule
is CI-enforced (gate-keepers check 13) instead of relying on review.

What counts as language (flagged):

  1. Letter runs of length >= 2 in a non-Latin script (CJK ideographs,
     kana, hangul, cyrillic, greek words, thai, arabic, ...) — a run of
     one letter is a glyph citation or math notation, a run of two or
     more is a word.
  2. Latin-script words carrying diacritics outside the proper-noun
     allowlist (scientific attributions, see PROPER_NOUNS below).

What is deliberately kept (functional, not language):

  1. Isolated single letters (µs, pi in math prose, Δx, a katakana or
     runic glyph cited as an example in docs) — cannot form a word.
  2. Charset DATA lines (glyph payloads, not prose): any line matching
     a data-line pattern (a charset set assignment, a glyph array
     constant, or a parser fixture call). Glyphs are the product's
     payload; the themed presets (halfwidth katakana, math symbols,
     box drawing) are owner-directed data.
  3. Files whose FUNCTION is unicode stress: wide-char rejection
     fixtures, message sanitizer fixtures, adversarial CLI/config
     corpora, glyph provenance comments. Each exemption in
     EXEMPT_FILES carries its reason inline so a future sweep can
     re-review them.

Usage: python3 scripts/language_audit.py
Exit 0 = clean, 1 = language content found (printed with file:line).
"""

import re
import subprocess
import sys
import unicodedata
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# Scanned extensions: every tracked text surface (code, docs, configs).
EXTENSIONS = (".rs", ".py", ".sh", ".md", ".toml", ".yml", ".yaml", ".txt")

# Scientific proper nouns (attribution accuracy beats the ASCII rule —
# misspelling a person's name to satisfy a linter is worse than the
# diacritic). Keys use unicode escapes so this file stays pure ASCII
# and the audit can scan itself.
PROPER_NOUNS = {
    "r\u00f6ssler": "Rossler attractor (Otto Rossler, spelled with diaeresis)",
    "bj\u00f6rn": "Bjorn Ottosson (OKLab author, spelled with diaeresis)",
    "ottosson": "Bjorn Ottosson (OKLab author)",
}

# Charset DATA line patterns — the line is a glyph payload, not prose.
DATA_LINE_PATTERNS = (
    re.compile(r"set\s*=\s*[\"']"),  # charset-custom values (template, tests, heredocs)
    re.compile(r"GHOST_CHARS"),  # ghost-event glyph array + its data lines
    re.compile(r"const\s+\w*_SET\b"),  # themed charset preset constants
    re.compile(r"parse_charset_value\("),  # charset parser fixture calls
    re.compile(r"sanitize_message_text\("),  # message sanitizer fixture calls
    # A line that is ONLY a quoted string literal (multiline string
    # continuation in Rust / Python) is data, never prose.
    re.compile(r"^[\s]*[\"'][^\"']*[\"'][\s,)]*$"),
)

# File-level exemptions — unicode is the FUNCTION under test. Each
# entry: path -> reason. Review these first when tightening the rule.
EXEMPT_FILES = {
    "test/tests/depthtest_cli_config.rs": (
        "adversarial CLI/config corpus: math-alnum numerals, ligatures, "
        "gothic letter, CJK unknown-key fixture — the parser must eat them"
    ),
    "test/tests/width_guard.rs": (
        "width-guard documentation quotes the historical CJK glyph set"
    ),
    "test/config/configfile_tests/bug19.rs": (
        "quoted-value charset fixture corpus including one wide char"
    ),
    "test/config/config_apply_tests/template_presets.rs": (
        "owner-directed themed charset preset data (katakana set)"
    ),
    "src/scene/charset_custom.rs": (
        "wide-char rejection fixtures drive the single-width filter"
    ),
    "src/output/message.rs": (
        "message sanitizer fixtures: CJK width replacement behavior"
    ),
    "src/engine/cosmic_dragon_engine/cloud/events/ghost.rs": (
        "ghost-event glyph payload (halfwidth katakana) + provenance "
        "comment naming the retired fullwidth glyph set"
    ),
}

WORD_RE = re.compile(r"\w+")


def tracked_text_files():
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.splitlines()
    return [
        REPO / f
        for f in out
        if f.endswith(EXTENSIONS)
        and not f.startswith("docs/archive/")
        and not re.match(r"benchmark/bench-labs/(sweep_|PGO_AB_|BOLT2_)", f)
    ]


def is_letter(ch):
    return unicodedata.category(ch).startswith("L")


def is_latin(ch):
    return "LATIN" in unicodedata.name(ch, "").upper()


def line_is_data(line):
    return any(p.search(line) for p in DATA_LINE_PATTERNS)


def letter_runs(line):
    """Collapse consecutive non-ASCII letter chars into script-stable runs."""
    letters = [(i, ch) for i, ch in enumerate(line) if ord(ch) > 127 and is_letter(ch)]
    runs = []
    for i, ch in letters:
        if (
            runs
            and i == runs[-1][0] + len(runs[-1][1])
            and is_latin(ch) == is_latin(runs[-1][1][0])
        ):
            runs[-1][1].append(ch)
        else:
            runs.append([i, [ch]])
    return runs


def audit_line(line):
    """Return human-readable findings for one line of text."""
    findings = []
    if line_is_data(line):
        return findings
    for start, chars in letter_runs(line):
        if not is_latin(chars[0]):
            if len(chars) >= 2:
                word = "".join(chars)
                findings.append(f"non-latin letter run '{word}' ({len(chars)} chars)")
            # A single non-Latin letter is a glyph citation or math
            # notation — not a word. Kept by design (see docstring).
            continue
        # Latin script with diacritics: allowed only for allowlisted
        # proper nouns. Extract the full word around the run.
        token = word_at(line, start)
        if token.lower() in PROPER_NOUNS:
            continue
        findings.append(
            f"latin diacritic word '{token}' (not an allowlisted proper noun)"
        )
    return findings


def word_at(line, pos):
    r"""Return the maximal word token containing char position pos."""
    for m in WORD_RE.finditer(line):
        if m.start() <= pos < m.end():
            return m.group(0)
    return line[pos]


def main():
    files = tracked_text_files()
    hits = []
    for f in files:
        rel = str(f.relative_to(REPO))
        if rel in EXEMPT_FILES:
            continue
        try:
            lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            continue
        for lineno, line in enumerate(lines, 1):
            if line.isascii():
                continue
            for finding in audit_line(line):
                hits.append((rel, lineno, finding, line.strip()[:100]))

    print(
        f"language audit: scanned {len(files)} tracked text files "
        f"({len(EXEMPT_FILES)} function-exempt)"
    )
    if hits:
        print(f"X {len(hits)} non-English finding(s):")
        for rel, lineno, finding, ctx in hits:
            print(f"  {rel}:{lineno}: {finding}")
            print(f"    {ctx}")
        print(
            "cosmostrix is a pure-English project — translate or remove the "
            "text above (see scripts/language_audit.py docstring for the "
            "functional-data categories)."
        )
        sys.exit(1)
    print(
        "OK 0 findings — prose is pure English (isolated math letters, "
        "charset glyph data and unicode-stress fixtures are exempt by design)"
    )


if __name__ == "__main__":
    main()
