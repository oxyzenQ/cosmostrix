#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
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
# The post-hoc diagnostic scan is a BUILD-lane check. A TEST lane
# (the test / nextest subcommands) is verdict-by-exit-code: the test
# harness echoes captured test output verbatim, and a hostile-input
# battery legitimately prints diagnostic-looking lines (the binary's
# own "error: ..." rejections), so a line-scan would fail a fully
# green suite (observed: the maintenance lane killed "3111 passed;
# 0 failed" on the snapback battery's own rejection lines). Compile
# warnings on a test lane stay hard-denied by the RUSTFLAGS env;
# cargo's exit code is the verdict.
#
# Usage:
#   bash scripts/ci/ci-strict-build.sh -- build --profile dev --locked
#   bash scripts/ci/ci-strict-build.sh -- test --all --locked
#   bash scripts/ci/ci-strict-build.sh -- clippy --all-targets -- -D warnings
#
# The "--" separator is required: everything after it is the cargo
# subcommand and its arguments. The wrapper itself adds NO flags to
# cargo — strictness comes from RUSTFLAGS + the post-hoc scan on
# build lanes / the exit code on test lanes.
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

# The lane switch (night-dragon-maintenance-12): test lanes are
# verdict-by-exit-code, build lanes carry the post-hoc diagnostic
# scan. See the header for the why. An alias invocation (the first
# argument is an alias name, e.g. pro-linux-amd64-v3-musl) defaults
# to the build lane - an alias expands to a build, and build
# diagnostics remain decisive.
SCAN_DIAGNOSTICS=1
case "$1" in
test | nextest)
	SCAN_DIAGNOSTICS=0
	;;
esac

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
# Build lanes: the diagnostic lines are decisive even at exit 0 - a
# warning is a failed build. Test lanes: only the exit code is
# decisive, because captured test output legitimately contains
# diagnostic-looking lines (see the header); when a test lane does
# fail, the banner below still prints those lines as the context.
if [[ ${CARGO_EXIT} -ne 0 ]] || { [[ "${SCAN_DIAGNOSTICS}" = 1 ]] && [[ -n "${WARN_LINES}" ]]; }; then
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

if [[ "${SCAN_DIAGNOSTICS}" = 1 ]]; then
	echo "Clean build: 0 warnings, 0 errors."
else
	echo "Clean test run: cargo exited 0; captured test output is not scanned (compile warnings stay denied by RUSTFLAGS)."
fi
exit 0
