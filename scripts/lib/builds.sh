# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/build.sh — not a standalone executable (no main, no
#   argument parsing; the entry script owns both).
#
# COSMOSTRIX BUILD MODULE: builds
# cargo build commands (debug / alias-dispatched release /
# release-with-debug), dependency update + audit, artifact cleaning,
# cache statistics, the benchmark passthrough and the release-build
# verifier.
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

# pick_release_alias: resolve the host-matched release alias, the
# binary output path, and the CPU pin. One line of output:
#   <alias>|<binary-path>|<cpu-flag>
# The alias law (night-dragon-maintenance-3, owner directive): release
# builds ride the named cargo alias family — never a bare
# `cargo build --release`. The CPU detection reuses lib/pgo.sh
# detect_cpu_target (sourced by the entry script). The pro-native
# fallback carries no --target pin, so the build always matches the
# running machine on any host the script supports.
pick_release_alias() {
        local os arch cpu
        os="$(uname -s 2>/dev/null || echo unknown)"
        arch="$(uname -m 2>/dev/null || echo unknown)"
        cpu="$(detect_cpu_target 2>/dev/null || echo native)"

        case "${os}:${arch}" in
        Linux:x86_64 | Linux:amd64)
                case "${cpu}" in
                x86-64-v4)
                        echo "pro-linux-amd64-v4-gnu|target/x86_64-unknown-linux-gnu/pro-linux-amd64-v4-gnu/${PROJECT_NAME}|-Ctarget-cpu=x86-64-v4"
                        ;;
                x86-64-v3)
                        echo "pro-linux-amd64-v3-gnu|target/x86_64-unknown-linux-gnu/pro-linux-amd64-v3-gnu/${PROJECT_NAME}|-Ctarget-cpu=x86-64-v3"
                        ;;
                *)
                        echo "pro-native|target/pro-native/${PROJECT_NAME}|-Ctarget-cpu=native"
                        ;;
                esac
                ;;
        Darwin:arm64)
                echo "pro-macos-aarch64-native|target/aarch64-apple-darwin/pro-macos-aarch64-native/${PROJECT_NAME}|-Ctarget-cpu=native"
                ;;
        FreeBSD:x86_64 | FreeBSD:amd64)
                echo "pro-freebsd-amd64|target/x86_64-unknown-freebsd/pro-freebsd-amd64/${PROJECT_NAME}|-Ctarget-cpu=native"
                ;;
        *)
                echo "pro-native|target/pro-native/${PROJECT_NAME}|-Ctarget-cpu=native"
                ;;
        esac
}

build_release() {
        log_step "Building optimized release binary (named-alias dispatch)..."

        local pick alias binary cpu_flag
        pick="$(pick_release_alias)"
        alias="${pick%%|*}"
        pick="${pick#*|}"
        binary="${pick%%|*}"
        cpu_flag="${pick##*|}"

        # Composed RUSTFLAGS: the CPU pin plus the hardened flags, the exact
        # composition release.yml ships with. Set at step level so the env
        # deterministically wins over the alias's build.rustflags config
        # (cargo precedence: RUSTFLAGS env > build.rustflags) — the same
        # env-shadow law supermassive-kernel.yml fixed: an ambient RUSTFLAGS
        # would otherwise silently drop the alias's target-cpu pin.
        # COSMOSTRIX_NO_HARDEN=1 keeps the dev-iteration opt-out: the CPU pin
        # still composes, only the hardening flags are skipped.
        if [ "${COSMOSTRIX_NO_HARDEN:-0}" = "1" ]; then
                export RUSTFLAGS="${cpu_flag}"
        else
                export RUSTFLAGS="${cpu_flag} --remap-path-prefix=${HOME}/.cargo=redacted --remap-path-prefix=${PWD}=redacted -Cforce-frame-pointers=yes"
        fi
        export CARGO_BUILD_JOBS="${MAX_JOBS}"

        if cargo "${alias}"; then
                local size
                size=$(du -h "$binary" 2>/dev/null | cut -f1 || echo "unknown")
                log_success "Release build complete via ${alias} (${size})"
                echo "  └─ Binary: ${binary}"
        else
                log_error "Release build failed (${alias})"
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
