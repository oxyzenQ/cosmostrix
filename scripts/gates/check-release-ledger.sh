#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). Uses awk, grep, sed and
#   the sourced ledger probes. Not for Windows cmd.exe / PowerShell
#   - use Git Bash or WSL on Windows.
#
# COSMOSTRIX RELEASE-LEDGER SHAPE CONTRACT CHECK
#
# The CHANGELOG.md release ledger is a machine-checked shape, not
# prose the owner's eyes re-audit every push. version-to.sh verifies
# its own writes (scripts/lib/changelog-ledger.sh, the zelynic
# NIGHT-bump-1 port); this gate catches hand-edit drift BETWEEN
# bumps - the exact window where the fatal class the owner hit on
# the v80.0.x line used to live unnoticed: the bump's release line
# rode below the unreleased section because nothing ever verified
# where a write landed.
#
# Laws asserted against the real CHANGELOG.md (read-only):
#   1. Heading law: exactly one '## Released' and one
#      '## Unreleased' heading.
#   2. Order law: '## Released' sits above '## Unreleased'.
#   3. Spacer law: a blank line under the '## Released' heading.
#   4. Purity law: only blank lines and bare vX.Y.Z version lines
#      inside '## Released' - foreign content refuses.
#   5. Strand law: no bare version line anywhere outside
#      '## Released' (the incident class).
#   6. Duplicate law: no release line repeats.
#
# Usage: bash scripts/gates/check-release-ledger.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHANGELOG="${REPO_ROOT}/CHANGELOG.md"

# The ledger probe functions are the writer's own - one
# implementation, no copied grammar.
# shellcheck disable=SC1091 # lib/ modules are analyzed standalone by
# their own shellcheck run
source "${REPO_ROOT}/scripts/lib/changelog-ledger.sh"

FAILS=0

fail() {
	echo "FAIL: $1"
	FAILS=$((FAILS + 1))
}

ok() {
	echo "OK: $1"
}

if [ ! -f "${CHANGELOG}" ]; then
	echo "FAIL: CHANGELOG.md not found at ${CHANGELOG} (tree fault: the release ledger is missing)"
	exit 1
fi

# 1. Heading law: exactly one of each.
rcount="$(grep -c '^## Released$' "${CHANGELOG}" || true)"
ucount="$(grep -c '^## Unreleased$' "${CHANGELOG}" || true)"
if [ "${rcount}" -eq 1 ] && [ "${ucount}" -eq 1 ]; then
	ok "heading law: exactly one ## Released and one ## Unreleased"
else
	fail "heading law: found ${rcount} '## Released' and ${ucount} '## Unreleased' heading(s); expected exactly one of each"
fi

# 2. Order law: Released above Unreleased.
if ledger_order_ok "${CHANGELOG}"; then
	ok "order law: ## Released sits above ## Unreleased"
else
	fail "order law: ## Released sits below ## Unreleased, or a heading count is off (relocate the block above the unreleased work)"
fi

# 3. Spacer law.
if [ "${rcount}" -eq 1 ] && ledger_spacer_blank "${CHANGELOG}"; then
	ok "spacer law: blank line under the ## Released heading"
else
	fail "spacer law: the blank line under ## Released is gone (the owner shape is heading, blank line, version lines)"
fi

# 4. Purity law.
imp="$(ledger_impurities "${CHANGELOG}" | head -3 || true)"
if [ -z "${imp}" ]; then
	ok "purity law: only blank lines and bare version lines inside ## Released"
else
	fail "purity law: foreign content inside ## Released (first findings):"
	printf '%s\n' "${imp}"
fi

# 5. Strand law: no bare version line outside ## Released. The
# whole-file count minus the inside count is the strand count.
vtotal="$(grep -cE '^v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$' "${CHANGELOG}" || true)"
vinside="$(ledger_versions "${CHANGELOG}" | wc -l | tr -d ' ')"
if [ "${vtotal}" -eq "${vinside}" ]; then
	ok "strand law: no version line stranded outside ## Released (${vinside} release line(s) in section)"
else
	fail "strand law: $((vtotal - vinside)) version line(s) stranded outside ## Released - the incident class; move them inside the section"
fi

# 6. Duplicate law.
dups="$(ledger_versions "${CHANGELOG}" | sort | uniq -d || true)"
if [ -z "${dups}" ]; then
	ok "duplicate law: no release line repeats"
else
	fail "duplicate law: repeated release line(s):"
	printf '%s\n' "${dups}"
fi

echo ""
if [ "${FAILS}" -ne 0 ]; then
	echo "release ledger shape contract: ${FAILS} law(s) broken"
	exit 1
fi
echo "release ledger shape contract: all laws hold"
