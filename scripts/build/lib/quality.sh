# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/build/build.sh — not a standalone executable (no main, no
#   argument parsing; the entry script owns both).
#
# COSMOSTRIX BUILD MODULE: quality
# the check family: fmt, clippy, tests (nextest or stock),
# cross-platform type check, LOC guards, SPDX headers, symbol-only output,
# version anti-patterns, shellcheck, ruff, version sync, plus the
# comprehensive (check-all) and quick (check) aggregates.
#
# Part of the build.sh split (NIGHT-lts-2): the single-entry orchestrator
# was de-monolithed into sourced modules; function bodies are byte-identical
# moves from the former flat script.

run_tests() {
	log_step "Running test suite..."

	local test_output
	if [ "${NEXTEST_AVAILABLE:-0}" -eq 1 ]; then
		if [ "${QUIET_CHECK}" -eq 1 ]; then
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
		if [ "${QUIET_CHECK}" -eq 1 ]; then
			test_output=$(cargo test --target "${TARGET}" --jobs "${MAX_JOBS}" -- --test-threads="${MAX_JOBS}" 2>&1)
			local rc=$?
			# NIGHT-enhanced-3: hide the "test result: ok. <N> passed"
			# success line - only surface actual failures. The previous
			# `test result:` match was leaking the passing summary into
			# quiet output, contradicting the -q brief ("only failures/
			# warnings").
			echo "$test_output" | grep -E '(FAILED|failures:|error\[|panicked at|^test .* FAILED)' || true
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

	if [ "${QUIET_CHECK}" -eq 1 ]; then
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

	# NIGHT-enhanced-3: in quiet mode, capture `cargo fmt --check`
	# output and surface only the actual formatting violations
	# (the "Diff in <path>:" headers and unified diff hunks). The
	# success path is silent - log_success_quietable gates on
	# QUIET_CHECK, and the empty captured output produces no
	# stdout. Without this branch, cargo fmt would write its diff
	# directly to stdout even in quiet mode.
	if [ "${QUIET_CHECK}" -eq 1 ]; then
		local fmt_output
		fmt_output=$(cargo fmt --all -- --check 2>&1)
		local rc=$?
		if [ $rc -ne 0 ]; then
			echo "$fmt_output"
			log_error "Formatting issues found. Run: cargo fmt --all"
		fi
		return $rc
	fi

	if cargo fmt --all -- --check 2>&1; then
		log_success "Code formatting is correct"
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
		# CI parity (2026-09-04 lesson): the CI cross-build jobs run with
		# RUSTFLAGS=-D warnings, so cfg-gated unused warnings (unused_mut
		# on windows, unused imports on non-x86_64) are ERRORS in CI — a
		# bare cargo check exits 0 on them locally (f19470a6 warning arm).
		if RUSTFLAGS="-D warnings" cargo check --quiet --target "${t}" 2>/dev/null; then
			log_success_quietable "  ${t}: OK"
		else
			log_error "  ${t}: FAILED — reproduce with: RUSTFLAGS='-D warnings' cargo check --target ${t}"
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

	if [ "${QUIET_CHECK}" -eq 1 ]; then
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

	if [ ! -x "scripts/gates/check-rs-loc.sh" ]; then
		log_warning "scripts/gates/check-rs-loc.sh not found or not executable (skipping)"
		return 0
	fi

	local loc_output
	if [ "${QUIET_CHECK}" -eq 1 ]; then
		loc_output=$(bash scripts/gates/check-rs-loc.sh 2>&1)
		local rc=$?
		# NIGHT-enhanced-3: the old grep `(FAIL|ERROR|over|exceeds)`
		# was too permissive - the substring "over" also matches
		# filenames containing "recovery", "overrides", "discover"
		# etc., leaking unrelated LOC count lines into the quiet
		# output. Tighten to the actual violation surface:
		# - "VIOLATES" matches the per-file `^^^ VIOLATES <N> limit`
		#   line emitted by check-rs-loc.sh when a file exceeds the
		#   cap without an exemption marker.
		# - "^FAIL:" matches the trailing summary block printed only
		#   when at least one non-exempt violation exists.
		# - "ERROR" matches any future hard-error path.
		# The summary lines ("Files over 800 (exempt ...): N",
		# "Files over 800 (NOT exempt - BUILD FAIL): 0") are
		# intentionally NOT matched - they are success-path
		# informational output (the count of exempt files is debt
		# tracking, not a failure).
		echo "$loc_output" | grep -E '(VIOLATES|^FAIL:|ERROR)' || true
		if [ $rc -eq 0 ]; then
			log_success_quietable "LOC check passed"
		else
			log_error "LOC check failed"
		fi
		return $rc
	else
		if bash scripts/gates/check-rs-loc.sh; then
			log_success "LOC check passed"
		else
			log_error "LOC check failed"
			return 1
		fi
	fi
}

run_scripts_loc_check() {
	log_step "Checking scripts file sizes (1K LOC cap)..."

	if [ ! -x "scripts/gates/check-scripts-loc.sh" ]; then
		log_warning "scripts/gates/check-scripts-loc.sh not found or not executable (skipping)"
		return 0
	fi

	local sloc_output
	if [ "${QUIET_CHECK}" -eq 1 ]; then
		sloc_output=$(bash scripts/gates/check-scripts-loc.sh 2>&1)
		local rc=$?
		# Same quiet-mode surface as run_loc_check above: only the
		# per-file VIOLATES lines, the trailing FAIL block and hard
		# errors surface; the exempt-debt summary lines are
		# success-path information and stay hidden.
		echo "$sloc_output" | grep -E '(VIOLATES|^FAIL:|ERROR)' || true
		if [ $rc -eq 0 ]; then
			log_success_quietable "Scripts LOC check passed"
		else
			log_error "Scripts LOC check failed"
		fi
		return $rc
	else
		if bash scripts/gates/check-scripts-loc.sh; then
			log_success "Scripts LOC check passed"
		else
			log_error "Scripts LOC check failed"
			return 1
		fi
	fi
}

run_header_check() {
	log_step "Checking SPDX license headers..."

	if [ ! -f "scripts/gates/check-headers.sh" ]; then
		log_error "scripts/gates/check-headers.sh not found"
		return 1
	fi

	local hdr_output
	if [ "${QUIET_CHECK}" -eq 1 ]; then
		hdr_output=$(bash scripts/gates/check-headers.sh 2>&1)
		local rc=$?
		echo "$hdr_output" | grep -iE '(missing|FAIL|ERROR)' || true
		if [ $rc -eq 0 ]; then
			log_success_quietable "Header check passed"
		else
			log_error "Header check failed"
		fi
		return $rc
	else
		if bash scripts/gates/check-headers.sh; then
			log_success "Header check passed"
		else
			log_error "Header check failed"
			return 1
		fi
	fi
}

run_symbol_only_output_check() {
	log_step "Checking for icon glyphs in output surfaces (symbol-only rule)..."

	if [ ! -f "scripts/gates/check-symbol-only-output.sh" ]; then
		log_error "scripts/gates/check-symbol-only-output.sh not found"
		return 1
	fi

	if [ "${QUIET_CHECK}" -eq 1 ]; then
		local soo_output
		soo_output=$(bash scripts/gates/check-symbol-only-output.sh 2>&1)
		local soo_rc=$?
		echo "$soo_output" | grep -iE '(FAIL|VIOLATION)' || true
		if [ $soo_rc -eq 0 ]; then
			log_success_quietable "Symbol-only output check passed"
		else
			log_error "Symbol-only output check failed (icon glyphs in output - see docs/RULES.md Output Glyph Policy)"
			return 1
		fi
	else
		if bash scripts/gates/check-symbol-only-output.sh; then
			log_success "Symbol-only output check passed"
		else
			log_error "Symbol-only output check failed (icon glyphs in output - see docs/RULES.md Output Glyph Policy)"
			return 1
		fi
	fi
}

run_version_anti_pattern_check() {
	log_step "Checking for hardcoded version-string anti-patterns..."

	if [ ! -f "scripts/gates/check-version-anti-patterns.sh" ]; then
		log_error "scripts/gates/check-version-anti-patterns.sh not found"
		return 1
	fi

	local vap_output
	if [ "${QUIET_CHECK}" -eq 1 ]; then
		vap_output=$(bash scripts/gates/check-version-anti-patterns.sh 2>&1)
		local rc=$?
		# NIGHT-enhanced-3: the old grep
		# `(FAIL|ERROR|found|anti-pattern)` also matched the
		# success-path line "OK: <N> source files checked, no
		# version-anti-pattern violations" because the success
		# summary itself contains the word "anti-pattern".
		# Tighten to the actual violation surface: per-file
		# `VIOLATION: <path>` lines and the `^FAIL:` summary block.
		echo "$vap_output" | grep -E '(^VIOLATION:|^FAIL:|ERROR)' || true
		if [ $rc -eq 0 ]; then
			log_success_quietable "Version anti-pattern check passed"
		else
			log_error "Version anti-pattern check failed (use env!(\"CARGO_PKG_VERSION\") instead)"
			return 1
		fi
	else
		if bash scripts/gates/check-version-anti-patterns.sh; then
			log_success "Version anti-pattern check passed"
		else
			log_error "Version anti-pattern check failed (use env!(\"CARGO_PKG_VERSION\") instead)"
			return 1
		fi
	fi

	log_step "Checking Rust version sync across all sources..."

	if [ ! -f "scripts/gates/check-rust-version-sync.sh" ]; then
		log_error "scripts/gates/check-rust-version-sync.sh not found"
		return 1
	fi

	if [ "${QUIET_CHECK}" -eq 1 ]; then
		local vs_output
		vs_output=$(bash scripts/gates/check-rust-version-sync.sh 2>&1)
		local vs_rc=$?
		echo "$vs_output" | grep -iE '(FAIL|ERROR|mismatch|desync|differ)' || true
		if [ $vs_rc -eq 0 ]; then
			log_success_quietable "Rust version sync check passed"
		else
			log_error "Rust version sync check failed — see output above for mismatched sources"
			return 1
		fi
	else
		if bash scripts/gates/check-rust-version-sync.sh; then
			log_success "Rust version sync check passed"
		else
			log_error "Rust version sync check failed — see output above for mismatched sources"
			return 1
		fi
	fi
}

run_shellcheck() {
	log_step "Running shellcheck on scripts/ (recursive)..."

	if ! command -v shellcheck >/dev/null 2>&1; then
		log_warning "shellcheck not installed (skipping). Install: apt install shellcheck or brew install shellcheck"
		return 0
	fi

	# NIGHT-refactor-1: scripts live in category subdirectories now;
	# the old scripts/*.sh glob silently skipped every subdir script.
	# Recursive find, same convention as gate-keepers.sh check 1.
	local SH_FILES
	SH_FILES=$(find scripts -name '*.sh' 2>/dev/null | sort)
	if [ -z "$SH_FILES" ]; then
		log_warning "no .sh files found under scripts/ (skipping)"
		return 0
	fi

	if [ "${QUIET_CHECK}" -eq 1 ]; then
		local sh_output
		# shellcheck disable=SC2086 # word splitting is intentional for file list
		sh_output=$(shellcheck ${SH_FILES} 2>&1)
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
		# shellcheck disable=SC2086 # word splitting is intentional for file list
		if shellcheck ${SH_FILES}; then
			log_success "Shellcheck passed"
		else
			log_error "Shellcheck failed — fix warnings before committing"
			return 1
		fi
	fi
}

run_python_lint() {
	log_step "Running ruff check + format on scripts/ (recursive)..."

	if ! command -v ruff >/dev/null 2>&1; then
		log_warning "ruff not installed (skipping Python lint). Install: pip install ruff"
		return 0
	fi

	local py_failed=0
	local py_output
	if [ "${QUIET_CHECK}" -eq 1 ]; then
		if ! py_output=$(ruff check scripts 2>&1); then
			echo "$py_output"
			log_error "ruff check failed — fix Python lint issues"
			((py_failed++))
		fi
		if ! py_output=$(ruff format --check scripts 2>&1); then
			echo "$py_output"
			log_error "ruff format check failed — run 'ruff format scripts' to fix"
			((py_failed++))
		fi
	else
		if ! ruff check scripts; then
			log_error "ruff check failed — fix Python lint issues"
			((py_failed++))
		fi

		if ! ruff format --check scripts; then
			log_error "ruff format check failed — run 'ruff format scripts' to fix"
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

	# NIGHT-enhanced-3: the section banner and surrounding blank
	# lines are noise in `check-all -q` mode (success/passed output
	# is hidden). log_info already gates on QUIET_CHECK, but the
	# bare `echo ""` calls bypass any gate - guard them too so
	# quiet mode produces only failures/warnings with zero framing.
	if [ "${QUIET_CHECK}" -eq 0 ]; then
		echo ""
		log_info "=== Comprehensive Code Quality Check ==="
		echo ""
	fi

	check_rust_toolchain || ((failed++))
	run_fmt_check || ((failed++))
	run_loc_check || ((failed++))
	run_scripts_loc_check || ((failed++))
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

	if [ "${QUIET_CHECK}" -eq 0 ]; then
		echo ""
	fi
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
