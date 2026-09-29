#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
#
# COSMOSTRIX RELEASE-NOTES SHAPE CONTRACT CHECK
#
# Re-executes scripts/release/generate-release-notes.sh through its
# REAL command-line path (positional arguments, quoted, exactly the
# way .github/workflows/release.yml invokes it) against a hermetic
# throwaway git repository, and asserts the documented decision
# table of legal invocation shapes:
#
#   1. Stable release      PREV_TAG == LAST_STABLE (dual range
#                          collapses to a single "N commits since"
#                          line), [!TIP] alert, exit 0.
#   2. Pre-release build   [!WARNING] alert, exit 0.
#   3. Dual range          PREV_TAG != LAST_STABLE renders both
#                          "previous build" and "last stable"
#                          distances, exit 0.
#   4. Initial release     RANGE/PREV_TAG/LAST_STABLE all present
#                          but EMPTY (the workflow's first-release
#                          call passes "" "" "") — "Initial release."
#                          marker, exit 0. This is the legal-empty
#                          class: a present-but-empty argument is a
#                          documented legal state, never a usage
#                          error (zelynic NIGHT-dinner-26 lineage:
#                          its v11.0.0 stable publish died on a
#                          flag parser that rejected --since-stable
#                          "" — the cosmostrix generator is
#                          positional, and this probe keeps it
#                          honest about that contract).
#   5. Usage error         a missing positional argument still
#                          fails with usage + nonzero exit.
#
# The temp repo carries synthetic commits in the repo's own subject
# conventions so the classifier's git plumbing (log, name-only,
# md-only pass) runs end-to-end on the real path too.
#
# No network, no dependency on the host repository's history or
# tags (works in shallow CI checkouts), no writes outside mktemp.
#
# Usage: bash scripts/gates/check-release-notes-shapes.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GENERATOR="${REPO_ROOT}/scripts/release/generate-release-notes.sh"

if [ ! -f "${GENERATOR}" ]; then
	echo "FAIL: generate-release-notes.sh not found at ${GENERATOR}"
	exit 1
fi

# ── Hermetic fixture: a throwaway repo with the four-tag history ──
# v1.0.0 (stable base) -> v1.1.0-beta.1 (pre-release) ->
# v1.1.0-rc.1 (pre-release) -> v1.1.0 (stable cut).
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

fixture() {
	git init -q "${WORK}"
	git -C "${WORK}" config user.name "shape contract"
	git -C "${WORK}" config user.email "shape-contract@localhost"

	echo "fn main() {}" >"${WORK}/engine.rs"
	git -C "${WORK}" add engine.rs
	git -C "${WORK}" commit -qm "Internal research: fix the stable base renderer"
	git -C "${WORK}" tag v1.0.0

	echo "b" >"${WORK}/engine.rs"
	git -C "${WORK}" add engine.rs
	git -C "${WORK}" commit -qm "Internal research: feat the beta pipeline"
	git -C "${WORK}" tag v1.1.0-beta.1

	echo "# notes" >"${WORK}/NOTES.md"
	git -C "${WORK}" add NOTES.md
	git -C "${WORK}" commit -qm "Internal research: docs the release research record"
	git -C "${WORK}" tag v1.1.0-rc.1

	echo "c" >"${WORK}/engine.rs"
	git -C "${WORK}" add engine.rs
	git -C "${WORK}" commit -qm "release: v1.1.0"
	git -C "${WORK}" tag v1.1.0
}

# Run the generator from inside the fixture (it runs git log against
# the ambient repository) and print PASS/FAIL per shape.
SHAPES_PASS=0
SHAPES_FAIL=0

shape() {
	local name="$1" want_rc="$2" body="$3"
	shift 3
	local out rc=0
	out="$("${GENERATOR}" "$@" 2>&1)" || rc=$?

	if [ "${want_rc}" = "zero" ] && [ "${rc}" -ne 0 ]; then
		echo "FAIL: ${name}: expected exit 0, got ${rc}"
		SHAPES_FAIL=$((SHAPES_FAIL + 1))
		return
	fi
	if [ "${want_rc}" = "nonzero" ] && [ "${rc}" -eq 0 ]; then
		echo "FAIL: ${name}: expected a usage error, got exit 0"
		SHAPES_FAIL=$((SHAPES_FAIL + 1))
		return
	fi
	if ! printf '%s' "${out}" | grep -qF -- "${body}"; then
		echo "FAIL: ${name}: marker not rendered: ${body}"
		SHAPES_FAIL=$((SHAPES_FAIL + 1))
		return
	fi
	echo "OK: ${name}"
	SHAPES_PASS=$((SHAPES_PASS + 1))
}

fixture
cd "${WORK}"

# All direct greps below capture the generator's output first and
# grep the capture: a `generator | grep -q` pipeline under pipefail
# dies on SIGPIPE (grep -q exits at first match, the generator gets
# EPIPE mid-write, pipefail turns a found match into a false verdict).

# 1. Stable cut: PREV_TAG == LAST_STABLE -> single range + TIP alert.
shape "stable (single range)" zero "**Stable release.**" \
	"v1.0.0..v1.1.0" "v1.1.0" "false" "v1.0.0" "v1.0.0"
stable_out="$(${GENERATOR} "v1.0.0..v1.1.0" "v1.1.0" "false" "v1.0.0" "v1.0.0" 2>&1)"
if printf '%s' "${stable_out}" | grep -q "last stable"; then
	echo "FAIL: stable (single range): dual-range line rendered for PREV == LAST_STABLE"
	SHAPES_FAIL=$((SHAPES_FAIL + 1))
else
	echo "OK: stable (single range): no dual-range line for PREV == LAST_STABLE"
	SHAPES_PASS=$((SHAPES_PASS + 1))
fi

# 2. Pre-release build: WARNING alert.
shape "pre-release (warning alert)" zero "Pre-release build" \
	"v1.0.0..v1.1.0-beta.1" "v1.1.0-beta.1" "true" "v1.0.0" "v1.0.0"

# 3. Dual range: PREV != LAST_STABLE renders both distances.
shape "dual range (both distances)" zero "previous build" \
	"v1.1.0-beta.1..v1.1.0-rc.1" "v1.1.0-rc.1" "true" "v1.1.0-beta.1" "v1.0.0"
dual_out="$(${GENERATOR} "v1.1.0-beta.1..v1.1.0-rc.1" "v1.1.0-rc.1" "true" \
	"v1.1.0-beta.1" "v1.0.0" 2>&1)"
if printf '%s' "${dual_out}" | grep -q "last stable"; then
	echo "OK: dual range (both distances): last-stable distance rendered"
	SHAPES_PASS=$((SHAPES_PASS + 1))
else
	echo "FAIL: dual range (both distances): last-stable distance missing"
	SHAPES_FAIL=$((SHAPES_FAIL + 1))
fi

# 4. Initial release: every optional value present-but-empty (the
#    workflow's first-release call) through the real CLI path.
shape "initial release (legal-empty)" zero "Initial release." \
	"" "v2.0.0" "false" "" ""

# 5. Usage error: a missing positional argument must fail.
shape "usage error (missing argument)" nonzero "Usage:" \
	"v1.0.0..v1.1.0" "v2.0.0" "false"

# 6. Classifier plumbing: the repo-convention feat subject lands in
#    the achievements section (proves the log/name-only/md-only
#    passes produced bucketed output on the real path).
if printf '%s' "${stable_out}" | grep -q "<summary><strong>feat"; then
	echo "OK: classifier plumbing (feat bucket rendered)"
	SHAPES_PASS=$((SHAPES_PASS + 1))
else
	echo "FAIL: classifier plumbing (feat bucket missing from achievements)"
	SHAPES_FAIL=$((SHAPES_FAIL + 1))
fi

echo ""
echo "release-notes shape contract: ${SHAPES_PASS} passed, ${SHAPES_FAIL} failed"
if [ "${SHAPES_FAIL}" -ne 0 ]; then
	echo "FAIL: the release-notes generator broke its shape contract"
	exit 1
fi
echo "OK: release-notes generator honors its full shape contract"
exit 0
