#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
#
# COSMOSTRIX STRICT CI BUILD WRAPPER
#
# Wraps a cargo invocation so that:
#   1. Any compiler/clippy warning is treated as a hard failure
#      (belt-and-suspenders on top of the global RUSTFLAGS="-D
#      warnings" env: catches warnings emitted by build scripts,
#      proc-macros, or any rustc invocation that ignores RUSTFLAGS).
#   2. When the build fails OR emits any warning/error line, a clean
#      "FAILED CI BUILD" summary block is printed at the end of the
#      step, listing every warning/error line so the failure is
#      immediately visible in the GitHub Actions log without
#      scrolling through thousands of compilation lines.
#
# Policy: warning is not ignored, can trigger like failed build
# even with warning. High quality and clean build because warning
# is not ignored.
#
# Usage:
#   bash scripts/ci-strict-build.sh -- build --profile dev --locked
#   bash scripts/ci-strict-build.sh -- test --all --locked
#   bash scripts/ci-strict-build.sh -- clippy --all-targets -- -D warnings
#
# The "--" separator is required: everything after it is the cargo
# subcommand and its arguments. The wrapper itself adds NO flags to
# cargo — strictness comes from RUSTFLAGS + post-hoc warning scan.
#
# Exit codes:
#   0  Clean build: zero warnings, zero errors, cargo exited 0.
#   1  Strict policy triggered: warning or error detected, or cargo
#      exited non-zero. The FAILED CI BUILD block is printed.
#   2  Usage error (missing -- or no subcommand).

set -euo pipefail

if [[ "${1:-}" != "--" ]]; then
	echo "Usage: $0 -- <cargo subcommand> [args...]" >&2
	echo "Example: $0 -- build --profile dev --locked" >&2
	exit 2
fi
shift # consume the --

if [[ $# -lt 1 ]]; then
	echo "Error: no cargo subcommand provided after --" >&2
	exit 2
fi

# Temp log file for post-mortem analysis. mktemp with no args uses
# $TMPDIR (or /tmp) — works identically on Linux, macOS, and FreeBSD
# CI runners. Cleaned up on exit via trap.
TMP_LOG="$(mktemp)"
trap 'rm -f "${TMP_LOG}"' EXIT

# Run cargo, teeing output to both stdout (live log) and the temp
# file. Disable pipefail + set +e so we can capture cargo's exit
# code via PIPESTATUS[0] (the first command in the pipeline).
set +e
cargo "$@" 2>&1 | tee "${TMP_LOG}"
CARGO_EXIT=${PIPESTATUS[0]}
set -e

# Extract warning and error lines from the captured log.
#
# Pattern matches the standard rustc/cargo diagnostic prefixes:
#   "warning:"        rustc/cargo warnings
#   "warning["        rustc warnings with diagnostic code (rare form)
#   "error:"          generic cargo errors
#   "error["          rustc errors with diagnostic code (e.g. error[E0308])
#
# grep -E is POSIX-portable. The || true prevents set -e from
# aborting when grep finds no matches (grep returns 1 on no match).
WARN_LINES="$(grep -E '^(warning|error)(\[|:)' "${TMP_LOG}" || true)"
WARN_COUNT="$(grep -cE '^warning(\[|:)' "${TMP_LOG}" || true)"
ERR_COUNT="$(grep -cE '^error(\[|:)' "${TMP_LOG}" || true)"

# Handle the case where grep -c returns "0" with leading whitespace
# on some platforms. Coerce to integer.
WARN_COUNT="${WARN_COUNT// /}"
ERR_COUNT="${ERR_COUNT// /}"
[[ -z "${WARN_COUNT}" ]] && WARN_COUNT=0
[[ -z "${ERR_COUNT}" ]] && ERR_COUNT=0

# Strict policy: any warning OR any error OR non-zero cargo exit = failure.
if [[ -n "${WARN_LINES}" || ${CARGO_EXIT} -ne 0 ]]; then
	echo ""
	echo "================================================"
	echo "FAILED CI BUILD - strict policy triggered"
	echo "================================================"
	echo "Command:  cargo $*"
	echo "Exit:     ${CARGO_EXIT}"
	echo "Warnings: ${WARN_COUNT}"
	echo "Errors:   ${ERR_COUNT}"
	echo "------------------------------------------------"
	echo "${WARN_LINES}"
	echo "================================================"
	# Force failure if cargo didn't already (warning-only case).
	# When cargo already failed, exit 1 still surfaces the failure
	# cleanly to GitHub Actions with the summary block above.
	exit 1
fi

echo "Clean build: 0 warnings, 0 errors."
exit 0
