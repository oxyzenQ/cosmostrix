#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
"""CAS census pin - the standing NO behind the interleaving-theorem
question (night-netrunner-Z2).

The owner's LTS deepen candidate asked whether cosmostrix needs the
zelynic night-frontier-Z17 instrument: an exhaustive for-all proof
over the CPU interleaving space of the concurrency protocol (every
ordering of CAS draw / refill / debt pay on the same bucket). The
2026-11-24 census answered NO, by measurement rather than taste:

  - src/ carries ZERO CAS-family primitives (compare_exchange,
    compare_exchange_weak, fetch_update). A lock-free multi-step
    protocol - the thing an interleaving theorem proves - cannot be
    built without them. The atomic variables are single-variable
    flags, counters and stamps whose laws ride the pinned tests.
  - src/ carries ZERO AtomicPtr and zero location-backed atomics
    (from_ptr / from_mut). The pointer-cast-over-shared-memory
    discipline zelynic's kernel datapath needs does not exist here.
  - The render cores are compiler-forbidden from unsafe (five
    forbid(unsafe_code) modules) and the screen state has a single
    writer by construction; helper threads touch flags and channels.
  - The ONE protocol with a genuine interleaving-sensitive invariant
    (the two-flag shutdown handshake, Release store / Acquire read)
    already carries the exhaustive instrument: the cfg(loom) model
    proves a fired shutdown can never be lost for every interleaving
    up to the preemption bound, on every push.

So the for-all tier over interleavings would prove for-all over an
empty set. This gate makes that NO standing instead of archival: it
re-reads the tree on every push and fails the day the basis moves -
the exact absence-guard shape the zelynic estate uses for its own
signed NOs. A finding here does not mean the code is wrong; it means
the interleaving-theorem question must be answered deliberately
again, with a fresh census, before any new protocol ships.

Three pins, all deterministic:

  1. Zero CAS-family method calls in src/ (comment lines ignored).
  2. Zero shared-memory pointer-cast primitives in src/
     (AtomicPtr, from_ptr, from_mut).
  3. Exactly one cfg(loom) module in src/ - the shutdown handshake
     model. Zero means the model vanished (tree fault); two or more
     means a second interleaving-sensitive protocol landed.
"""

from __future__ import annotations

import sys
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent.parent / "src"

# The CAS family: the only std primitives that can build a lock-free
# multi-step protocol with interleaving-sensitive transitions.
CAS_METHODS = (".compare_exchange(", ".compare_exchange_weak(", ".fetch_update(")

# The shared-memory pointer-cast class: atomics placed over raw
# memory (the kernel map-value shape), or pointers shared across
# threads. cosmostrix's law is that none of it exists. The method
# tokens carry the call-paren and the leading dot so identifier
# substrings (try_get_matches_from_mut and kin) never match.
POINTER_CAST_TOKENS = ("AtomicPtr", ".from_ptr(", ".from_mut(")

# Exactly one loom module is expected: the shutdown handshake model.
PINNED_LOOM_MODULES = 1

LOOM_MODULE_PREFIX = "#[cfg(loom)]"


def code_lines(path: Path) -> list[str]:
    """The file's lines minus whole-line comments (// and ///)."""
    lines = []
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        stripped = raw.strip()
        if stripped.startswith("//"):
            continue
        lines.append(raw)
    return lines


def main() -> int:
    if not SRC.is_dir():
        print(f"cas-census: FINDING - src/ not found at {SRC}")
        return 1

    cas_files: list[str] = []
    ptr_files: list[str] = []
    loom_modules: list[str] = []

    for path in sorted(SRC.rglob("*.rs")):
        lines = code_lines(path)
        rel = path.relative_to(SRC.parent)
        for line in lines:
            if any(m in line for m in CAS_METHODS):
                cas_files.append(f"{rel}: {line.strip()}")
            if any(t in line for t in POINTER_CAST_TOKENS):
                ptr_files.append(f"{rel}: {line.strip()}")
            if line.strip().startswith(LOOM_MODULE_PREFIX):
                loom_modules.append(str(rel))

    findings = []
    if cas_files:
        findings.append(
            f"CAS-family primitive(s) landed ({len(cas_files)} line(s)) - the "
            "interleaving-theorem question reopens: " + "; ".join(cas_files[:5])
        )
    if ptr_files:
        findings.append(
            "shared-memory pointer-cast primitive(s) landed "
            f"({len(ptr_files)} line(s)) - the interleaving-theorem question "
            "reopens: " + "; ".join(ptr_files[:5])
        )
    if len(loom_modules) != PINNED_LOOM_MODULES:
        findings.append(
            f"cfg(loom) module count is {len(loom_modules)}, pinned at "
            f"{PINNED_LOOM_MODULES} (the shutdown handshake model): "
            + (", ".join(loom_modules[:5]) if loom_modules else "none found")
        )

    if findings:
        for f in findings:
            print(f"cas-census: FINDING - {f}")
        print(
            "cas-census: the standing NO's basis moved - run a fresh "
            "concurrency census and answer the interleaving-theorem "
            "question deliberately (see docs/INNOVATIONS.md, Retired by "
            "verdict)"
        )
        return 1

    print(
        "cas-census: OK - zero CAS-family primitives, zero "
        "pointer-cast atomics, exactly one cfg(loom) interleaving model "
        "(the shutdown handshake, exhaustive for-all); the "
        "interleaving-theorem NO's basis holds"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
