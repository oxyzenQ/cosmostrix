#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Docs audit engine: broken refs, stale paths, stale counts, duplicates.

rg-philosophy: pattern-driven sweeps over git-tracked .md files, excluding
docs/archive/** and benchmark/bench-labs/sweep_* (archive is excluded per
owner directive; sweep files are auto-generated artifacts).
"""

import re
import subprocess
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def git_md_files():
    out = subprocess.run(
        ["git", "ls-files", "*.md"], cwd=REPO, capture_output=True, text=True
    ).stdout.splitlines()
    return [
        REPO / f
        for f in out
        if not f.startswith("docs/archive/")
        and not re.match(r"benchmark/bench-labs/sweep_", f)
    ]


files = git_md_files()
print(
    f"=== INVENTORY: {len(files)} tracked .md files (archive + sweeps excluded) ===\n"
)

# ── 1. Broken references ────────────────────────────────────────────────
print("=== 1. BROKEN REFERENCES (links to non-existent files) ===")
link_re = re.compile(
    r"\]\(([^)#\s]+)\)|`((?:docs|src|scripts|benchmark|\.github)/[\w\-./]+)`"
)
broken = defaultdict(list)
for f in files:
    text = f.read_text(errors="replace")
    refs = set()
    for m in re.finditer(r"\]\(([^)#\s]+)\)", text):
        refs.add(m.group(1))
    for m in re.finditer(
        r"`((?:docs|src|scripts|benchmark|\.github)/[\w\-./]+\.(?:rs|md|sh|py|toml|yml))`",
        text,
    ):
        refs.add(m.group(1))
    for ref in refs:
        if ref.startswith(("http", "mailto:", "#")):
            continue
        # Resolve: try file-relative first, then repo-root (backtick code
        # refs like `src/config/configfile.rs` are root-relative by convention).
        if (f.parent / ref).resolve().exists():
            continue
        if (REPO / ref).resolve().exists():
            continue
        # normalize ./ links
        if ref.startswith("./") and (f.parent / ref[2:]).resolve().exists():
            continue
        broken[str(f.relative_to(REPO))].append(ref)
for f, refs in sorted(broken.items()):
    for r in sorted(refs):
        print(f"  {f}: {r}")
if not broken:
    print("  (none)")

# ── 2. Stale source paths (pre-refactor flat layout) ───────────────────
print("\n=== 2. STALE SOURCE PATHS (old flat layout, moved modules) ===")
stale_patterns = [
    (r"src/cloud/", "moved to src/cosmic_dragon_engine/cloud/"),
    (r"src/frame\.rs", "moved to src/cosmic_dragon_engine/frame.rs"),
    (r"src/terminal\.rs", "moved to src/cosmic_dragon_engine/terminal/"),
    (r"src/chroma_dragon_engine\.rs", "moved to src/chroma_dragon_engine/"),
    (r"src/chroma/", "moved to src/chroma_dragon_engine/"),
    (
        r"src/adaptive\.rs",
        "moved to central_control_dragon_power/ + interactive/adaptive.rs",
    ),
    (
        r"src/ambient_scheduler\.rs",
        "moved to src/crystal_dragon_engine/ambient_scheduler/",
    ),
    (r"src/ambient\.rs", "moved to src/crystal_dragon_engine/ambient/"),
    (r"src/config\.rs", "moved to src/config/"),
    (r"src/palette\.rs", "moved (palette types live in chroma_dragon_engine/)"),
    (r"src/constants\.rs(?! *\))", "lifted to src/types/constants.rs (re-exported)"),
    (r"src/rain_style\.rs", "moved (RainStyle)"),
    (r"src/self_healer\.rs", "moved to src/central_control_dragon_power/self_healer/"),
    (
        r"src/power_manager\.rs",
        "moved to src/central_control_dragon_power/power_manager/",
    ),
    (r"src/endurance_health\.rs", "moved to src/central_control_dragon_power/"),
    (r"src/reclaim_state\.rs", "moved to src/central_control_dragon_power/"),
    (r"src/thermal_sampler\.rs", "moved to src/central_control_dragon_power/"),
    (r"src/phase_predictor\.rs", "moved to src/central_control_dragon_power/"),
    (
        r"interactive/adaptive\.rs:\d+",
        "adaptive.rs is a re-export hub; subsystems moved to central_control_dragon_power/",
    ),
]
stale_hits = defaultdict(lambda: defaultdict(list))
for f in files:
    rel = str(f.relative_to(REPO))
    text = f.read_text(errors="replace")
    for i, line in enumerate(text.splitlines(), 1):
        for pat, note in stale_patterns:
            if re.search(pat, line):
                stale_hits[rel][pat].append(i)
for f in sorted(stale_hits):
    print(f"  {f}:")
    for pat, lines in sorted(stale_hits[f].items()):
        note = dict(stale_patterns)[pat]
        print(
            f"    {pat}  x{len(lines)}  (lines {lines[:6]}{'...' if len(lines) > 6 else ''})  [{note}]"
        )
if not stale_hits:
    print("  (none)")

# ── 3. Stale counts / version claims ────────────────────────────────────
print("\n=== 3. STALE COUNT CLAIMS (source of truth in parens) ===")
count_patterns = [
    (r"\b43 themes\b|\b43 builtin\b|themes\*? == 43", "44 themes (catalog.rs)"),
    (r"\b18 invariants\b", "19 invariants (chroma lock suite)"),
    (r"\b1[45]\d\d\+? tests\b", "1649 tests (current suite)"),
    (r"~1,?500 tests\b", "1649 tests (current suite)"),
    (r"\b220\+ source files\b|\b226 source\b", "226 .rs files (current)"),
    (r"Phase 9-B", "Phase 9-D (final form)"),
]
count_hits = defaultdict(list)
for f in files:
    rel = str(f.relative_to(REPO))
    text = f.read_text(errors="replace")
    for pat, note in count_patterns:
        for m in re.finditer(pat, text):
            count_hits[rel].append((m.group(0), note))
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
        for title, rel, lines in sorted(lst):
            print(f"    {lines:5d} ln  {rel}   ({title})")

# content-signature duplicates: shared first-300-chars normalized
sigs = defaultdict(list)
for f in files:
    rel = str(f.relative_to(REPO))
    text = f.read_text(errors="replace")
    body = re.sub(r"<!--.*?-->", "", text, flags=re.S)
    body = re.sub(r"[^a-z0-9]", "", body.lower())[:400]
    sigs[body].append(rel)
for sig, lst in sorted(sigs.items()):
    if len(lst) > 1:
        print(f"  IDENTICAL PREFIX group: {lst}")

print("\n=== DONE ===")
