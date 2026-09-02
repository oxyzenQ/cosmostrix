#!/usr/bin/env bash
#
# COSMOSTRIX BUILD AUTOMATION SCRIPT
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
# Optimized build script with intelligent core detection and advanced caching.
# See `./scripts/build.sh help` for the command list.

set -euo pipefail

# Colors for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly CYAN='\033[0;36m'
readonly NC='\033[0m'

# Configuration with intelligent defaults
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

# Intelligent job calculation: 75% of cores, min 1, max 8 for heat control
calculate_jobs() {
	local cores
	cores=$(nproc 2>/dev/null || echo 4)
	local jobs=$((cores * 3 / 4))
	jobs=$((jobs < 1 ? 1 : jobs))
	jobs=$((jobs > 8 ? 8 : jobs))
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
log_info() {
	echo -e "${BLUE}[INFO]${NC} $1"
}

# v80.0.0-beta.2 owner rule: diagnostic output uses ASCII symbols only
# (icon glyphs render as tofu/garbage on some OS/terminal combos):
# [OK] success, [!] warning, [X] error, [>] step, [INFO] info.
log_success() {
	echo -e "${GREEN}[OK]${NC} $1"
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

log_success_quietable() {
	# In quiet mode, skip the success message entirely.
	# In normal mode, behave like log_success.
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

update_dependencies() {
	log_step "Updating dependencies..."

	if ! cargo update --quiet; then
		log_error "Failed to update dependencies"
		return 1
	fi

	# Security audit
	if command -v cargo-audit &>/dev/null; then
		if cargo audit --quiet 2>/dev/null; then
			log_success "Security audit passed"
		else
			log_warning "Security vulnerabilities detected (run 'cargo audit' for details)"
		fi
	else
		log_warning "cargo-audit not installed. Install: cargo install cargo-audit --locked"
	fi

	log_success "Dependencies updated"
}

build_debug() {
	log_step "Building debug binary..."

	if cargo build --profile dev --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		local binary="target/${TARGET}/debug/${PROJECT_NAME}"
		local size
		size=$(du -h "$binary" 2>/dev/null | cut -f1 || echo "unknown")
		log_success "Debug build complete (${size})"
		echo "  └─ Binary: ${binary}"
	else
		log_error "Debug build failed"
		return 1
	fi
}

build_release() {
	log_step "Building optimized release binary..."

	apply_hardened_rustflags
	if cargo build --profile release --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		local binary="target/${TARGET}/release/${PROJECT_NAME}"
		local size
		size=$(du -h "$binary" 2>/dev/null | cut -f1 || echo "unknown")
		log_success "Release build complete (${size})"
		echo "  └─ Binary: ${binary}"
	else
		log_error "Release build failed"
		return 1
	fi
}

build_release_with_debug() {
	log_step "Building release with debug symbols..."

	if cargo build --profile release-with-debug --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		local binary="target/${TARGET}/release-with-debug/${PROJECT_NAME}"
		local size
		size=$(du -h "$binary" 2>/dev/null | cut -f1 || echo "unknown")
		log_success "Release-debug build complete (${size})"
		echo "  └─ Binary: ${binary}"
	else
		log_error "Release-debug build failed"
		return 1
	fi
}

run_tests() {
	log_step "Running test suite..."

	local test_output
	if [ "${NEXTEST_AVAILABLE:-0}" -eq 1 ]; then
		if [ ${QUIET_CHECK} -eq 1 ]; then
			test_output=$(cargo nextest run --target "${TARGET}" --jobs "${MAX_JOBS}" 2>&1)
			local rc=$?
			# In quiet mode, show only failures
			echo "$test_output" | grep -E '(FAILED|failures:|error\[)' || true
			if [ $rc -eq 0 ]; then
				log_success_quietable "All tests passed (nextest)"
			else
				log_error "Tests failed"
			fi
			return $rc
		else
			if cargo nextest run --target "${TARGET}" --jobs "${MAX_JOBS}"; then
				log_success "All tests passed (nextest)"
				return 0
			else
				log_error "Tests failed"
				return 1
			fi
		fi
	else
		if [ ${QUIET_CHECK} -eq 1 ]; then
			test_output=$(cargo test --target "${TARGET}" --jobs "${MAX_JOBS}" -- --test-threads="${MAX_JOBS}" 2>&1)
			local rc=$?
			# In quiet mode, show only failures + summary line
			echo "$test_output" | grep -E '(FAILED|failures:|test result:)' || true
			if [ $rc -eq 0 ]; then
				log_success_quietable "All tests passed"
			else
				log_error "Tests failed"
			fi
			return $rc
		else
			if cargo test --target "${TARGET}" --jobs "${MAX_JOBS}" -- --test-threads="${MAX_JOBS}"; then
				log_success "All tests passed"
				return 0
			else
				log_error "Tests failed"
				return 1
			fi
		fi
	fi
}

run_clippy() {
	log_step "Running Clippy linter..."

	if [ ${QUIET_CHECK} -eq 1 ]; then
		# Quiet: show only errors/warnings (cargo clippy outputs to stderr)
		local clip_output
		clip_output=$(cargo clippy --target "${TARGET}" --all-targets --all-features -- -D warnings 2>&1)
		local clip_rc=$?
		echo "$clip_output" | grep -E '^(error|warning)' || true
		if [ $clip_rc -eq 0 ]; then
			log_success_quietable "Clippy checks passed"
		else
			log_error "Clippy found issues"
		fi
		return $clip_rc
	else
		if cargo clippy --target "${TARGET}" --all-targets --all-features -- -D warnings; then
			log_success "Clippy checks passed"
			return 0
		else
			log_error "Clippy found issues"
			return 1
		fi
	fi
}

run_fmt_check() {
	log_step "Checking code formatting..."

	if cargo fmt --all -- --check 2>&1; then
		log_success_quietable "Code formatting is correct"
		return 0
	else
		log_error "Formatting issues found. Run: cargo fmt --all"
		return 1
	fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Cross-platform type check (v52 guard)
# ─────────────────────────────────────────────────────────────────────────────
# The dev host is Linux, but CI additionally builds Windows, FreeBSD,
# macOS and Android. Platform-gated code (#[cfg(target_os = ...)]) can
# type-check cleanly on the host and break every other build — e.g. a
# cfg-gated `use` line whose attribute silently re-attaches to the NEXT
# import when the use line is deleted (f19470a6 lesson: a dangling
# cfg left the PowerManager import Linux-only; Windows/FreeBSD/macOS/
# Android CI all went red while local gates stayed green).
# This check runs `cargo check` for every CI-built non-host target.
# Targets are installed on demand (rust-std only, seconds each); after
# the first run everything is cached (~1 s per target). When a target
# cannot be installed (offline sandbox) the check skips with a warning
# — the real CI matrix remains the final gate in that case.
CROSS_CHECK_TARGETS=(
	"x86_64-pc-windows-gnu"
	"x86_64-unknown-freebsd"
	"aarch64-apple-darwin"
	"aarch64-linux-android"
)

run_cross_platform_check() {
	log_step "Cross-platform type check (${#CROSS_CHECK_TARGETS[@]} CI targets)..."
	local t
	for t in "${CROSS_CHECK_TARGETS[@]}"; do
		if ! rustup target list --installed 2>/dev/null | grep -qx "${t}"; then
			rustup target add "${t}" &>/dev/null || {
				log_warn "  ${t}: not installed and rustup add failed — skipping"
				continue
			}
		fi
		if cargo check --quiet --target "${t}" 2>/dev/null; then
			log_success_quietable "  ${t}: OK"
		else
			log_error "  ${t}: FAILED — reproduce with: cargo check --target ${t}"
			return 1
		fi
	done
	return 0
}

run_fmt_fix() {
	log_step "Formatting code..."
	cargo fmt --all
	log_success "Code formatted"
}

run_audit() {
	log_step "Running security audit..."

	if ! command -v cargo-audit &>/dev/null; then
		log_warning "cargo-audit not installed (skipping). Install: cargo install cargo-audit --locked"
		return 0
	fi

	if [ ${QUIET_CHECK} -eq 1 ]; then
		local audit_output
		audit_output=$(cargo audit 2>&1)
		local rc=$?
		echo "$audit_output" | grep -iE '(vulnerabilit[yi]|CVE-|warning|error)' || true # codespell:ignore
		if [ $rc -eq 0 ]; then
			log_success_quietable "Security audit passed"
		else
			log_warning "Security issues detected"
		fi
		return $rc
	else
		if cargo audit; then
			log_success "Security audit passed"
		else
			log_warning "Security issues detected"
			return 1
		fi
	fi
}

run_loc_check() {
	log_step "Checking Rust source file sizes..."

	if [ ! -x "scripts/check-rs-loc.sh" ]; then
		log_warning "scripts/check-rs-loc.sh not found or not executable (skipping)"
		return 0
	fi

	local loc_output
	if [ ${QUIET_CHECK} -eq 1 ]; then
		loc_output=$(bash scripts/check-rs-loc.sh 2>&1)
		local rc=$?
		echo "$loc_output" | grep -E '(FAIL|ERROR|over|exceeds)' || true
		if [ $rc -eq 0 ]; then
			log_success_quietable "LOC check passed"
		else
			log_error "LOC check failed"
		fi
		return $rc
	else
		if bash scripts/check-rs-loc.sh; then
			log_success "LOC check passed"
		else
			log_error "LOC check failed"
			return 1
		fi
	fi
}

run_header_check() {
	log_step "Checking SPDX license headers..."

	if [ ! -f "scripts/check-headers.sh" ]; then
		log_error "scripts/check-headers.sh not found"
		return 1
	fi

	local hdr_output
	if [ ${QUIET_CHECK} -eq 1 ]; then
		hdr_output=$(bash scripts/check-headers.sh 2>&1)
		local rc=$?
		echo "$hdr_output" | grep -iE '(missing|FAIL|ERROR)' || true
		if [ $rc -eq 0 ]; then
			log_success_quietable "Header check passed"
		else
			log_error "Header check failed"
		fi
		return $rc
	else
		if bash scripts/check-headers.sh; then
			log_success "Header check passed"
		else
			log_error "Header check failed"
			return 1
		fi
	fi
}

run_symbol_only_output_check() {
	log_step "Checking for icon glyphs in output surfaces (symbol-only rule)..."

	if [ ! -f "scripts/check-symbol-only-output.sh" ]; then
		log_error "scripts/check-symbol-only-output.sh not found"
		return 1
	fi

	if [ ${QUIET_CHECK} -eq 1 ]; then
		local soo_output
		soo_output=$(bash scripts/check-symbol-only-output.sh 2>&1)
		local soo_rc=$?
		echo "$soo_output" | grep -iE '(FAIL|VIOLATION)' || true
		if [ $soo_rc -eq 0 ]; then
			log_success_quietable "Symbol-only output check passed"
		else
			log_error "Symbol-only output check failed (icon glyphs in output - see docs/RULES.md Output Glyph Policy)"
			return 1
		fi
	else
		if bash scripts/check-symbol-only-output.sh; then
			log_success "Symbol-only output check passed"
		else
			log_error "Symbol-only output check failed (icon glyphs in output - see docs/RULES.md Output Glyph Policy)"
			return 1
		fi
	fi
}

run_version_anti_pattern_check() {
	log_step "Checking for hardcoded version-string anti-patterns..."

	if [ ! -f "scripts/check-version-anti-patterns.sh" ]; then
		log_error "scripts/check-version-anti-patterns.sh not found"
		return 1
	fi

	local vap_output
	if [ ${QUIET_CHECK} -eq 1 ]; then
		vap_output=$(bash scripts/check-version-anti-patterns.sh 2>&1)
		local rc=$?
		echo "$vap_output" | grep -iE '(FAIL|ERROR|found|anti-pattern)' || true
		if [ $rc -eq 0 ]; then
			log_success_quietable "Version anti-pattern check passed"
		else
			log_error "Version anti-pattern check failed (use env!(\"CARGO_PKG_VERSION\") instead)"
			return 1
		fi
	else
		if bash scripts/check-version-anti-patterns.sh; then
			log_success "Version anti-pattern check passed"
		else
			log_error "Version anti-pattern check failed (use env!(\"CARGO_PKG_VERSION\") instead)"
			return 1
		fi
	fi

	log_step "Checking Rust version sync across all sources..."

	if [ ! -f "scripts/check-rust-version-sync.sh" ]; then
		log_error "scripts/check-rust-version-sync.sh not found"
		return 1
	fi

	if [ ${QUIET_CHECK} -eq 1 ]; then
		local vs_output
		vs_output=$(bash scripts/check-rust-version-sync.sh 2>&1)
		local vs_rc=$?
		echo "$vs_output" | grep -iE '(FAIL|ERROR|mismatch|desync|differ)' || true
		if [ $vs_rc -eq 0 ]; then
			log_success_quietable "Rust version sync check passed"
		else
			log_error "Rust version sync check failed — see output above for mismatched sources"
			return 1
		fi
	else
		if bash scripts/check-rust-version-sync.sh; then
			log_success "Rust version sync check passed"
		else
			log_error "Rust version sync check failed — see output above for mismatched sources"
			return 1
		fi
	fi
}

run_shellcheck() {
	log_step "Running shellcheck on scripts/*.sh..."

	if ! command -v shellcheck >/dev/null 2>&1; then
		log_warning "shellcheck not installed (skipping). Install: apt install shellcheck or brew install shellcheck"
		return 0
	fi

	if [ ${QUIET_CHECK} -eq 1 ]; then
		local sh_output
		sh_output=$(shellcheck scripts/*.sh 2>&1)
		local rc=$?
		# Only show lines with actual findings
		echo "$sh_output" | grep -E '(^In |^scripts/|SC[0-9])' || true
		if [ $rc -eq 0 ]; then
			log_success_quietable "Shellcheck passed"
		else
			log_error "Shellcheck failed — fix warnings before committing"
		fi
		return $rc
	else
		if shellcheck scripts/*.sh; then
			log_success "Shellcheck passed"
		else
			log_error "Shellcheck failed — fix warnings before committing"
			return 1
		fi
	fi
}

run_python_lint() {
	log_step "Running ruff check + format on scripts/*.py..."

	if ! command -v ruff >/dev/null 2>&1; then
		log_warning "ruff not installed (skipping Python lint). Install: pip install ruff"
		return 0
	fi

	local py_failed=0
	local py_output
	if [ ${QUIET_CHECK} -eq 1 ]; then
		if ! py_output=$(ruff check scripts/*.py 2>&1); then
			echo "$py_output"
			log_error "ruff check failed — fix Python lint issues"
			((py_failed++))
		fi
		if ! py_output=$(ruff format --check scripts/*.py 2>&1); then
			echo "$py_output"
			log_error "ruff format check failed — run 'ruff format scripts/*.py' to fix"
			((py_failed++))
		fi
	else
		if ! ruff check scripts/*.py; then
			log_error "ruff check failed — fix Python lint issues"
			((py_failed++))
		fi

		if ! ruff format --check scripts/*.py; then
			log_error "ruff format check failed — run 'ruff format scripts/*.py' to fix"
			((py_failed++))
		fi
	fi

	if [ $py_failed -eq 0 ]; then
		log_success_quietable "Python lint + format passed"
		return 0
	else
		return 1
	fi
}

run_comprehensive_check() {
	local failed=0

	echo ""
	log_info "=== Comprehensive Code Quality Check ==="
	echo ""

	check_rust_toolchain || ((failed++))
	run_fmt_check || ((failed++))
	run_loc_check || ((failed++))
	run_header_check || ((failed++))
	run_version_anti_pattern_check || ((failed++))
	run_symbol_only_output_check || ((failed++))
	run_shellcheck || ((failed++))
	run_python_lint || ((failed++))
	run_version_sync || ((failed++))
	run_clippy || ((failed++))
	run_cross_platform_check || ((failed++))
	run_tests || ((failed++))
	run_audit || ((failed++))

	echo ""
	if [ $failed -eq 0 ]; then
		log_success "All quality checks passed!"
		return 0
	else
		log_error "$failed check(s) failed"
		return 1
	fi
}

run_quick_check() {
	log_step "Running quick checks..."

	run_fmt_check && run_clippy
}

clean_build() {
	log_step "Cleaning build artifacts..."

	cargo clean

	if command -v sccache &>/dev/null; then
		sccache --zero-stats 2>/dev/null || true
	fi

	log_success "Build artifacts cleaned"
}

show_cache_stats() {
	if command -v sccache &>/dev/null; then
		echo ""
		log_info "=== Build Cache Statistics ==="
		sccache --show-stats
	else
		log_warning "sccache not available"
	fi
}

run_benchmark() {
	log_step "Running benchmarks..."

	if [ -x "benchmark/benchmark.sh" ]; then
		if bash benchmark/benchmark.sh; then
			log_success "Benchmarks complete"
		else
			log_error "Benchmarks failed"
			return 1
		fi
	else
		log_error "benchmark/benchmark.sh not found"
		return 1
	fi
}

verify_release_builds() {
	log_step "Verifying Linux x86_64 release builds..."

	if scripts/verify-release-build.sh; then
		log_success "Release build verification complete"
	else
		log_error "Release build verification failed"
		return 1
	fi
}

show_help() {
	cat <<'EOF'
cosmostrix build script

USAGE:
    ./scripts/build.sh [COMMAND] [OPTIONS]

Version bumping is handled by ./scripts/version-to.sh — see its --help for
details. The recommended one-shot bump+build flow is:

    ./scripts/version-to.sh vX.Y.Z && ./scripts/build.sh release

COMMANDS (essentials):
    debug           Build debug version (default)
    release         Build optimized release version
    pgo             PGO nitro build (instrument → benchmark → optimize, +5-15% FPS)
                    Pass --auto to auto-detect the best CPU target for this host.
                    Pass --validate to also build a release baseline + compare FPS.
                    Pass --no-resume to force a full rebuild (ignore stage stamps).
                    Shortcut: cargo use-pgo
                    Crash recovery: stage stamps in target/pgo-stages/ allow
                    resuming from the last completed stage if the build is
                    interrupted. Logs in target/pgo-logs/ (rotated, last 5 kept).
    miri            Run Miri UB verification on audited pure-Rust modules
                    (~3-10 min, requires nightly — auto-installed if missing).
                    Updates target/miri-stamp so every subsequent build shows
                    a Miri status banner (VERIFIED / STALE / FAILED / never-run).
                    Pass --filter <pat> for narrow runs (no stamp update).
                    Pass --full to run entire lib test suite (slow, may fail on FFI).
                    Pass --no-install to skip auto-install of nightly/miri.
    test            Run test suite
    check           Quick checks (fmt + clippy)
    check-all       Comprehensive checks (fmt + clippy + test + audit + headers + LOC
                    + version anti-pattern guard + version-sync)
    version-sync    Verify all active version refs agree with Cargo.toml
                    (no build — fails fast on desync; same as
                    ./scripts/version-to.sh --check <cargo-toml-version>)
    fmt             Format code
    clean           Clean build artifacts
    help            Show this help

COMMANDS (secondary):
    release-debug   Build release with debug symbols
    verify-release  Build and verify Linux x86_64 release variants (v1/v2/v3/v4)
    bench           Run benchmarks via benchmark/benchmark.sh
    update          Update dependencies and audit
    all             Full pipeline (check + debug + release + test)
    ci              CI pipeline (check-all + release)
    stats           Show build cache statistics

OPTIONS:
    --no-cache      Disable build caching
    --verbose       Enable verbose output (set -x)
    --auto          Auto-detect best CPU target for PGO (v4/v3/native)
    --validate      Build release baseline + compare FPS delta (PGO validation)
    --no-resume     Force full PGO rebuild (ignore stage stamps)
    --filter <pat>  Narrow Miri test scope (substring match, e.g. 'validation::')
    --no-install    Don't auto-install nightly/miri (fail if missing)
    --full          Run full lib test suite under Miri (slow, may fail on FFI)
    --quiet-miri    Suppress Miri status banner (for CI jobs that don't care)
    --quiet, -q     Suppress passing output in check-all, only show failures/warnings

ENVIRONMENT:
    COSMOSTRIX_JOBS             Override CPU core limit (default: 75% of cores, max 8)
    COSMOSTRIX_TARGET           Override build target (default: rustc host target)
    COSMOSTRIX_TARGET_CPU       Override -C target-cpu for the FINAL PGO binary
                                (default: native, or auto-detected when --auto is passed).
                                This binary ships to users — can target v4 even on
                                a v3-only build host.
    COSMOSTRIX_INSTRUMENT_CPU   Override -C target-cpu for the INSTRUMENTED PGO binary
                                (default: x86-64-v3 on x86_64, native elsewhere).
                                This binary must RUN on the build host to collect
                                profile data — keep it conservative. Override only
                                if you know the host supports a higher target.
    RUST_BACKTRACE              Control backtrace verbosity (default: 1)

EXAMPLES:
    ./scripts/build.sh release                  # optimized release build
    ./scripts/build.sh check-all                # all quality gates
    ./scripts/build.sh pgo --auto               # PGO with auto CPU detection
    cargo use-pgo                               # same as above
    COSMOSTRIX_JOBS=4 ./scripts/build.sh all    # full pipeline, 4 cores
    ./scripts/build.sh version-sync             # verify all version refs in sync

    # Bump version + build in one flow (version-to.sh handles the bump):
    ./scripts/version-to.sh vX.Y.Z && ./scripts/build.sh release
    ./scripts/version-to.sh vX.Y.Z && ./scripts/build.sh pgo --auto

OPTIONAL TOOLS (auto-detected, silently skipped if absent):
    sccache   - Build caching       (cargo install sccache)
    mold/lld  - Fast linker         (system package manager)
    nextest   - Fast test runner    (cargo install cargo-nextest)
    audit     - Security auditing   (cargo install cargo-audit)

MIRI VERIFICATION:
    Miri (https://github.com/rust-lang/miri) detects undefined behavior in
    unsafe Rust. It runs under the nightly toolchain, which is auto-installed
    on first `./scripts/build.sh miri` invocation. The full test suite under
    Miri is slow and many tests touch TTY/FFI that Miri cannot execute, so
    the default scope is the 6 pure-Rust modules audited in
    docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md:
      - config_hints::tests  (41 tests)
      - validation::tests    (18 tests)
      - color_cache::tests   (12 tests)
      - safepath::tests      (22 tests)
      - humanize::tests      (9 tests)
      - bolt::tests          (5 tests)

    Verification status is cached in target/miri-stamp (key=value). A status
    banner is shown at the start of every build.sh invocation:
      [OK] Miri VERIFIED at <commit> · <timestamp> · N tests (0 fail) · Xs
      [!] Miri STALE — verified at <old>, HEAD is <new>. Re-run to refresh.
      [X] Miri FAILED at <commit>. See target/miri-log.txt for details.
      [INFO] Miri: never run on this workspace.

    The stamp is invalidated automatically when HEAD changes (status flips
    to STALE). To force a re-verify: ./scripts/build.sh miri

EOF
}

# ── Version sync (verification only) ───────────────────────────────────
# Version bumping is owned by ./scripts/version-to.sh — see its --help for
# the full list of files it touches (Cargo.toml, Cargo.lock, PKGBUILD,
# .SRCINFO, README.md, docs/workflow/ABOUT_CI.md). build.sh only exposes
# the `version-sync` subcommand, which verifies all active version refs
# agree with Cargo.toml without writing anything.

# Read the current package version from Cargo.toml (single source of truth).
# Used by `version-sync` to know what to verify against.
read_cargo_version() {
	local cargo_toml="${PWD}/Cargo.toml"
	if [ ! -f "${cargo_toml}" ]; then
		log_error "Cargo.toml not found at ${cargo_toml}"
		return 1
	fi
	local ver
	ver="$(grep -E '^version = "' "${cargo_toml}" | head -1 | sed -E 's/^version = "(.+)"/\1/')"
	if [ -z "${ver}" ]; then
		log_error "Could not extract version from Cargo.toml"
		return 1
	fi
	echo "${ver}"
}

# Verify all active version refs agree with Cargo.toml. No build, no writes.
# Wraps `version-to.sh --check <cargo-version>` for convenience.
run_version_sync() {
	log_step "Verifying version sync across all active files..."

	local current
	current="$(read_cargo_version)" || return 1

	local bumper="${PWD}/scripts/version-to.sh"
	if [ ! -x "${bumper}" ]; then
		log_error "Version bumper not found or not executable: ${bumper}"
		return 1
	fi

	if [ ${QUIET_CHECK} -eq 1 ]; then
		local vsync_output
		vsync_output=$("${bumper}" --check "${current}" 2>&1)
		local vsync_rc=$?
		echo "$vsync_output" | grep -iE '(FAIL|ERROR|mismatch|desync|differ|desync)' || true
		if [ $vsync_rc -eq 0 ]; then
			log_success_quietable "All active version refs agree with Cargo.toml (v${current})"
		else
			log_error "Version desync detected — run './scripts/version-to.sh v${current}' to fix"
		fi
		return $vsync_rc
	else
		if "${bumper}" --check "${current}"; then
			log_success "All active version refs agree with Cargo.toml (v${current})"
		else
			log_error "Version desync detected — run './scripts/version-to.sh v${current}' to fix"
			return 1
		fi
	fi
}

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
		PGO_AUTO=1
		shift
		;;
	--validate)
		# Used by `pgo` subcommand: build a release baseline + run
		# benchmark on both, then print FPS delta. Doubles build
		# time but proves PGO actually helps.
		PGO_VALIDATE=1
		shift
		;;
	--no-resume)
		# Used by `pgo` subcommand: ignore stage stamps and rebuild
		# from scratch. Default is to resume from the last completed
		# stage if the commit hash matches.
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
		MIRI_FILTER="$2"
		shift 2
		;;
	--no-install)
		# Used by `miri` subcommand: don't auto-install nightly/miri.
		MIRI_NO_INSTALL=1
		shift
		;;
	--full)
		# Used by `miri` subcommand: run entire lib test suite
		# (slow, may fail on FFI tests). Skips the 6-module filter.
		MIRI_FULL=1
		shift
		;;
	--quiet-miri)
		# Suppress Miri status banner (for CI jobs that don't care).
		MIRI_QUIET=1
		shift
		;;
	--quiet | -q)
		# Suppress passing output in check-all, only show failures/warnings.
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

# ── Miri (UB detector) integration ────────────────────────────────────
# Miri runs under nightly and verifies unsafe code is sound. The full
# test suite under Miri takes 30+ min and many tests touch TTY/FFI which
# Miri cannot run. We restrict to the 6 pure-Rust modules already audited
# in docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md:
#   - config_hints::tests  (41 tests)
#   - validation::tests    (18 tests)
#   - color_cache::tests   (12 tests)
#   - safepath::tests      (22 tests)
#   - humanize::tests      (9 tests)
#   - bolt::tests          (5 tests)
#
# Verification status is cached in target/miri-stamp (key=value format)
# so we don't re-run on every build. A status banner is shown at the
# start of every build.sh invocation reflecting the stamp state:
#   - VERIFIED  — stamp commit == HEAD
#   - STALE     — stamp commit != HEAD (suggests re-run)
#   - FAILED    — last run had violations
#   - NEVER RUN — no stamp yet

readonly MIRI_STAMP_FILE="target/miri-stamp"
readonly MIRI_LOG_FILE="target/miri-log.txt"
readonly MIRI_AUDIT_MODULES=(
	"config_hints::"
	"validation::"
	"color_cache::"
	"safepath::"
	"humanize::"
	"bolt::"
)

# Print one-line Miri status banner. Called from main() before dispatch.
# Quiet when --quiet-miri is passed (CI jobs that don't care).
show_miri_status() {
	[ "${MIRI_QUIET:-0}" = "1" ] && return 0

	local head head_short
	head=$(git rev-parse HEAD 2>/dev/null || echo "")
	head_short=$(git rev-parse --short HEAD 2>/dev/null || echo "")

	if [ ! -f "${MIRI_STAMP_FILE}" ]; then
		log_info "Miri: never run on this workspace. Run './scripts/build.sh miri' to verify (needs nightly, ~3-10 min)."
		return 0
	fi

	# Parse stamp (key=value format, no jq dependency)
	local stamp_commit stamp_short stamp_ts_iso stamp_status stamp_dur stamp_run stamp_fail
	stamp_commit=$(grep '^commit=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_short=$(grep '^commit_short=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_ts_iso=$(grep '^timestamp_iso=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_status=$(grep '^status=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_dur=$(grep '^duration_ms=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_run=$(grep '^tests_run=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_fail=$(grep '^tests_failed=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)

	local dur_sec=""
	if [ -n "${stamp_dur}" ]; then
		dur_sec=$(awk -v ms="${stamp_dur}" 'BEGIN { printf "%.1f", ms/1000 }')
	fi

	case "${stamp_status}" in
	verified)
		if [ "${stamp_commit}" = "${head}" ]; then
			log_success "Miri VERIFIED at ${stamp_short} · ${stamp_ts_iso} · ${stamp_run} tests (${stamp_fail} fail) · ${dur_sec}s"
		else
			log_warning "Miri STALE — verified at ${stamp_short}, HEAD is ${head_short}. Run './scripts/build.sh miri' to refresh."
		fi
		;;
	failed)
		log_error "Miri FAILED at ${stamp_short} · ${stamp_ts_iso} · ${stamp_fail} violations. See ${MIRI_LOG_FILE}."
		;;
	skipped:*)
		local reason="${stamp_status#skipped:}"
		log_info "Miri SKIPPED (${reason}) at ${stamp_short}. Run './scripts/build.sh miri' to verify."
		;;
	*)
		log_info "Miri: unknown status '${stamp_status}' at ${stamp_short}."
		;;
	esac
}

# Run Miri verification on the pure-Rust modules. Updates stamp file.
# Args (parsed globally before main):
#   --filter <pat>  Narrow test scope (substring match, e.g. "validation::")
#   --no-install    Don't auto-install nightly/miri (fail if missing)
#   --full          Run entire lib test suite (slow, may fail on FFI tests)
run_miri() {
	local filter="${MIRI_FILTER:-}"
	local full="${MIRI_FULL:-0}"

	# 1. rustup is required (needed for nightly toolchain).
	if ! command -v rustup &>/dev/null; then
		log_error "rustup not installed. Miri requires nightly. Install from https://rustup.rs"
		exit 1
	fi

	# 2. Ensure nightly toolchain is installed.
	if ! rustup toolchain list 2>/dev/null | grep -q '^nightly-'; then
		if [ "${MIRI_NO_INSTALL}" = "1" ]; then
			log_error "nightly toolchain not installed and --no-install given. Aborting."
			exit 1
		fi
		log_step "Installing nightly toolchain (rustup toolchain install nightly --profile minimal --component miri)..."
		rustup toolchain install nightly --profile minimal --component miri || {
			log_error "Failed to install nightly + miri. Try manually: rustup toolchain install nightly --component miri"
			exit 1
		}
	else
		# 3. Ensure miri component is installed on nightly.
		# Modern rustup emits per-target lines like
		# `miri-x86_64-unknown-linux-gnu (installed)`, so the
		# separator after `miri` can be either a hyphen (newer
		# rustup) or a space (older rustup). Match both.
		if ! rustup component list --toolchain nightly 2>/dev/null | grep -q '^miri[-[:space:]].*installed'; then
			if [ "${MIRI_NO_INSTALL}" = "1" ]; then
				log_error "miri component not installed on nightly and --no-install given. Aborting."
				exit 1
			fi
			log_step "Installing miri component on nightly..."
			rustup component add miri --toolchain nightly || {
				log_error "Failed to install miri component."
				exit 1
			}
		fi
	fi

	local nightly_ver miri_ver
	nightly_ver=$(rustc +nightly --version 2>/dev/null | head -1)
	miri_ver=$(cargo +nightly miri --version 2>/dev/null | head -1)
	log_info "Miri: ${miri_ver}"
	log_info "Nightly: ${nightly_ver}"

	# 4. One-time libstd setup (non-interactive) to avoid prompt during test run.
	log_step "Ensuring Miri sysroot is set up (cargo +nightly miri setup)..."
	cargo +nightly miri setup 2>&1 | tail -3 || true

	# 5. Build the test filter list.
	local filter_args=()
	if [ -n "${filter}" ]; then
		filter_args=("${filter}")
		log_step "Running Miri with filter: ${filter}"
	elif [ "${full}" = "1" ]; then
		# No filter — run entire lib test suite.
		log_step "Running Miri on FULL lib test suite (this is slow, may fail on FFI tests)..."
	else
		# Default: 6 known-good pure-Rust modules.
		read -r -a filter_args <<<"${MIRI_AUDIT_MODULES[*]}"
		log_step "Running Miri on ${#filter_args[@]} audited pure-Rust modules: ${filter_args[*]}"
	fi

	# 6. Run Miri. MIRIFLAGS disables isolation so tests that need
	#    env vars / time / file paths don't fail spuriously.
	#    Note: cosmostrix is a binary crate (no src/lib.rs), so we
	#    don't pass --lib. The default test target covers all
	#    unittests embedded in src/*.rs modules.
	local start_ms end_ms duration_ms
	start_ms=$(date +%s%3N 2>/dev/null || date +%s)

	mkdir -p target
	local miri_exit=0
	MIRIFLAGS="${MIRIFLAGS:--Zmiri-disable-isolation}" \
		cargo +nightly miri test -- "${filter_args[@]}" \
		2>&1 | tee "${MIRI_LOG_FILE}" || miri_exit=$?

	end_ms=$(date +%s%3N 2>/dev/null || date +%s)
	duration_ms=$((end_ms - start_ms))

	# 7. Parse test result counts from log (sum across all test binaries).
	local tests_run=0 tests_failed=0
	tests_run=$(grep -oE '[0-9]+ passed' "${MIRI_LOG_FILE}" 2>/dev/null | awk '{s+=$1} END {print s+0}')
	tests_failed=$(grep -oE '[0-9]+ failed' "${MIRI_LOG_FILE}" 2>/dev/null | awk '{s+=$1} END {print s+0}')

	# 8. Determine final status.
	local status
	if [ "${miri_exit}" = "0" ]; then
		status="verified"
		log_success "Miri verification PASSED (${tests_run} tests, 0 fail, ${duration_ms}ms)"
	else
		status="failed"
		log_error "Miri verification FAILED (${tests_failed} failures, ${duration_ms}ms). See ${MIRI_LOG_FILE}."
	fi

	# 9. Write stamp file. Skip stamp update if --filter or --full was used
	#    (partial runs don't represent the audited scope).
	if [ -n "${filter}" ] || [ "${full}" = "1" ]; then
		log_info "Partial run — stamp file not updated. Run './scripts/build.sh miri' (no flags) to refresh."
	else
		local head head_short ts_iso ts_unix
		head=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
		head_short=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
		ts_unix=$(date +%s)
		ts_iso=$(date -u +%Y-%m-%dT%H:%M:%SZ)

		cat >"${MIRI_STAMP_FILE}" <<STAMP_EOF
# cosmostrix Miri verification stamp (generated by scripts/build.sh miri)
# Format: key=value. Parse with grep '^key=' | cut -d= -f2-
commit=${head}
commit_short=${head_short}
timestamp_unix=${ts_unix}
timestamp_iso=${ts_iso}
status=${status}
duration_ms=${duration_ms}
tests_run=${tests_run}
tests_failed=${tests_failed}
modules=$(
			IFS=','
			echo "${MIRI_AUDIT_MODULES[*]}"
		)
miri_version=${miri_ver}
nightly_version=${nightly_ver}
STAMP_EOF
		log_info "Stamp written to ${MIRI_STAMP_FILE}"
		log_info "Log saved to ${MIRI_LOG_FILE}"
	fi

	if [ "${miri_exit}" != "0" ]; then
		exit 1
	fi
}

# ── PGO (Profile-Guided Optimization) nitro build ───────────────────────
# Two-stage: instrument → benchmark → recompile with profile data.
# Expected gain: 5-15% FPS improvement over the pro profile.

# Detect the best -C target-cpu value for the host machine.
#   - Linux x86_64: reads /proc/cpuinfo for avx512f / avx2 flags
#   - macOS x86_64: uses sysctl machdep.cpu.features
#   - aarch64 / other: returns "native" (LLVM already tunes well for ARM)
# Echoes the target-cpu value (suitable for COSMOSTRIX_TARGET_CPU).
detect_cpu_target() {
	local arch
	arch="$(uname -m 2>/dev/null || echo unknown)"

	# ARM and other non-x86 architectures: LLVM's codegen already
	# produces excellent ARM64 code without PGO-specific tuning, and
	# there is no equivalent of the x86-64-vN microarchitecture
	# levels. Stick with native.
	if [ "${arch}" != "x86_64" ] && [ "${arch}" != "amd64" ]; then
		echo "native"
		return 0
	fi

	local flags=""
	if [ -r /proc/cpuinfo ]; then
		# Linux: grep the flags line from the first CPU entry.
		# The line is space-separated, so grep -o is safe.
		flags="$(grep -m1 -E '^flags' /proc/cpuinfo 2>/dev/null | cut -d: -f2- || true)"
	elif command -v sysctl >/dev/null 2>&1; then
		# macOS: machdep.cpu.features is a space-separated list,
		# uppercased (e.g. AVX2, AVX512F).
		flags="$(sysctl -n machdep.cpu.features 2>/dev/null || true)"
	fi

	# Normalize to lowercase for case-insensitive matching.
	flags="$(echo "${flags}" | tr '[:upper:]' '[:lower:]')"

	if echo "${flags}" | grep -qw avx512f; then
		echo "x86-64-v4"
	elif echo "${flags}" | grep -qw avx2; then
		echo "x86-64-v3"
	else
		# No AVX2 — fall back to native rather than guessing v1/v2.
		# native lets rustc pick the host's actual feature set.
		echo "native"
	fi
}

build_pgo() {
	log_step "Starting PGO nitro build (3-stage: instrument → profile → optimize)"

	local pgo_dir="${PWD}/target/pgo-data"
	local pgo_stages_dir="${PWD}/target/pgo-stages"
	local pgo_logs_dir="${PWD}/target/pgo-logs"
	local instrument_bin="target/${TARGET}/pgo-instrument/${PROJECT_NAME}"
	local nitro_bin="target/${TARGET}/pgo-use/${PROJECT_NAME}"
	local pgo_log_file
	pgo_log_file="${pgo_logs_dir}/pgo-$(date +%Y%m%d-%H%M%S).log"

	# ── Pre-flight checks ──────────────────────────────────────────
	# Fail fast on missing tools or insufficient disk space before
	# we spend 5+ minutes building the instrumented binary.
	pgo_preflight_check "${pgo_dir}"

	# Set up log directory + rotate old logs (keep last 5)
	mkdir -p "${pgo_logs_dir}"
	pgo_rotate_logs "${pgo_logs_dir}"

	# Tee all subsequent output to the log file (and stdout).
	# Use exec to redirect the script's stdout/stderr, but keep stdout
	# visible via tee.
	exec > >(tee -a "${pgo_log_file}") 2>&1
	log_info "PGO training log: ${pgo_log_file}"

	# Capture environment for reproducibility (written to log via tee)
	log_info "── PGO Environment ──"
	log_info "  rustc: $(rustc --version 2>/dev/null || echo 'unknown')"
	log_info "  cargo: $(cargo --version 2>/dev/null || echo 'unknown')"
	log_info "  host:  $(uname -s -m -r 2>/dev/null || echo 'unknown')"
	log_info "  target: ${TARGET}"
	log_info "  jobs: ${MAX_JOBS}"
	log_info "  commit: $(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
	log_info "  validate: $([ "${PGO_VALIDATE}" -eq 1 ] && echo 'yes (will build release baseline)' || echo 'no')"
	log_info "  resume: $([ "${PGO_NO_RESUME}" -eq 1 ] && echo 'disabled (--no-resume)' || echo 'enabled')"

	# PGO target CPU resolution — TWO separate targets:
	#
	#   instrument_cpu  → used for the Stage 1 instrumented binary, which
	#                     MUST EXECUTE on the build host to collect profile
	#                     data. Must be a target the host can actually run.
	#   final_cpu       → used for the Stage 3 optimized binary, which is
	#                     shipped to users. Can be more aggressive than
	#                     instrument_cpu because it never runs on the host.
	#
	# Why split them? Profile data captures branch frequencies and hot
	# paths — it is independent of SIMD codegen. So a v3-built instrumented
	# binary produces valid profile data for a v4-built final binary.
	# This lets CI build v4 PGO binaries even when the runner lacks
	# AVX-512 (which would otherwise SIGILL the instrumented binary and
	# abort Stage 2 with no profile data collected).
	#
	# final_cpu priority: COSMOSTRIX_TARGET_CPU env > --auto detect > native.
	# The --auto flag (set by `cargo use-pgo`) probes the host CPU and
	# selects the highest x86-64 microarchitecture level it supports.
	if [ -z "${COSMOSTRIX_TARGET_CPU:-}" ] && [ "${PGO_AUTO}" -eq 1 ]; then
		local detected
		detected="$(detect_cpu_target)"
		export COSMOSTRIX_TARGET_CPU="${detected}"
		log_info "Auto-detected final CPU target: ${detected}"
	fi
	local final_cpu="${COSMOSTRIX_TARGET_CPU:-native}"

	# instrument_cpu priority: COSMOSTRIX_INSTRUMENT_CPU env > safe default.
	# Safe default: x86-64-v3 on x86_64 (universally supported on every
	# modern x86_64 host including GitHub Actions runners, which lack
	# AVX-512), native elsewhere (LLVM handles ARM tuning well).
	local instrument_cpu="${COSMOSTRIX_INSTRUMENT_CPU:-}"
	if [ -z "${instrument_cpu}" ]; then
		local instr_arch
		instr_arch="$(uname -m 2>/dev/null || echo unknown)"
		if [ "${instr_arch}" = "x86_64" ] || [ "${instr_arch}" = "amd64" ]; then
			instrument_cpu="x86-64-v3"
		else
			instrument_cpu="native"
		fi
	fi
	log_info "Instrument CPU: ${instrument_cpu} (must run on build host)"
	log_info "Final CPU: ${final_cpu}"

	mkdir -p "${pgo_dir}" "${pgo_stages_dir}"

	# ── Stage 1: Build instrumented binary ─────────────────────────
	# Crash recovery: if stage-1 stamp exists + commit matches + binary
	# exists, skip Stage 1 (saves ~60s on re-runs after Stage 2/3 failure).
	local stage1_stamp="${pgo_stages_dir}/stage-1-instrument"
	if [ "${PGO_NO_RESUME}" -eq 0 ] && pgo_stage_stamp_valid "${stage1_stamp}" "${instrument_cpu}" "${final_cpu}"; then
		if [ -f "${instrument_bin}" ]; then
			log_success "Stage 1/3: SKIPPED (stamp valid + binary exists)"
			log_info "  Use --no-resume to force rebuild"
		else
			log_warning "Stage 1 stamp valid but binary missing — rebuilding"
			pgo_stage_run_1 "${pgo_dir}" "${instrument_cpu}" "${instrument_bin}"
		fi
	else
		pgo_stage_run_1 "${pgo_dir}" "${instrument_cpu}" "${instrument_bin}"
	fi
	pgo_stage_stamp_write "${stage1_stamp}" "${instrument_cpu}" "${final_cpu}"

	# ── Stage 2: Run multi-workload PGO training ───────────────────
	# Multi-workload PGO training:
	#   1. monolith + zen (default, peak throughput — train hot loop)
	#   2. cinematic + katana (heavier scene, glyph pool — train glyph-emit)
	#   3. signal + binary (glyph scene with anomaly zones — train post-fx)
	#   4. screensaver + message box (train message-box overlay + screensaver)
	#
	# All runs use --bench-io (wet I/O) so the BenchIoWriter +
	# VisualSampler + clear_dirty paths are exercised. This was the
	# biggest gap in the old training — the wet path was completely
	# untrained, causing PGO to emit suboptimal code for it.
	#
	# Crash recovery: if stage-2 stamp exists + commit matches + at least
	# 4 profraw files exist, skip Stage 2 (saves ~50s on re-runs after
	# Stage 3 failure).
	local stage2_stamp="${pgo_stages_dir}/stage-2-train"
	local stage2_skip=0
	if [ "${PGO_NO_RESUME}" -eq 0 ] && pgo_stage_stamp_valid "${stage2_stamp}" "${instrument_cpu}" "${final_cpu}"; then
		local profraw_count
		profraw_count=$(find "${pgo_dir}" -name "*.profraw" 2>/dev/null | wc -l)
		if [ "${profraw_count}" -ge 4 ]; then
			log_success "Stage 2/3: SKIPPED (stamp valid + ${profraw_count} profraw files exist)"
			log_info "  Use --no-resume to force retraining"
			stage2_skip=1
		fi
	fi

	if [ "${stage2_skip}" -eq 0 ]; then
		log_info "Stage 2/3: Running multi-workload PGO training (~50s total)..."
		if [ ! -f "${instrument_bin}" ]; then
			log_error "Stage 2 failed: instrumented binary not found at ${instrument_bin}"
			exit 1
		fi

		# Export LLVM_PROFILE_FILE so each invocation writes to a unique
		# .profraw file. %p = PID, %m = module hash. Without this, concurrent
		# or sequential runs would overwrite each other's profile data.
		export LLVM_PROFILE_FILE="${pgo_dir}/cosmostrix-%p-%m.profraw"

		local train_failed=0

		# Workload 1: monolith + zen (default, peak throughput)
		pgo_run_workload 1 4 "monolith + zen" \
			"${instrument_bin}" --benchmark -C zen --bench-io --bench-duration 20 --scene monolith ||
			train_failed=1

		# Workload 2: cinematic + katana (heavier scene, larger glyph pool)
		pgo_run_workload 2 4 "cinematic + katakana" \
			"${instrument_bin}" --benchmark -C katakana --bench-io --bench-duration 12 --scene cinematic ||
			train_failed=1

		# Workload 3: signal + binary (anomaly zones, post-fx path)
		pgo_run_workload 3 4 "signal + binary" \
			"${instrument_bin}" --benchmark -C binary --bench-io --bench-duration 10 --scene signal ||
			train_failed=1

		# Workload 4: screensaver + message box (overlay + screensaver path)
		pgo_run_workload 4 4 "screensaver + message box" \
			"${instrument_bin}" --benchmark --bench-io --bench-duration 8 --screensaver -mb "pgo training" ||
			train_failed=1

		unset LLVM_PROFILE_FILE

		local profile_count
		profile_count=$(find "${pgo_dir}" -name "*.profraw" 2>/dev/null | wc -l)
		if [ "${profile_count}" -eq 0 ]; then
			log_error "Stage 2 failed: no profile data collected in ${pgo_dir}"
			log_info "Hint: ensure the benchmark ran for at least 5 seconds"
			log_info "Hint: check LLVM_PROFILE_FILE was exported correctly"
			log_info "Hint: see ${pgo_log_file} for per-workload output"
			exit 1
		fi
		log_success "Stage 2 complete: ${profile_count} profile file(s) collected from 4 workloads"
		if [ "${train_failed}" -ne 0 ]; then
			log_warning "Some workloads exited non-zero — profile coverage may be partial"
		fi
		pgo_stage_stamp_write "${stage2_stamp}" "${instrument_cpu}" "${final_cpu}"
	fi

	# ── Merge profile data (robust — skips corrupt profraw files) ──
	local profdata_file="${pgo_dir}/cosmostrix.profdata"
	pgo_merge_profiles "${pgo_dir}" "${profdata_file}"

	# ── Stage 3: Build optimized binary with profile data ──────────
	local stage3_stamp="${pgo_stages_dir}/stage-3-optimize"
	if [ "${PGO_NO_RESUME}" -eq 0 ] && pgo_stage_stamp_valid "${stage3_stamp}" "${instrument_cpu}" "${final_cpu}"; then
		if [ -f "${nitro_bin}" ]; then
			log_success "Stage 3/3: SKIPPED (stamp valid + binary exists)"
			log_info "  Use --no-resume to force rebuild"
		else
			log_warning "Stage 3 stamp valid but binary missing — rebuilding"
			pgo_stage_run_3 "${final_cpu}" "${profdata_file}" "${nitro_bin}"
		fi
	else
		pgo_stage_run_3 "${final_cpu}" "${profdata_file}" "${nitro_bin}"
	fi
	pgo_stage_stamp_write "${stage3_stamp}" "${instrument_cpu}" "${final_cpu}"

	local size
	size=$(du -h "${nitro_bin}" | cut -f1)
	log_success "PGO nitro build complete (${size})"

	# ── Summary report ─────────────────────────────────────────────
	pgo_print_summary "${nitro_bin}" "${instrument_bin}" "${pgo_dir}" "${profdata_file}" \
		"${instrument_cpu}" "${final_cpu}" "${pgo_log_file}"

	# ── Optional: delta validation (PGO vs release) ────────────────
	if [ "${PGO_VALIDATE}" -eq 1 ]; then
		pgo_validate_delta "${nitro_bin}" "${final_cpu}"
	else
		echo ""
		log_info "PGO gain: expected 5-15% FPS improvement over pro profile"
		log_info "Run: ${nitro_bin} --benchmark to measure"
		log_info "Add --validate to automatically build release baseline + compare FPS"
	fi
}

# ── PGO helper functions ────────────────────────────────────────────────

# Pre-flight checks: verify required tools + disk space before starting.
# Fails fast on missing llvm-profdata or insufficient disk space.
pgo_preflight_check() {
	local pgo_dir="$1"

	log_step "Pre-flight checks..."

	# 1. rustc supports -C profile-generate (stable since Rust 1.0,
	#    but verify the flag is recognized)
	# NOTE: capture output to a variable first, then grep. Direct pipe
	# (`rustc -C help | grep -q`) fails under `set -o pipefail` because
	# grep -q exits early, rustc gets SIGPIPE, and pipefail makes the
	# whole pipe fail.
	local rustc_help
	rustc_help=$(rustc -C help 2>/dev/null || true)
	if ! echo "${rustc_help}" | grep -q 'profile-generate'; then
		log_error "rustc does not support -C profile-generate. PGO requires Rust stable."
		exit 1
	fi
	log_success "  rustc supports -C profile-generate"

	# 2. llvm-profdata available (either in PATH or via rustup llvm-tools)
	local profdata_tool=""
	if command -v llvm-profdata >/dev/null 2>&1; then
		profdata_tool="llvm-profdata"
	else
		local rustup_profdata
		rustup_profdata="$(rustc --print sysroot 2>/dev/null)/lib/rustlib/$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')/bin/llvm-profdata"
		if [ -x "${rustup_profdata}" ]; then
			profdata_tool="${rustup_profdata}"
		fi
	fi
	if [ -z "${profdata_tool}" ]; then
		log_error "llvm-profdata not found. Install with: rustup component add llvm-tools-preview"
		log_error "If already installed, the rust-toolchain.toml override may be hiding it."
		log_error "The binary lives at: \$(rustc --print sysroot)/lib/rustlib/\$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-profdata"
		exit 1
	fi
	log_success "  llvm-profdata: ${profdata_tool}"

	# 3. Disk space check — PGO data can be 100s of MB
	local avail_mb
	avail_mb=$(df -m "${pgo_dir%/*}" 2>/dev/null | awk 'NR==2 {print $4}' || echo 0)
	if [ "${avail_mb}" -gt 0 ] && [ "${avail_mb}" -lt 500 ]; then
		log_error "Insufficient disk space: ${avail_mb}MB available, need at least 500MB for PGO data"
		log_info "PGO data lives in ${pgo_dir} (~100-300MB typical)"
		exit 1
	fi
	if [ "${avail_mb}" -gt 0 ]; then
		log_success "  Disk space: ${avail_mb}MB available"
	else
		log_warning "  Could not determine disk space (df failed) — proceeding anyway"
	fi

	# 4. Target directory writable
	if ! mkdir -p "${pgo_dir}" 2>/dev/null; then
		log_error "Cannot create PGO data directory: ${pgo_dir}"
		exit 1
	fi
	log_success "  PGO data directory writable: ${pgo_dir}"
}

# Run a single PGO training workload with logging + crash tolerance.
# Args: workload_index total_workloads description bin [args...]
# Captures stdout/stderr to target/pgo-logs/workload-N.log
# Returns 0 on success, 1 on non-zero exit (does NOT abort the pipeline).
pgo_run_workload() {
	local idx="$1"
	local total="$2"
	local desc="$3"
	shift 3
	local bin="$1"
	shift

	local workload_log="${PWD}/target/pgo-logs/workload-${idx}.log"
	log_info "  [${idx}/${total}] ${desc}"

	if "$bin" "$@" >"${workload_log}" 2>&1; then
		log_success "    workload ${idx} completed"
		return 0
	else
		local exit_code=$?
		log_warning "    workload ${idx} exited non-zero (exit ${exit_code}) — see ${workload_log}"
		# Don't abort — partial profile data is still useful
		return 1
	fi
}

# Robust profile merge: skip corrupt/empty profraw files, report stats.
# Args: pgo_dir profdata_file
pgo_merge_profiles() {
	local pgo_dir="$1"
	local profdata_file="$2"

	local profdata_tool=""
	if command -v llvm-profdata >/dev/null 2>&1; then
		profdata_tool="llvm-profdata"
	else
		local rustup_profdata
		rustup_profdata="$(rustc --print sysroot 2>/dev/null)/lib/rustlib/$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')/bin/llvm-profdata"
		if [ -x "${rustup_profdata}" ]; then
			profdata_tool="${rustup_profdata}"
		fi
	fi

	if [ -z "${profdata_tool}" ]; then
		log_warning "llvm-profdata not found. Using raw profdata directory (rustc can handle this)"
		return
	fi

	log_info "Merging profile data with ${profdata_tool}..."

	# Validate each profraw file before merging — skip corrupt ones.
	local valid_files=()
	local skipped=0
	local total_size=0
	for profraw in "${pgo_dir}"/*.profraw; do
		[ -f "${profraw}" ] || continue
		local fsize
		fsize=$(stat -c%s "${profraw}" 2>/dev/null || stat -f%z "${profraw}" 2>/dev/null || echo 0)
		if [ "${fsize}" -lt 100 ]; then
			log_warning "  Skipping ${profraw##*/}: too small (${fsize} bytes — likely empty/corrupt)"
			skipped=$((skipped + 1))
			continue
		fi
		valid_files+=("${profraw}")
		total_size=$((total_size + fsize))
	done

	if [ "${#valid_files[@]}" -eq 0 ]; then
		log_error "No valid profraw files to merge"
		exit 1
	fi

	local total_size_mb=$((total_size / 1024 / 1024))
	log_info "  Merging ${#valid_files[@]} profraw files (${total_size_mb}MB total, ${skipped} skipped)"

	if "${profdata_tool}" merge -o "${profdata_file}" "${valid_files[@]}" 2>&1 | tail -3; then
		local profdata_size
		profdata_size=$(stat -c%s "${profdata_file}" 2>/dev/null || stat -f%z "${profdata_file}" 2>/dev/null || echo 0)
		local profdata_size_mb=$((profdata_size / 1024 / 1024))
		log_success "  Merged profdata: ${profdata_size_mb}MB (${profdata_file})"
	else
		log_error "  profdata merge failed"
		exit 1
	fi
}

# Stage 1 (build instrumented binary) — extracted for reuse by resume logic.
# Args: pgo_dir instrument_cpu instrument_bin
pgo_stage_run_1() {
	local pgo_dir="$1"
	local instrument_cpu="$2"
	local instrument_bin="$3"

	log_info "Stage 1/3: Building instrumented binary..."
	export COSMOSTRIX_BUILD="nitro-pgo-instrument"
	export COSMOSTRIX_PROFILE="pgo-instrument"
	export COSMOSTRIX_LTO="off"
	export COSMOSTRIX_STRIP="no"
	# Use instrument_cpu (safe target) so the binary can execute on the
	# build host. The final binary (Stage 3) uses final_cpu for shipping.
	export RUSTFLAGS="-C target-cpu=${instrument_cpu} -C profile-generate=${pgo_dir}"

	if ! cargo build --profile pgo-instrument --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		log_error "Stage 1 failed: instrumented build failed"
		exit 1
	fi
	log_success "Stage 1 complete: instrumented binary built"
}

# Stage 3 (build optimized PGO binary) — extracted for reuse by resume logic.
# Args: final_cpu profdata_file nitro_bin
pgo_stage_run_3() {
	local final_cpu="$1"
	local profdata_file="$2"
	local nitro_bin="$3"

	log_info "Stage 3/3: Building PGO-optimized nitro binary..."
	export COSMOSTRIX_BUILD="nitro-pgo"
	export COSMOSTRIX_PROFILE="pgo-use"
	export COSMOSTRIX_LTO="fat"
	export COSMOSTRIX_STRIP="yes"
	export RUSTFLAGS="-C target-cpu=${final_cpu} -C profile-use=${profdata_file}"
	# Append hardened flags (path remap + frame pointers) for the
	# shipping binary. Idempotent — safe to call after RUSTFLAGS is
	# set above. The instrumented binary (Stage 1) is never shipped
	# so it skips hardening.
	apply_hardened_rustflags

	if ! cargo build --profile pgo-use --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		log_error "Stage 3 failed: PGO-optimized build failed"
		exit 1
	fi
	log_success "Stage 3 complete: PGO-optimized binary built"
}

# Check if a stage stamp is valid (exists + commit matches HEAD).
# Args: stamp_file instrument_cpu final_cpu
# Returns 0 if valid, 1 if invalid/missing.
pgo_stage_stamp_valid() {
	local stamp_file="$1"
	local expected_instr_cpu="$2"
	local expected_final_cpu="$3"

	[ -f "${stamp_file}" ] || return 1

	local stamp_commit stamp_instr stamp_final
	stamp_commit=$(grep '^commit=' "${stamp_file}" | cut -d= -f2-)
	stamp_instr=$(grep '^instrument_cpu=' "${stamp_file}" | cut -d= -f2-)
	stamp_final=$(grep '^final_cpu=' "${stamp_file}" | cut -d= -f2-)

	local head
	head=$(git rev-parse HEAD 2>/dev/null || echo "")

	[ "${stamp_commit}" = "${head}" ] || return 1
	[ "${stamp_instr}" = "${expected_instr_cpu}" ] || return 1
	[ "${stamp_final}" = "${expected_final_cpu}" ] || return 1
	return 0
}

# Write a stage stamp file recording commit + CPU targets.
# Args: stamp_file instrument_cpu final_cpu
pgo_stage_stamp_write() {
	local stamp_file="$1"
	local instrument_cpu="$2"
	local final_cpu="$3"

	local head ts_iso
	head=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
	ts_iso=$(date -u +%Y-%m-%dT%H:%M:%SZ)

	cat >"${stamp_file}" <<STAMP_EOF
# PGO stage stamp (generated by scripts/build.sh pgo)
commit=${head}
timestamp_iso=${ts_iso}
instrument_cpu=${instrument_cpu}
final_cpu=${final_cpu}
target=${TARGET}
STAMP_EOF
}

# Rotate PGO logs — keep last 5 timestamped logs + always keep workload-N.log
# (overwritten each run).
# Args: logs_dir
pgo_rotate_logs() {
	local logs_dir="$1"
	[ -d "${logs_dir}" ] || return 0

	# List timestamped logs (pgo-YYYYMMDD-HHMMSS.log), oldest first
	local logs=()
	while IFS= read -r f; do
		logs+=("$f")
	done < <(find "${logs_dir}" -maxdepth 1 -name 'pgo-*.log' 2>/dev/null | sort)

	# Keep last 5; delete the rest
	local count="${#logs[@]}"
	if [ "${count}" -gt 5 ]; then
		local to_delete=$((count - 5))
		local i=0
		for f in "${logs[@]}"; do
			if [ "${i}" -lt "${to_delete}" ]; then
				rm -f "${f}"
				i=$((i + 1))
			else
				break
			fi
		done
		log_info "Rotated ${to_delete} old PGO log(s) (kept last 5)"
	fi
}

# Print consolidated PGO summary report.
# Args: nitro_bin instrument_bin pgo_dir profdata_file instrument_cpu final_cpu log_file
pgo_print_summary() {
	local nitro_bin="$1"
	local instrument_bin="$2"
	local pgo_dir="$3"
	local profdata_file="$4"
	local instrument_cpu="$5"
	local final_cpu="$6"
	local log_file="$7"

	local nitro_size instr_size profraw_count profdata_size
	nitro_size=$(du -h "${nitro_bin}" 2>/dev/null | cut -f1 || echo "?")
	instr_size=$(du -h "${instrument_bin}" 2>/dev/null | cut -f1 || echo "?")
	profraw_count=$(find "${pgo_dir}" -name "*.profraw" 2>/dev/null | wc -l)
	if [ -f "${profdata_file}" ]; then
		profdata_size=$(du -h "${profdata_file}" 2>/dev/null | cut -f1 || echo "?")
	else
		profdata_size="(raw dir)"
	fi

	echo ""
	echo "── PGO Build Summary ────────────────────────────────────────────"
	printf "  PGO binary:        %s (%s)\n" "${nitro_bin}" "${nitro_size}"
	printf "  Instrument binary: %s (%s)\n" "${instrument_bin}" "${instr_size}"
	printf "  Profile data:      %s (%s profraw → %s profdata)\n" \
		"${pgo_dir}" "${profraw_count}" "${profdata_size}"
	printf "  Instrument CPU:    %s (must run on host)\n" "${instrument_cpu}"
	printf "  Final CPU:         %s (shipping binary)\n" "${final_cpu}"
	printf "  Training log:      %s\n" "${log_file}"
	echo "────────────────────────────────────────────────────────────────"
}

# Validate PGO gain by building a release baseline + comparing FPS.
# Args: nitro_bin final_cpu
pgo_validate_delta() {
	local nitro_bin="$1"
	local final_cpu="$2"
	local release_bin="target/${TARGET}/release/${PROJECT_NAME}"

	echo ""
	log_step "Delta validation: PGO vs release baseline"

	# 1. Build release baseline (with same final_cpu for fair comparison)
	log_info "Building release baseline (target-cpu=${final_cpu})..."
	export COSMOSTRIX_BUILD="pgo-validate-baseline"
	export COSMOSTRIX_PROFILE="release"
	export COSMOSTRIX_LTO="off"
	export COSMOSTRIX_STRIP="yes"
	export RUSTFLAGS="-C target-cpu=${final_cpu}"
	apply_hardened_rustflags

	if ! cargo build --profile release --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		log_warning "Release baseline build failed — skipping delta validation"
		return 1
	fi
	log_success "Release baseline built"

	# 2. Run --benchmark on both binaries (10s each, same scene)
	local bench_scene="monolith"
	local bench_color="zen"
	local bench_duration=10

	log_info "Benchmarking release baseline (${bench_duration}s, ${bench_scene}/${bench_color})..."
	local release_fps
	release_fps=$("${release_bin}" --benchmark -C "${bench_color}" --bench-io \
		--bench-duration "${bench_duration}" --scene "${bench_scene}" 2>/dev/null |
		grep -oE 'avg_fps[": ]+[0-9.]+' | head -1 | grep -oE '[0-9.]+$' || echo "0")
	log_info "  release avg_fps: ${release_fps}"

	log_info "Benchmarking PGO binary (${bench_duration}s, ${bench_scene}/${bench_color})..."
	local pgo_fps
	pgo_fps=$("${nitro_bin}" --benchmark -C "${bench_color}" --bench-io \
		--bench-duration "${bench_duration}" --scene "${bench_scene}" 2>/dev/null |
		grep -oE 'avg_fps[": ]+[0-9.]+' | head -1 | grep -oE '[0-9.]+$' || echo "0")
	log_info "  pgo avg_fps:     ${pgo_fps}"

	# 3. Compute delta
	if [ "${release_fps}" = "0" ] || [ "${pgo_fps}" = "0" ]; then
		log_warning "Could not extract avg_fps from benchmark output — skipping delta"
		log_info "Run manually: ${nitro_bin} --benchmark"
		return 1
	fi

	local delta_pct
	delta_pct=$(awk -v pgo="${pgo_fps}" -v rel="${release_fps}" \
		'BEGIN { if (rel > 0) printf "%+.1f", (pgo - rel) / rel * 100; else print "N/A" }')

	echo ""
	echo "── PGO Delta Report ─────────────────────────────────────────────"
	printf "  Release baseline: %s avg_fps\n" "${release_fps}"
	printf "  PGO binary:       %s avg_fps\n" "${pgo_fps}"
	printf "  Delta:            %s%%\n" "${delta_pct}"
	echo "────────────────────────────────────────────────────────────────"

	# 4. Regression warning
	if awk -v pgo="${pgo_fps}" -v rel="${release_fps}" \
		'BEGIN { exit !(pgo < rel * 0.98) }'; then
		log_warning "PGO regression detected: PGO is slower than release by more than 2%"
		log_info "This may indicate:"
		log_info "  - Training workloads don't match production usage"
		log_info "  - Profile data is stale (commit changed since training)"
		log_info "  - LLVM version mismatch between instrument + optimize"
		log_info "Try: ./scripts/build.sh pgo --no-resume (forces full rebuild)"
	else
		log_success "PGO validation passed: ${delta_pct}% vs release"
	fi
}

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
