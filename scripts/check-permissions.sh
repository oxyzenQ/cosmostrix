#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
# cosmostrix File Permission Guard
#
# Owner rule (2026-09-13): git-tracked files carry 644, git-tracked
# executables and every repository directory carry 755. Wired into
# gate-keepers.sh as check 14, so a checkout with wrong permission
# bits fails the pre-commit gate instead of slipping into a push
# (the CI EXE001 incident class from run #1484: shebang'd scripts
# demoted to 100644 pass a local gate that lacks a permission check).
#
# Checks:
#   1. Tracked git mode 100644 -> filesystem mode 644
#   2. Tracked git mode 100755 -> filesystem mode 755
#   3. Every directory holding tracked files (and the repo root) -> 755
#   4. Shebang parity: a tracked file whose first line is #! must be
#      git-tracked 100755 (generalizes ruff EXE001 beyond scripts/*.py
#      to every tracked script, including .sh and benchmark/ tools)
#
# Usage:
#   bash scripts/check-permissions.sh          # check only
#   bash scripts/check-permissions.sh --fix    # chmod violations in place
#
# Scope notes:
#   - Only git-tracked paths are checked; target/, __pycache__/ and
#     other untracked build output never appear
#   - --fix changes permission bits only, never file content
#   - git records only the executable bit, so 644<->664 and 755<->775
#     repairs are invisible to git status; a 644<->755 flip (exec bit)
#     does show in git diff and needs to be committed
#   - A umask 002 clone materializes 664/775 bits; that is a violation
#     under the exact 644/755 contract and --fix restores them

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

FIX_MODE=false
if [[ "${1:-}" == "--fix" ]]; then
	FIX_MODE=true
elif [[ $# -gt 0 ]]; then
	echo "ERROR: unknown argument: $1 (expected --fix or none)"
	exit 1
fi

VIOLATIONS=0
SHOWN=0
MAX_SHOWN=20

# Show a violation line, capped at MAX_SHOWN detail lines so a badly
# broken checkout does not flood the gatekeeper output; the total is
# always reported in the summary.
report_violation() {
	VIOLATIONS=$((VIOLATIONS + 1))
	if [ "$SHOWN" -lt "$MAX_SHOWN" ]; then
		echo "FAIL: $*"
		SHOWN=$((SHOWN + 1))
	fi
}

# Portable filesystem mode query: GNU stat (-c %a) first, BSD stat
# (-f %Lp) second. Prints the octal mode, or fails when the path is
# unreadable (tracked file deleted without git rm, and similar).
fs_mode() {
	local path="$1"
	local mode=""
	if mode=$(stat -c '%a' "$path" 2>/dev/null); then
		echo "$mode"
	elif mode=$(stat -f '%Lp' "$path" 2>/dev/null); then
		echo "$mode"
	else
		return 1
	fi
}

# Apply a permission fix (or record a violation when not fixing).
fix_or_report() {
	local path="$1"
	local expected="$2"
	local actual="$3"
	local kind="$4"

	if $FIX_MODE; then
		if chmod "$expected" "$path" 2>/dev/null; then
			if [ "$SHOWN" -lt "$MAX_SHOWN" ]; then
				echo "fixed: $kind $path ${actual} -> ${expected}"
				SHOWN=$((SHOWN + 1))
			fi
		else
			report_violation "$kind $path is $actual, expected $expected (chmod failed)"
		fi
	else
		report_violation "$kind $path is $actual, expected $expected"
	fi
}

# ── 1 + 2. Tracked files: filesystem mode must match the git mode ──────────

FILES_CHECKED=0
EXEC_FILES=0
while read -r gmode _object _stage path; do
	case "$gmode" in
	100644)
		expected="644"
		;;
	100755)
		expected="755"
		EXEC_FILES=$((EXEC_FILES + 1))
		;;
	*)
		# Symlinks (120000) and any other exotic mode: out of scope.
		continue
		;;
	esac
	FILES_CHECKED=$((FILES_CHECKED + 1))

	actual=$(fs_mode "$path") || actual="unreadable"
	if [ "$actual" != "$expected" ]; then
		fix_or_report "$path" "$expected" "$actual" "file"
	fi
done < <(git ls-files -s)

# ── 3. Directories holding tracked files (plus the repo root): 755 ─────────

DIRS_CHECKED=0
while read -r dir; do
	[ -n "$dir" ] || continue
	DIRS_CHECKED=$((DIRS_CHECKED + 1))

	actual=$(fs_mode "$dir") || actual="unreadable"
	if [ "$actual" != "755" ]; then
		fix_or_report "$dir" "755" "$actual" "directory"
	fi
done < <(
	git ls-files |
		awk -F/ 'NF > 1 {
			for (i = 1; i < NF; i++) {
				p = $1
				for (j = 2; j <= i; j++) p = p "/" $j
				print p
			}
		}' |
		sort -u
)
# The repository root itself.
DIRS_CHECKED=$((DIRS_CHECKED + 1))
root_mode=$(fs_mode ".") || root_mode="unreadable"
if [ "$root_mode" != "755" ]; then
	fix_or_report "." "755" "$root_mode" "directory"
fi

# ── 4. Shebang parity: #! on line 1 requires git mode 100755 ───────────────
# git grep lists tracked files containing a #! line anywhere; the
# head -n 1 filter below keeps only true shebangs. This is the
# generalized EXE001 rule: it covers .sh, .py and any other scripted
# file, including benchmark/ tools that ruff never scans.

SHEBANG_CHECKED=0
while read -r candidate; do
	[ -n "$candidate" ] || continue
	first_line=$(head -n 1 "$candidate" 2>/dev/null) || continue
	case "$first_line" in
	'#!'*) ;;
	*)
		continue
		;;
	esac
	SHEBANG_CHECKED=$((SHEBANG_CHECKED + 1))

	gmode=$(git ls-files -s -- "$candidate" | awk '{print $1}')
	if [ "$gmode" != "100755" ]; then
		if $FIX_MODE; then
			if git update-index --chmod=+x "$candidate" &&
				chmod 755 "$candidate"; then
				echo "fixed: $candidate gained the executable bit (shebang present)"
			else
				report_violation "$candidate has a shebang but git mode $gmode (expected 100755; fix failed)"
			fi
		else
			report_violation "$candidate has a shebang but git mode $gmode (expected 100755)"
		fi
	fi
done < <(git grep -I -l '^#!' -- . 2>/dev/null || true)

# ── Summary ────────────────────────────────────────────────────────────────

if [ "$VIOLATIONS" -gt 0 ]; then
	if [ $((VIOLATIONS - SHOWN)) -gt 0 ]; then
		echo "  ... and $((VIOLATIONS - SHOWN)) more violation(s) not listed"
	fi
	echo "FAIL: $VIOLATIONS permission violation(s) across $FILES_CHECKED files, $DIRS_CHECKED directories, $SHEBANG_CHECKED shebang files"
	if ! $FIX_MODE; then
		echo "Fix: bash scripts/check-permissions.sh --fix (or gate-keepers.sh --fix-all)"
	fi
	exit 1
else
	echo "OK: $FILES_CHECKED tracked files at 644/755 ($EXEC_FILES executables), $DIRS_CHECKED directories at 755, shebang parity holds across $SHEBANG_CHECKED scripts"
	exit 0
fi
