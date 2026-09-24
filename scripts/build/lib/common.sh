# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/build/build.sh — not a standalone executable (no main, no
#   argument parsing; the entry script owns both).
#
# COSMOSTRIX BUILD MODULE: common
# shared foundation: output logging (ASCII symbol rule), colors,
# and the QUIET_CHECK gate, toolchain verification, build-cache detection
# (sccache/mold/lld/nextest), hardened RUSTFLAGS, system info, plus the
# shared readonly configuration (TARGET, MAX_JOBS, PROJECT_NAME).
#
# Part of the build.sh split (NIGHT-lts-2): the single-entry orchestrator
# was de-monolithed into sourced modules; function bodies are byte-identical
# moves from the former flat script.

# Colors for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly CYAN='\033[0;36m'
readonly NC='\033[0m'

# Configuration with intelligent defaults
# shellcheck disable=SC2034 # read by lib/builds.sh and lib/pgo.sh binary paths
readonly PROJECT_NAME="cosmostrix"

default_target() {
	if command -v rustc >/dev/null 2>&1; then
		local host
		host=$(rustc -vV 2>/dev/null | sed -n 's/^host: //p' || true)
		if [ -n "${host}" ]; then
			echo "${host}"
			return 0
		fi
	fi
	echo "x86_64-unknown-linux-gnu"
}

readonly TARGET="${COSMOSTRIX_TARGET:-$(default_target)}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

# Max-muscle job calculation: 100% of detected cores, min 1.
# NIGHT-boost-4 (owner mandate 2026-09-24): every build - local and CI -
# uses all cores of the machine it runs on (4 detected cores = 4 parallel
# jobs; no artificial ceiling). The former 75%-of-cores / max-8 throttle
# ("heat control") silently under-used CI runners and workstations alike;
# a machine that needs a thermal or load cap sets COSMOSTRIX_JOBS
# explicitly. Core detection honors the documented macOS path
# (sysctl -n hw.logicalcpu when nproc is absent), which the previous
# implementation claimed in the header but never executed.
calculate_jobs() {
	local cores
	cores=$(nproc 2>/dev/null || sysctl -n hw.logicalcpu 2>/dev/null || echo 4)
	local jobs=$((cores < 1 ? 1 : cores))
	echo "$jobs"
}

MAX_JOBS="${COSMOSTRIX_JOBS:-$(calculate_jobs)}"
export MAKEFLAGS="-j${MAX_JOBS}"
export CARGO_BUILD_JOBS="${MAX_JOBS}"

# Rust optimization flags
export CARGO_TERM_COLOR=always

# Quiet mode: suppress passing output, only show failures/warnings.
# Toggle with --quiet / -q flag.
QUIET_CHECK=0

# Functions
# NIGHT-enhanced-3: log_info / log_success now respect QUIET_CHECK
# directly so the section banners ("=== Comprehensive Code Quality
# Check ==="), per-step "OK" lines and Miri status banner are hidden
# in `check-all -q`. log_step already had the gate; log_warning /
# log_error never gate (they are the failure/warning surface the
# quiet mode is meant to surface).
log_info() {
	if [ ${QUIET_CHECK} -eq 0 ]; then
		echo -e "${BLUE}[INFO]${NC} $1"
	fi
}

# v80.0.0-beta.2 owner rule: diagnostic output uses ASCII symbols only
# (icon glyphs render as tofu/garbage on some OS/terminal combos):
# [OK] success, [!] warning, [X] error, [>] step, [INFO] info.
log_success() {
	if [ ${QUIET_CHECK} -eq 0 ]; then
		echo -e "${GREEN}[OK]${NC} $1"
	fi
}

log_warning() {
	echo -e "${YELLOW}[!]${NC} $1"
}

log_error() {
	echo -e "${RED}[X]${NC} $1" >&2
}

log_step() {
	if [ ${QUIET_CHECK} -eq 0 ]; then
		echo -e "${CYAN}[>]${NC} $1"
	fi
}

# log_success_quietable is retained for backward compatibility with
# the many existing call sites that already use it. After NIGHT-
# enhanced-3 it is functionally identical to log_success (both gate
# on QUIET_CHECK), but the explicit name documents intent at the
# call site: "this success message is safe to suppress in quiet
# mode" (as opposed to a hard-won one we always want to print).
log_success_quietable() {
	if [ ${QUIET_CHECK} -eq 0 ]; then
		log_success "$1"
	fi
}

check_rust_toolchain() {
	log_step "Checking Rust toolchain..."

	if ! command -v rustup &>/dev/null; then
		log_error "rustup not installed. Install from: https://rustup.rs"
		exit 1
	fi

	if ! command -v rustc &>/dev/null; then
		log_error "rustc not available in PATH. Install a Rust toolchain with rustup."
		exit 1
	fi

	if [ -z "${TARGET}" ]; then
		log_error "Could not determine Rust host target (TARGET is empty)."
		exit 1
	fi

	# Ensure target is installed
	if ! rustup target list --installed | grep -q "^${TARGET}$"; then
		log_info "Installing target: ${TARGET}"
		rustup target add "${TARGET}"
	fi

	log_success "Rust toolchain ready"
}

setup_build_cache() {
	# Detect available build accelerators and emit a single quiet summary
	# line. Missing tools (sccache/mold/lld/nextest) are silently skipped
	# — install them if you want faster builds; their absence is not an
	# error condition worth a warning per tool.
	local bits=()

	if command -v sccache &>/dev/null; then
		# sccache and incremental compilation conflict; sccache wins.
		export CARGO_INCREMENTAL=0
		export RUSTC_WRAPPER=sccache
		sccache --start-server 2>/dev/null || true
		bits+=("sccache")
	else
		export CARGO_INCREMENTAL=1
	fi

	if command -v mold &>/dev/null; then
		export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=mold"
		bits+=("mold")
	elif command -v lld &>/dev/null; then
		export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=lld"
		bits+=("lld")
	fi

	if command -v cargo-nextest &>/dev/null; then
		NEXTEST_AVAILABLE=1
		bits+=("nextest")
	else
		# shellcheck disable=SC2034 # read by run_tests in lib/quality.sh
		NEXTEST_AVAILABLE=0
	fi

	if [ ${#bits[@]} -gt 0 ]; then
		log_info "Build cache: $(
			IFS=', '
			echo "${bits[*]}"
		)"
	else
		log_info "Build cache: none (install sccache + mold/lld for faster rebuilds)"
	fi
}

# Append hardened RUSTFLAGS for release/pro/pgo builds.
#
# These flags remove local filesystem paths from the produced binary
# (so release artifacts do not leak the builder's cargo registry path
# or project working directory) and preserve frame pointers for
# post-mortem debugging without sacrificing runtime performance.
#
# Idempotent: skips flags already present in $RUSTFLAGS so it is safe
# to call after the PGO stage sets its own -C target-cpu / profile-use
# flags.
#
# To opt out (e.g. local dev iteration with full path info), set
# COSMOSTRIX_NO_HARDEN=1 in the environment.
apply_hardened_rustflags() {
	if [ "${COSMOSTRIX_NO_HARDEN:-0}" = "1" ]; then
		return 0
	fi

	local cargo_home="${HOME}/.cargo"
	local pwd_path="${PWD}"

	# --remap-path-prefix is order-sensitive: first matching prefix
	# wins, so remap the deeper cargo registry path before the
	# project working directory.
	#
	# Each flag MUST be a single array element. The idempotency check
	# below does substring matching against existing RUSTFLAGS, so
	# splitting `-C` and its arg into 2 elements would cause `-C` to
	# match ANY existing `-C <something>` flag (e.g. `-C target-cpu=...`
	# or `-C profile-use=...` set by the PGO stage), skipping the `-C`
	# prefix while still appending the bare arg. The result is a rustc
	# invocation like `... -C profile-use=... force-frame-pointers=yes`
	# which rustc rejects as "multiple input filenames provided".
	# Combining `-C` + arg into one element (`-Cforce-frame-pointers=yes`)
	# makes the idempotency check match the full flag string.
	local extra=(
		"--remap-path-prefix=${cargo_home}=redacted"
		"--remap-path-prefix=${pwd_path}=redacted"
		"-Cforce-frame-pointers=yes"
	)

	local existing="${RUSTFLAGS:-}"
	local merged=""
	local flag
	for flag in "${extra[@]}"; do
		# Skip if the flag is already present (idempotent).
		if [[ "${existing}" == *"${flag}"* ]]; then
			continue
		fi
		if [ -n "${merged}" ]; then
			merged+=" "
		fi
		merged+="${flag}"
	done

	if [ -z "${merged}" ]; then
		return 0
	fi

	if [ -n "${existing}" ]; then
		export RUSTFLAGS="${existing} ${merged}"
	else
		export RUSTFLAGS="${merged}"
	fi
	log_info "Hardened RUSTFLAGS applied: ${merged}"
}

show_system_info() {
	# One-line summary — full `cargo --version` etc. is already on the
	# stdout/stderr of the actual build command that follows.
	log_info "Target: ${TARGET} | Jobs: ${MAX_JOBS} | $(rustc --version)"
}
