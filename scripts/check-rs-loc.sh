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
#
# Exemption mechanism: NO hardcoded file list. Instead, each file that
# legitimately exceeds 800 LOC self-declares with a marker comment:
#
#   // LOC_EXEMPT: <one-line justification>
#
# The script dynamically scans every .rs file under src/ (recursive)
# PLUS build.rs — the exact scope documented in src/RULES_LOC.md
# ("All `.rs` files under `src/`, plus `build.rs`"). The mirrored
# test/ tree (NIGHT-hunter-1) is intentionally OUT of scope: the cap
# governs production source only (src/RULES_LOC.md "When NOT to
# Split"; NIGHT-hunter-5 sync). For any file over the limit, it greps
# for the marker. If found -> exempt (tracked debt). If not -> FAIL.
#
# Benefits:
# - No hardcoded paths in this script (they drift out of sync).
# - The exemption lives WITH the file it exempts.
# - Removing an exemption = delete the marker comment (no script edit).
# - The justification is visible at the top of the exempt file.
#
# Usage: scripts/check-rs-loc.sh [MAX_LINES]
#   MAX_LINES: override the default limit (default: 800)
#
# Platform: UNIX-only (uses `find`, `wc -l`, `grep`). Not for Windows cmd.exe.

set -euo pipefail

MAX_LINES="${1:-800}"
FAILED=0
FOUND=0
EXEMPT_VIOLATIONS=0

# Marker that a file uses to self-declare an LOC exemption.
# Must be followed by a justification (one line, free-form text).
EXEMPT_MARKER='// LOC_EXEMPT:'

echo "Rust source file line counts (max ${MAX_LINES}):"
echo ""

# Dynamically collect all .rs files under src/ (recursive) plus
# build.rs (the RULES_LOC.md policy scope). No hardcoding.
FILES=$( (
	find src -name '*.rs' 2>/dev/null
	{ [ -f build.rs ] && echo build.rs; } || true
) | sort)

if [ -z "$FILES" ]; then
	echo "No .rs files found under src/ (or build.rs)"
	exit 0
fi

# Compute and display line counts sorted descending
while IFS= read -r f; do
	LINES=$(wc -l <"$f")
	printf "  %5d  %s\n" "$LINES" "$f"
	if [ "$LINES" -gt "$MAX_LINES" ]; then
		# Dynamically check if the file self-declares an exemption
		# via the marker comment (no hardcoded list lookup).
		if grep -qF "$EXEMPT_MARKER" "$f"; then
			EXEMPT_VIOLATIONS=$((EXEMPT_VIOLATIONS + 1))
		else
			FAILED=$((FAILED + 1))
			echo "    ^^^ VIOLATES ${MAX_LINES} limit (no // LOC_EXEMPT: marker found)"
			echo "           Either refactor below ${MAX_LINES}, or add a marker comment:"
			echo "               // LOC_EXEMPT: <one-line justification>"
		fi
	fi
	FOUND=$((FOUND + 1))
done <<<"$FILES"

echo ""
echo "Total files: ${FOUND}"
echo "Files over ${MAX_LINES} (exempt via // LOC_EXEMPT: marker): ${EXEMPT_VIOLATIONS}"
echo "Files over ${MAX_LINES} (NOT exempt — BUILD FAIL): ${FAILED}"

if [ "$FAILED" -gt 0 ]; then
	echo ""
	echo "FAIL: ${FAILED} file(s) exceed ${MAX_LINES} lines without a"
	echo "// LOC_EXEMPT: marker. Either refactor them below ${MAX_LINES}, or"
	echo "add the marker with a justification:"
	echo "    // LOC_EXEMPT: <reason this file cannot be split>"
	exit 1
fi

if [ "$EXEMPT_VIOLATIONS" -gt 0 ]; then
	echo ""
	echo "OK (with migration debt): ${EXEMPT_VIOLATIONS} file(s) exceed ${MAX_LINES}"
	echo "but self-declare exemption via // LOC_EXEMPT: marker."
	echo "Refactor incrementally — see src/RULES_LOC.md 'Migration Path' section."
	exit 0
fi

echo "OK: all files at or below ${MAX_LINES} lines (no exemptions needed)"
exit 0
