#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# dragon-history.sh — the simple commit-history view for the three
# dragon engines (chroma / cosmic / crystal), per the v100 LTS lock
# protocol. Owner request 2026-09-12: "simple method to show history
# commit using git log folder dragon engine".
#
# Usage:
#   ./scripts/dragon-history.sh                  # full history, all engines (newest first)
#   ./scripts/dragon-history.sh --since-lock     # commits touching the engines since the LOCK_AT boundary
#   ./scripts/dragon-history.sh --per-engine     # per-engine last-commit + count summary
#   ./scripts/dragon-history.sh A..B             # history within a commit range (e.g. 9c36a049..HEAD)
#   ./scripts/dragon-history.sh -n 30            # entry limit (default 15)
#
# The raw git command behind the default view (manual use):
#   git log --oneline -- \
#     src/engine/chroma_dragon_engine \
#     src/engine/cosmic_dragon_engine \
#     src/engine/crystal_dragon_engine
#
# The lock boundary (LOCK_AT) is the commit after which the three
# engines are frozen for the v100 stable-LTS cut — engine-folder
# commits newer than LOCK_AT are lock-audit material (they require an
# UNLOCK entry in the touched engine's KEY.md/RULES.md, per the lock
# protocol). Update LOCK_AT whenever a new lock round is signed.

set -euo pipefail

# ── Lock boundary (update on every new lock round) ────────────────────────
# 2026-09-12 round: NIGHT-hunter-34 shadow-honesty fix (cosmic terminal
# files) + retroactive depthtest-3 chroma colors-custom hardening.
LOCK_AT="100771483469daa179d74d3682c66f3c29f9c584"

DRAGON_PATHS=(
	src/engine/chroma_dragon_engine
	src/engine/cosmic_dragon_engine
	src/engine/crystal_dragon_engine
)

LIMIT=15
RANGE=""
MODE="full"

usage() {
	sed -n '2,30p' "$0" | rg -v '^#!' | sed 's/^# \{0,1\}//'
	exit 0
}

while [ $# -gt 0 ]; do
	case "$1" in
	--since-lock)
		MODE="since-lock"
		shift
		;;
	--per-engine)
		MODE="per-engine"
		shift
		;;
	-n)
		LIMIT="$2"
		shift 2
		;;
	-h | --help)
		usage
		;;
	*..*)
		RANGE="$1"
		shift
		;;
	*)
		echo "unknown argument: $1 (see --help)" >&2
		exit 2
		;;
	esac
done

case "$MODE" in
full)
	if [ -n "$RANGE" ]; then
		git log --oneline -n "$LIMIT" "$RANGE" -- "${DRAGON_PATHS[@]}"
	else
		git log --oneline -n "$LIMIT" -- "${DRAGON_PATHS[@]}"
	fi
	;;
since-lock)
	echo "# engine commits since lock ${LOCK_AT:0:8} (audit trail — each needs an UNLOCK entry):"
	git log --oneline "${LOCK_AT}..HEAD" -- "${DRAGON_PATHS[@]}"
	;;
per-engine)
	for path in "${DRAGON_PATHS[@]}"; do
		count=$(git rev-list --count HEAD -- "$path")
		last=$(git log -1 --format='%h %ad %s' --date=short -- "$path")
		printf '%-34s %5s commits | last: %s\n' "$path" "$count" "$last"
	done
	echo ""
	echo "since lock ${LOCK_AT:0:8}:"
	git log --oneline "${LOCK_AT}..HEAD" -- "${DRAGON_PATHS[@]}" || true
	;;
esac
