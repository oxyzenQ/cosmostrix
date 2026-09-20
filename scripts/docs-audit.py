#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
"""Docs audit engine: broken refs, stale paths, stale counts, duplicates.

rg-philosophy: pattern-driven sweeps over git-tracked .md files.

Corpus rules (NIGHT-hunt-5, 2026-09-20 — encoded to match the contract
documented in docs/FUTURE_BACKLOG.md section 1):
  - Historical snapshots are EXCLUDED (never rewritten, so their own
    refs/claims are not defects): docs/archive/**, docs/research/**
    (dated investigation logs), docs/audits/** (dated audit reports),
    the CHANGELOG-V*-ERA.md era files, benchmark/bench-labs/** A/B
    artifacts except the hand-maintained BENCH_LABS.md index, and the
    benchmark/bench-labs/sweep_*.md machine-generated reports.
  - CHANGELOG.md: only the `## Unreleased` section is live; everything
    below the next `## ` heading after it is released history.

Negation/history context (same philosophy as scripts/stale-hunt.py
applies to Rust comments): a ref or count claim whose line, preceding
3 lines, or enclosing markdown section (nearest heading, capped at 15
lines) carries an explicit history/removal/example marker is
intentional history documentation, not staleness. The accepted set is
registered in docs/FUTURE_BACKLOG.md section 2.
"""

import re
import subprocess
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# History/removal/example framing markers. A stale-looking ref or count
# inside such a context is intentional documentation of the past, not a
# live claim. Keep the list tight enough that a bare stale claim with no
# framing still flags.
HIST_RE = re.compile(
    r"removed|deleted|retired|no longer|moved to|moved from|"
    r"later moved|previously|formerly|"
    r"historical|legacy|superseded|archived|was removed|was deleted|"
    r"was written|was retained|example|illustrat|pattern|mirrors|"
    r"such as|e\.g\.|accurate at|predate|originally|used to|"
    r"at the time|snapshot|verbatim|never committed|"
    r"as-of-commit|retroactive",
    re.IGNORECASE,
)

SECTION_CAP = 15  # max lines between a hit and its nearest heading


def git_md_files():
    out = subprocess.run(
        ["git", "ls-files", "*.md"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.splitlines()
    keep = []
    for f in out:
        if f.startswith("docs/archive/"):
            continue
        if f.startswith("docs/research/"):
            continue
        if f.startswith("docs/audits/"):
            continue
        if re.match(r"CHANGELOG-V\d+-ERA\.md$", f):
            continue
        if (
            f.startswith("benchmark/bench-labs/")
            # Hand-maintained index stays; A/B artifacts and sweeps do not.
            and f != "benchmark/bench-labs/BENCH_LABS.md"
        ):
            continue
        keep.append(REPO / f)
    return keep


def unreleased_only(text: str) -> str:
    """CHANGELOG.md: keep the header + the `## Unreleased` section only."""
    lines = text.splitlines()
    out, in_unreleased, unreleased_seen = [], False, False
    for line in lines:
        if line.startswith("## "):
            if line.startswith("## Unreleased"):
                in_unreleased, unreleased_seen = True, True
            else:
                in_unreleased = False
                if not unreleased_seen:
                    break  # released history with no live section above it
            if unreleased_seen and not in_unreleased:
                break  # first released section ends the live part
            if not unreleased_seen:
                continue  # skip stray released sections above Unreleased
        if in_unreleased or not unreleased_seen:
            # pre-Unreleased front matter (title, intro) is live
            out.append(line)
    return "\n".join(out)


def context_exempt(lines: list[str], idx: int) -> bool:
    """Is the claim on line idx (0-based) framed as intentional history?

    Window: the hit line, up to 3 lines before, and 2 lines after —
    prose wraps, so the verb of a removal note ("X\n  was removed in")
    regularly lands on the line after the reference itself. Fallback:
    the enclosing markdown section, nearest heading above, capped.
    """
    window = "\n".join(lines[max(0, idx - 3) : idx + 3])
    if HIST_RE.search(window):
        return True
    # Enclosing markdown section: nearest heading above, capped.
    start = max(0, idx - SECTION_CAP)
    for j in range(idx - 1, start - 1, -1):
        if lines[j].startswith("#"):
            section = "\n".join(lines[j : idx + 1])
            return bool(HIST_RE.search(section))
    return False


files = git_md_files()
print(
    f"=== INVENTORY: {len(files)} live-corpus .md files "
    "(historical snapshots excluded) ===\n"
)

# ── 1. Broken references ────────────────────────────────────────────────
print("=== 1. BROKEN REFERENCES (links to non-existent files) ===")
broken = defaultdict(list)
for f in files:
    rel = str(f.relative_to(REPO))
    text = f.read_text(errors="replace")
    if rel == "CHANGELOG.md":
        text = unreleased_only(text)
    lines = text.splitlines()
    for i, line in enumerate(lines):
        refs = set()
        for m in re.finditer(r"\]\(([^)#\s]+)\)", line):
            refs.add(m.group(1))
        for m in re.finditer(
            r"`((?:docs|src|scripts|benchmark|\.github)/"
            r"[\w\-./]+\.(?:rs|md|sh|py|toml|yml))`",
            line,
        ):
            refs.add(m.group(1))
        for ref in refs:
            if ref.startswith(("http", "mailto:", "#")):
                continue
            # Resolve: file-relative first, then repo-root (backtick code
            # refs like `src/config/configfile.rs` are root-relative by
            # convention).
            if (f.parent / ref).resolve().exists():
                continue
            if (REPO / ref).resolve().exists():
                continue
            if ref.startswith("./") and (f.parent / ref[2:]).resolve().exists():
                continue
            if context_exempt(lines, i):
                continue  # intentional history framing
            broken[rel].append(ref)
for f, refs in sorted(broken.items()):
    for r in sorted(refs):
        print(f"  {f}: {r}")
if not broken:
    print("  (none)")

# ── 2. Stale source paths (pre-refactor flat layout) ───────────────────
# Truth notes refreshed 2026-09-14 (NIGHT-hunt-46 & docs-7) and
# 2026-09-20 (NIGHT-hunt-5): the engine modules live under src/engine/
# since the tree restructure, and the power subsystem is
# src/central_control_power_dragon/ (power_dragon, not dragon_power).
print("\n=== 2. STALE SOURCE PATHS (old flat layout, moved modules) ===")
stale_patterns = [
    (r"src/cloud/", "moved to src/engine/cosmic_dragon_engine/cloud/"),
    (r"src/frame\.rs", "moved to src/engine/cosmic_dragon_engine/frame.rs"),
    (r"src/terminal\.rs", "moved to src/engine/cosmic_dragon_engine/terminal/"),
    (r"src/chroma_dragon_engine\.rs", "moved to src/engine/chroma_dragon_engine/"),
    (r"src/chroma/", "moved to src/engine/chroma_dragon_engine/"),
    (
        r"src/adaptive\.rs",
        "moved to src/central_control_power_dragon/ + src/interactive/adaptive.rs",
    ),
    (
        r"src/ambient_scheduler\.rs",
        "moved to src/engine/crystal_dragon_engine/ambient_scheduler/",
    ),
    (r"src/ambient\.rs", "moved to src/engine/crystal_dragon_engine/ambient/"),
    (r"src/config\.rs", "moved to src/config/"),
    (
        r"src/palette\.rs",
        "moved (palette types live in src/engine/chroma_dragon_engine/palette/)",
    ),
    (r"src/constants\.rs(?! *\))", "lifted to src/types/constants.rs (re-exported)"),
    (r"src/rain_style\.rs", "moved to src/types/rain_style.rs"),
    (r"src/self_healer\.rs", "moved to src/central_control_power_dragon/self_healer/"),
    (
        r"src/power_manager\.rs",
        "moved to src/central_control_power_dragon/power_manager/",
    ),
    (r"src/endurance_health\.rs", "moved to src/central_control_power_dragon/"),
    (r"src/reclaim_state\.rs", "moved to src/central_control_power_dragon/"),
    (r"src/thermal_sampler\.rs", "moved to src/central_control_power_dragon/"),
    (r"src/phase_predictor\.rs", "moved to src/central_control_power_dragon/"),
    (
        r"interactive/adaptive\.rs:\d+",
        "adaptive.rs is a re-export hub; subsystems moved to src/central_control_power_dragon/",
    ),
]
stale_hits = defaultdict(lambda: defaultdict(list))
for f in files:
    rel = str(f.relative_to(REPO))
    text = f.read_text(errors="replace")
    if rel == "CHANGELOG.md":
        text = unreleased_only(text)
    lines = text.splitlines()
    for i, line in enumerate(lines):
        for pat, note in stale_patterns:
            if re.search(pat, line):
                if context_exempt(lines, i):
                    continue
                stale_hits[rel][pat].append(i + 1)
for f in sorted(stale_hits):
    print(f"  {f}:")
    for pat, line_nos in sorted(stale_hits[f].items()):
        note = dict(stale_patterns)[pat]
        print(
            f"    {pat}  x{len(line_nos)}  (lines {line_nos[:6]}"
            f"{'...' if len(line_nos) > 6 else ''})  [{note}]"
        )
if not stale_hits:
    print("  (none)")

# ── 3. Stale counts / version claims ────────────────────────────────────
print("\n=== 3. STALE COUNT CLAIMS (source of truth in parens) ===")
count_patterns = [
    (
        r"\b43 themes\b|\b43 builtin\b|themes\*? == 43",
        "44 themes (src/theme/mod.rs THEME_COUNT)",
    ),
    (r"\b18 invariants\b", "19 invariants (chroma lock suite)"),
    (r"\b1[45]\d\d\+? tests\b", "2952 test fns (current static count)"),
    (r"~1,?500 tests\b", "2952 test fns (current static count)"),
    (r"\b220\+ source files\b|\b226 source\b", "508 tracked .rs files (current)"),
    (r"~2,?500 tests\b|\b252[0-9] tests\b", "2952 test fns (current static count)"),
    (r"\b43[0-9] \.rs files\b", "508 tracked .rs files (current)"),
    (r"Phase 9-B", "Phase 9-D (final form, CHROMA_DRAGON_ENGINE_VERSION)"),
]
count_hits = defaultdict(list)
for f in files:
    rel = str(f.relative_to(REPO))
    text = f.read_text(errors="replace")
    if rel == "CHANGELOG.md":
        text = unreleased_only(text)
    lines = text.splitlines()
    for i, line in enumerate(lines):
        for pat, note in count_patterns:
            if re.search(pat, line):
                if context_exempt(lines, i):
                    continue
                count_hits[rel].append((re.search(pat, line).group(0), note))
for f in sorted(count_hits):
    print(f"  {f}:")
    for hit, note in count_hits[f]:
        print(f"    '{hit}'  ->  {note}")
if not count_hits:
    print("  (none)")

# ── 4. Duplicate detection ──────────────────────────────────────────────
print("\n=== 4. DUPLICATE / OVERLAP CANDIDATES ===")
titles = defaultdict(list)
for f in files:
    rel = str(f.relative_to(REPO))
    text = f.read_text(errors="replace")
    m = re.search(r"^# (.+)$", text, re.MULTILINE)
    title = m.group(1).strip() if m else "(no H1)"
    titles[title].append((rel, len(text.splitlines())))


# group by normalized title
def norm(t):
    t = t.lower()
    t = re.sub(r"[^a-z0-9 ]+", " ", t)
    t = re.sub(r"\b(a|an|the|of|for|and|v\d+|audit|doc|document|notes?)\b", " ", t)
    return re.sub(r"\s+", " ", t).strip()


groups = defaultdict(list)
for title, lst in titles.items():
    groups[norm(title)].extend([(title, *x) for x in lst])
for key, lst in sorted(groups.items()):
    if len(lst) > 1:
        print(f"  similar title group: {key!r}")
        for title, rel, line_nos in sorted(lst):
            print(f"    {line_nos:5d} ln  {rel}   ({title})")

# content-signature duplicates: shared first-300-chars normalized
sigs = defaultdict(list)
for f in files:
    rel = str(f.relative_to(REPO))
    text = f.read_text(errors="replace")
    body = re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
    body = re.sub(r"[^a-z0-9]", "", body.lower())[:400]
    sigs[body].append(rel)
for sig, lst in sorted(sigs.items()):
    if len(lst) > 1:
        print(f"  IDENTICAL PREFIX group: {lst}")

print("\n=== DONE ===")
