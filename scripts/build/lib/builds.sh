# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/build/build.sh — not a standalone executable (no main, no
#   argument parsing; the entry script owns both).
#
# COSMOSTRIX BUILD MODULE: builds
# cargo build commands (debug / release / release-with-debug),
# dependency update + audit, artifact cleaning, cache statistics, the
# benchmark passthrough and the release-build verifier.
#
# Part of the build.sh split (NIGHT-lts-2): the single-entry orchestrator
# was de-monolithed into sourced modules; function bodies are byte-identical
# moves from the former flat script.

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

	if scripts/release/verify-release-build.sh; then
		log_success "Release build verification complete"
	else
		log_error "Release build verification failed"
		return 1
	fi
}
