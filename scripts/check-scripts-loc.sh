#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
#
# COSMOSTRIX SCRIPTS FILE LOC CHECK (NIGHT-lts-1)
#
# Ensures every shell and Python script under scripts/ (any depth,
# subdirectories included) stays at or below the hard LOC cap of
# 1000 gross lines. Faithful mirror of scripts/check-rs-loc.sh (the
# Rust 800-line cap): same gross-line counting (wc -l), same
# self-declaring exemption marker, same fail / OK-with-debt exit
# semantics. Policy statement: docs/RULES.md "Scripts file size".
#
# Exemption mechanism: NO hardcoded file list. Instead, each script
# that legitimately exceeds 1000 lines self-declares with a marker
# comment (the shell/Python comment form of the Rust // marker):
#
#   # LOC_EXEMPT: <one-line justification>
#
# The scan is recursive (find scripts/ -name '*.sh' -o -name
# '*.py'), so scripts/depthbore/ and any future category directory
# inherit the cap automatically. For any script over the limit, the
# marker greps as tracked debt; without it the check FAILS.
#
# Benefits (mirrors check-rs-loc.sh):
# - No hardcoded paths in this script (they drift out of sync).
# - The exemption lives WITH the script it exempts.
# - Removing an exemption = delete the marker comment (no script
#   edit).
# - The justification is visible at the top of the exempt script.
#
# Usage: scripts/check-scripts-loc.sh [MAX_LINES]
#   MAX_LINES: override the default limit (default: 1000)
#
# Platform: UNIX-only (uses `find`, `wc -l`, `grep`). Not for
# Windows cmd.exe.

set -euo pipefail

MAX_LINES="${1:-1000}"
FAILED=0
FOUND=0
EXEMPT_VIOLATIONS=0

# Marker that a script uses to self-declare an LOC exemption.
# Must be followed by a justification (one line, free-form text).
EXEMPT_MARKER='# LOC_EXEMPT:'

# Resolve the repository root from this script's own location so the
# check works from any working directory (same pattern as
# scripts/check-permissions.sh).
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "Scripts file line counts (max ${MAX_LINES}):"
echo ""

# Dynamically collect every .sh/.py script under scripts/ (recursive).
# No hardcoded file list: new subdirectories inherit the cap on their
# own.
FILES=$(find scripts \( -name '*.sh' -o -name '*.py' \) 2>/dev/null | sort)

if [ -z "$FILES" ]; then
	echo "No .sh/.py files found under scripts/"
	exit 0
fi

# Compute and display line counts (alphabetical file order)
while IFS= read -r f; do
	LINES=$(wc -l <"$f")
	printf "  %5d  %s\n" "$LINES" "$f"
	if [ "$LINES" -gt "$MAX_LINES" ]; then
		# Dynamically check if the script self-declares an exemption
		# via the marker comment (no hardcoded list lookup).
		if grep -qF "$EXEMPT_MARKER" "$f"; then
			EXEMPT_VIOLATIONS=$((EXEMPT_VIOLATIONS + 1))
		else
			FAILED=$((FAILED + 1))
			echo "    ^^^ VIOLATES ${MAX_LINES} limit (no # LOC_EXEMPT: marker found)"
			echo "           Either refactor below ${MAX_LINES}, or add a marker comment:"
			echo "               # LOC_EXEMPT: <one-line justification>"
		fi
	fi
	FOUND=$((FOUND + 1))
done <<<"$FILES"

echo ""
echo "Total scripts: ${FOUND}"
echo "Scripts over ${MAX_LINES} (exempt via # LOC_EXEMPT: marker): ${EXEMPT_VIOLATIONS}"
echo "Scripts over ${MAX_LINES} (NOT exempt - BUILD FAIL): ${FAILED}"

if [ "$FAILED" -gt 0 ]; then
	echo ""
	echo "FAIL: ${FAILED} script(s) exceed ${MAX_LINES} lines without a"
	echo "# LOC_EXEMPT: marker. Either refactor them below ${MAX_LINES}, or"
	echo "add the marker with a justification:"
	echo "    # LOC_EXEMPT: <reason this script cannot be split>"
	exit 1
fi

if [ "$EXEMPT_VIOLATIONS" -gt 0 ]; then
	echo ""
	echo "OK (with migration debt): ${EXEMPT_VIOLATIONS} script(s) exceed ${MAX_LINES}"
	echo "but self-declare exemption via # LOC_EXEMPT: marker."
	echo "Refactor incrementally - see docs/RULES.md 'Scripts file size'."
	exit 0
fi

echo "OK: all scripts at or below ${MAX_LINES} lines (no exemptions needed)"
exit 0
