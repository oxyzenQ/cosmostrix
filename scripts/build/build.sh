#!/usr/bin/env bash
#
# COSMOSTRIX BUILD AUTOMATION SCRIPT (entry point)
#
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Uses `nproc`, `rustc -vV`, `command -v`, bash arrays. On macOS `nproc`
#   is replaced by `sysctl -n hw.logicalcpu` if missing. Will not run on
#   Windows cmd.exe / PowerShell.
# ─────────────────────────────────────────────────────────────────────────────
#
# Single-entry orchestrator: argument parsing, option state and command
# dispatch live here; every command family is implemented in the sourced
# lib/ modules (NIGHT-lts-2 split of the former 2187-line monolith —
# byte-identical function moves, no behavior change):
#   lib/common.sh       logging, toolchain check, build cache, hardened flags
#   lib/builds.sh       debug/release builds, update, clean, stats, bench
#   lib/quality.sh      fmt/clippy/tests + every gate-check + check-all
#   lib/help.sh         the help text
#   lib/version-sync.sh version-sync verification
#   lib/miri.sh         Miri UB verification runner + status banner
#   lib/pgo.sh          the PGO nitro 3-stage pipeline
# See `./scripts/build/build.sh help` for the command list.
#
# shellcheck disable=SC1091 # lib/ modules are analyzed standalone by
# design; the cross-file contracts are documented in each module header

set -euo pipefail

# Resolve this script's directory so the lib/ modules can be sourced
# regardless of the caller's working directory.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${SCRIPT_DIR}/lib/common.sh"
source "${SCRIPT_DIR}/lib/builds.sh"
source "${SCRIPT_DIR}/lib/quality.sh"
source "${SCRIPT_DIR}/lib/help.sh"
source "${SCRIPT_DIR}/lib/version-sync.sh"
source "${SCRIPT_DIR}/lib/miri.sh"
source "${SCRIPT_DIR}/lib/pgo.sh"

# Parse options (options can appear anywhere)
VERBOSE=0
NO_CACHE=0
PGO_AUTO=0
PGO_VALIDATE=0
PGO_NO_RESUME=0
MIRI_FILTER=""
MIRI_NO_INSTALL=0
MIRI_FULL=0
MIRI_QUIET=0
COMMAND=""

ARGS=()
while [ $# -gt 0 ]; do
	case "$1" in
	--verbose | -v)
		VERBOSE=1
		export RUST_BACKTRACE=full
		shift
		;;
	--no-cache)
		NO_CACHE=1
		unset RUSTC_WRAPPER
		shift
		;;
	--auto)
		# Used by `pgo` subcommand: auto-detect best CPU target
		# (x86-64-v4 / x86-64-v3 / native) instead of defaulting
		# to -C target-cpu=native. Also exposed via the
		# `cargo use-pgo` alias.
		# shellcheck disable=SC2034 # read by build_pgo in lib/pgo.sh
		PGO_AUTO=1
		shift
		;;
	--validate)
		# Used by `pgo` subcommand: build a release baseline + run
		# benchmark on both, then print FPS delta. Doubles build
		# time but proves PGO actually helps.
		# shellcheck disable=SC2034 # read by build_pgo in lib/pgo.sh
		PGO_VALIDATE=1
		shift
		;;
	--no-resume)
		# Used by `pgo` subcommand: ignore stage stamps and rebuild
		# from scratch. Default is to resume from the last completed
		# stage if the commit hash matches.
		# shellcheck disable=SC2034 # read by build_pgo in lib/pgo.sh
		PGO_NO_RESUME=1
		shift
		;;
	--filter)
		# Used by `miri` subcommand: narrow test scope.
		# Substring match against full test path (e.g. "validation::")
		if [ $# -lt 2 ]; then
			log_error "--filter requires an argument"
			exit 1
		fi
		# shellcheck disable=SC2034 # read by run_miri in lib/miri.sh
		MIRI_FILTER="$2"
		shift 2
		;;
	--no-install)
		# Used by `miri` subcommand: don't auto-install nightly/miri.
		# shellcheck disable=SC2034 # read by run_miri in lib/miri.sh
		MIRI_NO_INSTALL=1
		shift
		;;
	--full)
		# Used by `miri` subcommand: run entire lib test suite
		# (slow, may fail on FFI tests). Skips the 6-module filter.
		# shellcheck disable=SC2034 # read by run_miri in lib/miri.sh
		MIRI_FULL=1
		shift
		;;
	--quiet-miri)
		# Suppress Miri status banner (for CI jobs that don't care).
		# shellcheck disable=SC2034 # read by show_miri_status in lib/miri.sh
		MIRI_QUIET=1
		shift
		;;
	--quiet | -q)
		# Suppress passing output in check-all, only show failures/warnings.
		# shellcheck disable=SC2034 # read by log_* in lib/common.sh and checks in lib/quality.sh
		QUIET_CHECK=1
		shift
		;;
	help | -h | --help)
		COMMAND="help"
		shift
		;;
	*)
		if [ -z "${COMMAND}" ]; then
			COMMAND="$1"
			shift
		else
			ARGS+=("$1")
			shift
		fi
		;;
	esac
done

if [ "${VERBOSE}" -eq 1 ]; then
	set -x
fi

# Main execution
main() {
	# Ensure we're in a Rust project
	if [ ! -f "Cargo.toml" ]; then
		log_error "Not in a Rust project directory (Cargo.toml not found)"
		exit 1
	fi

	local command="${COMMAND:-debug}"

	if [ ${#ARGS[@]} -ne 0 ]; then
		log_error "Unexpected extra arguments: ${ARGS[*]}"
		echo ""
		show_help
		exit 1
	fi

	# `help` is pure documentation — skip cache setup so it stays quiet.
	if [ "$command" = "help" ] || [ "$command" = "-h" ] || [ "$command" = "--help" ]; then
		show_help
		exit 0
	fi

	# `version-sync` is a pure verification — no build, no cache setup.
	# Run it before setup_build_cache so the output stays clean.
	if [ "$command" = "version-sync" ]; then
		run_version_sync
		exit $?
	fi

	# `miri` manages its own toolchain (nightly), so skip the stable
	# toolchain check and PGO cache setup. Miri has its own sysroot.
	if [ "$command" = "miri" ]; then
		run_miri
		exit $?
	fi

	# Setup environment for anything that actually builds or tests.
	if [ $NO_CACHE -eq 0 ]; then
		setup_build_cache
	fi

	# Show Miri verification status banner on every build/test command.
	# This gives the owner/user visibility into UB-freedom at every invocation.
	show_miri_status

	case "$command" in
	debug)
		check_rust_toolchain
		show_system_info
		build_debug
		;;
	release)
		check_rust_toolchain
		show_system_info
		build_release
		;;
	release-debug)
		check_rust_toolchain
		show_system_info
		build_release_with_debug
		;;
	test)
		check_rust_toolchain
		run_tests
		;;
	bench | benchmark)
		check_rust_toolchain
		run_benchmark
		;;
	verify-release)
		check_rust_toolchain
		verify_release_builds
		;;
	check)
		check_rust_toolchain
		run_quick_check
		;;
	check-all | --check-all)
		run_comprehensive_check
		;;
	pgo)
		check_rust_toolchain
		show_system_info
		build_pgo
		;;
	ci)
		run_comprehensive_check
		build_release
		;;
	fmt | format)
		run_fmt_fix
		;;
	clean)
		clean_build
		;;
	update)
		check_rust_toolchain
		update_dependencies
		;;
	all)
		check_rust_toolchain
		show_system_info
		run_fmt_check
		run_clippy
		build_debug
		build_release
		run_tests
		show_cache_stats
		;;
	stats)
		show_cache_stats
		;;
	help | -h | --help)
		show_help
		;;
	*)
		log_error "Unknown command: $command"
		echo ""
		show_help
		exit 1
		;;
	esac
}

# Execute with error handling
if main "$@"; then
	exit 0
else
	log_error "Build script failed"
	exit 1
fi
