#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
#
# COSMOSTRIX RUST SOURCE FILE LOC CHECK
#
# Ensures all Rust source files stay under the hard LOC cap.
# See src/RULES_LOC.md for the full policy (hard/soft limits,
# when to split, generated-code exemption).
#
# Default hard limit: 800 lines (soft target: 500, not enforced).
# Previous cap was 1500 — migration is incremental. Files still
# over 800 are listed in EXEMPT_BELOW_800 and tracked for refactor.
# As each file is refactored below 800, remove it from the list.
# When the list is empty, the codebase is fully 800-compliant.
#
# Usage: scripts/check-rs-loc.sh [MAX_LINES]
#   MAX_LINES: override the default limit (default: 800)
#
# Platform: UNIX-only (uses `find`, `wc -l`, `sort`). Not for Windows cmd.exe.

set -euo pipefail

MAX_LINES="${1:-800}"
FAILED=0
FOUND=0
EXEMPT_VIOLATIONS=0

# Migration exemption list: files still over 800 LOC, tracked for
# incremental refactor. Each entry is a relative path from repo root.
# To remove an entry: refactor the file below 800, verify tests pass,
# then delete the line. The list MUST shrink over time — adding new
# entries requires a justification comment.
#
# Format: one path per line, no leading ./
EXEMPT_BELOW_800="
src/main.rs
src/interactive/event_loop.rs
src/cosmic_dragon_engine/cloud/tests/tests_quantum.rs
src/cosmic_dragon_engine/cloud/rain.rs
src/droplet/mod.rs
# Pure data file: 44-theme registry (ThemeDef entries, no logic).
# Exempt per src/RULES_LOC.md 'When NOT to Split' (generated-like data).
src/chroma_dragon_engine/catalog/themes.rs
"

echo "Rust source file line counts (max ${MAX_LINES}):"
echo ""

# Collect all .rs files under src/ plus any root .rs files
FILES=$(find src -name '*.rs' 2>/dev/null | sort)

if [ -z "$FILES" ]; then
        echo "No .rs files found under src/"
        exit 0
fi

# Compute and display line counts sorted descending
while IFS= read -r f; do
        LINES=$(wc -l <"$f")
        printf "  %5d  %s\n" "$LINES" "$f"
        if [ "$LINES" -gt "$MAX_LINES" ]; then
                # Check if this file is in the migration exemption list
                if echo "$EXEMPT_BELOW_800" | grep -qxF "$f"; then
                        EXEMPT_VIOLATIONS=$((EXEMPT_VIOLATIONS + 1))
                else
                        FAILED=$((FAILED + 1))
                        echo "    ^^^ VIOLATES ${MAX_LINES} limit (NOT in exemption list)"
                fi
        fi
        FOUND=$((FOUND + 1))
done <<<"$FILES"

echo ""
echo "Total files: ${FOUND}"
echo "Files over ${MAX_LINES} (exempt / tracked for refactor): ${EXEMPT_VIOLATIONS}"
echo "Files over ${MAX_LINES} (NOT exempt — BUILD FAIL): ${FAILED}"

if [ "$FAILED" -gt 0 ]; then
        echo ""
        echo "FAIL: ${FAILED} file(s) exceed ${MAX_LINES} lines and are NOT in the"
        echo "migration exemption list. Either refactor them below ${MAX_LINES} or,"
        echo "if genuinely cohesive, add them to EXEMPT_BELOW_800 with a comment."
        exit 1
fi

if [ "$EXEMPT_VIOLATIONS" -gt 0 ]; then
        echo ""
        echo "OK (with migration debt): ${EXEMPT_VIOLATIONS} file(s) exceed ${MAX_LINES}"
        echo "but are tracked in EXEMPT_BELOW_800. Refactor incrementally — see"
        echo "src/RULES_LOC.md 'Migration Path' section."
        exit 0
fi

echo "OK: all files at or below ${MAX_LINES} lines (exemption list empty)"
exit 0
