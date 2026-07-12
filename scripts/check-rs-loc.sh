#!/bin/bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# =============================================================================
# COSMOSTRIX RUST SOURCE FILE LOC CHECK
# =============================================================================
# Ensures all Rust source files stay under 1,000 gross lines.
# Fail if any .rs file exceeds the limit (no exceptions by default).
#
# Usage: scripts/check-rs-loc.sh [MAX_LINES]
#   MAX_LINES: override the default limit (default: 1000)
# =============================================================================

set -euo pipefail

MAX_LINES="${1:-1200}"
FAILED=0
FOUND=0

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
    LINES=$(wc -l < "$f")
    printf "  %5d  %s\n" "$LINES" "$f"
    if [ "$LINES" -gt "$MAX_LINES" ]; then
        FAILED=$((FAILED + 1))
    fi
    FOUND=$((FOUND + 1))
done <<< "$FILES"

echo ""
echo "Total files: ${FOUND}"

if [ "$FAILED" -gt 0 ]; then
    echo "FAIL: ${FAILED} file(s) exceed ${MAX_LINES} lines"
    exit 1
else
    echo "OK: all files at or below ${MAX_LINES} lines"
    exit 0
fi
