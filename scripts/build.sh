#!/bin/bash
#
# COSMOSTRIX BUILD AUTOMATION SCRIPT
#
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Optimized build script with intelligent core detection and advanced caching
# Author: rezky_nightky (oxyzenQ)
# Version: Stellar 4.0

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

# Functions
log_info() {
        echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
        echo -e "${GREEN}[✓]${NC} $1"
}

log_warning() {
        echo -e "${YELLOW}[⚠]${NC} $1"
}

log_error() {
        echo -e "${RED}[✗]${NC} $1" >&2
}

log_step() {
        echo -e "${CYAN}[→]${NC} $1"
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
        log_step "Configuring build acceleration..."

        # Check and setup sccache
        if command -v sccache &>/dev/null; then
                # Disable incremental compilation when using sccache (they conflict)
                export CARGO_INCREMENTAL=0
                export RUSTC_WRAPPER=sccache
                # Start sccache server if not running
                sccache --start-server 2>/dev/null || true
                log_success "sccache enabled (build caching active)"
        else
                # Enable incremental compilation when not using sccache
                export CARGO_INCREMENTAL=1
                log_warning "sccache not found. Install: cargo install sccache --locked"
        fi

        # Check for mold linker
        if command -v mold &>/dev/null; then
                export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=mold"
                log_success "mold linker enabled (faster linking)"
        elif command -v lld &>/dev/null; then
                export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=lld"
                log_success "lld linker enabled"
        else
                log_warning "Fast linker not found (mold/lld)."
        fi

        # Setup cargo-nextest if available
        if command -v cargo-nextest &>/dev/null; then
                NEXTEST_AVAILABLE=1
                log_success "cargo-nextest available (faster testing)"
        else
                NEXTEST_AVAILABLE=0
                log_warning "cargo-nextest not found. Install: cargo install cargo-nextest --locked"
        fi
}

show_system_info() {
        log_info "Build Configuration:"
        echo "  ├─ OS: $(uname -s) $(uname -m)"
        echo "  ├─ CPU Cores: $(nproc)"
        echo "  ├─ Build Jobs: ${MAX_JOBS}"
        echo "  ├─ Target: ${TARGET}"
        echo "  ├─ Rust: $(rustc --version)"
        echo "  ├─ Cargo: $(cargo --version)"
        echo "  ├─ Incremental: ${CARGO_INCREMENTAL:-1}"
        echo "  └─ Cache: ${RUSTC_WRAPPER:-none}"
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

        if [ "${NEXTEST_AVAILABLE:-0}" -eq 1 ]; then
                if cargo nextest run --target "${TARGET}" --jobs "${MAX_JOBS}"; then
                        log_success "All tests passed (nextest)"
                else
                        log_error "Tests failed"
                        return 1
                fi
        else
                if cargo test --target "${TARGET}" --jobs "${MAX_JOBS}" -- --test-threads="${MAX_JOBS}"; then
                        log_success "All tests passed"
                else
                        log_error "Tests failed"
                        return 1
                fi
        fi
}

run_clippy() {
        log_step "Running Clippy linter..."

        if cargo clippy --target "${TARGET}" --all-targets --all-features -- -D warnings; then
                log_success "Clippy checks passed"
        else
                log_error "Clippy found issues"
                return 1
        fi
}

run_fmt_check() {
        log_step "Checking code formatting..."

        if cargo fmt --all -- --check; then
                log_success "Code formatting is correct"
        else
                log_error "Formatting issues found. Run: cargo fmt --all"
                return 1
        fi
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

        if cargo audit; then
                log_success "Security audit passed"
        else
                log_warning "Security issues detected"
                return 1
        fi
}

run_loc_check() {
        log_step "Checking Rust source file sizes..."

        if [ ! -x "scripts/check-rs-loc.sh" ]; then
                log_warning "scripts/check-rs-loc.sh not found or not executable (skipping)"
                return 0
        fi

        if bash scripts/check-rs-loc.sh; then
                log_success "LOC check passed"
        else
                log_error "LOC check failed"
                return 1
        fi
}

run_header_check() {
        log_step "Checking SPDX license headers..."

        if [ ! -f "scripts/check-headers.sh" ]; then
                log_error "scripts/check-headers.sh not found"
                return 1
        fi

        if bash scripts/check-headers.sh; then
                log_success "Header check passed"
        else
                log_error "Header check failed"
                return 1
        fi
}

run_version_anti_pattern_check() {
        log_step "Checking for hardcoded version-string anti-patterns..."

        if [ ! -f "scripts/check-version-anti-patterns.sh" ]; then
                log_error "scripts/check-version-anti-patterns.sh not found"
                return 1
        fi

        if bash scripts/check-version-anti-patterns.sh; then
                log_success "Version anti-pattern check passed"
        else
                log_error "Version anti-pattern check failed (use env!(\"CARGO_PKG_VERSION\") instead)"
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
        run_clippy || ((failed++))
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
╔════════════════════════════════════════════════════════════════╗
║          Cosmostrix Build Script - Stellar 4.0                ║
╚════════════════════════════════════════════════════════════════╝

USAGE:
    ./scripts/build.sh [COMMAND] [OPTIONS]

COMMANDS:
    debug           Build debug version (default)
    release         Build optimized release version
    release-debug   Build release with debug symbols
    pgo             PGO nitro build (instrument → benchmark → optimize, +5-15% FPS)
    verify-release  Build and verify Linux x86_64 release variants
    test            Run test suite
    bench           Run benchmarks

    check           Quick checks (fmt + clippy)
    check-all       Comprehensive checks (fmt + clippy + test + audit)
    fmt             Format code
    clean           Clean build artifacts
    update          Update dependencies and audit

    all             Full pipeline (check + debug + release + test)
    ci              CI pipeline (check-all + release)
    stats           Show build cache statistics
    help            Show this help

OPTIONS:
    --no-cache      Disable build caching
    --verbose       Enable verbose output

ENVIRONMENT VARIABLES:
    COSMOSTRIX_JOBS     Override CPU core limit (default: auto)
    COSMOSTRIX_TARGET   Override build target (default: rustc host target)
    RUST_BACKTRACE      Control backtrace verbosity (default: 1)

EXAMPLES:
    ./scripts/build.sh release                  # Build release version
    ./scripts/build.sh verify-release           # Build and verify v1/v2/v3/v4 artifacts
    ./scripts/build.sh check-all                # Run all quality checks
    ./scripts/build.sh ci                       # Run CI pipeline
    COSMOSTRIX_JOBS=4 ./scripts/build.sh all    # Full build with 4 cores
    ./scripts/build.sh --verbose release        # Verbose release build

TOOLS INTEGRATION:
    sccache   - Build caching (install: cargo install sccache)
    nextest   - Fast test runner (install: cargo install cargo-nextest)
    audit     - Security auditing (install: cargo install cargo-audit)

EOF
}

# Parse options (options can appear anywhere)
VERBOSE=0
NO_CACHE=0
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

# ── PGO (Profile-Guided Optimization) nitro build ───────────────────────
# Two-stage: instrument → benchmark → recompile with profile data.
# Expected gain: 5-15% FPS improvement over the pro profile.
build_pgo() {
        log_step "Starting PGO nitro build (2-stage: instrument → profile → optimize)"

        local pgo_dir="${PWD}/target/pgo-data"
        local instrument_bin="target/${TARGET}/pgo-instrument/${PROJECT_NAME}"
        local nitro_bin="target/${TARGET}/pgo-use/${PROJECT_NAME}"

        # Stage 1: Build instrumented binary
        log_info "Stage 1/3: Building instrumented binary..."
        mkdir -p "${pgo_dir}"
        export COSMOSTRIX_BUILD="nitro-pgo-instrument"
        export COSMOSTRIX_PROFILE="pgo-instrument"
        export COSMOSTRIX_LTO="off"
        export COSMOSTRIX_STRIP="no"
        # Preserve target-cpu if set via COSMOSTRIX_TARGET_CPU (for v3/v4 PGO)
        local cpu_flag=""
        if [ -n "${COSMOSTRIX_TARGET_CPU:-}" ]; then
            cpu_flag="-C target-cpu=${COSMOSTRIX_TARGET_CPU}"
            log_info "PGO target CPU: ${COSMOSTRIX_TARGET_CPU}"
        fi
        export RUSTFLAGS="${cpu_flag} -C profile-generate=${pgo_dir}"

        if ! cargo build --profile pgo-instrument --target "${TARGET}" --jobs "${MAX_JOBS}"; then
                log_error "Stage 1 failed: instrumented build failed"
                exit 1
        fi
        log_success "Stage 1 complete: instrumented binary built"

        # Stage 2: Run benchmark to collect profile data
        log_info "Stage 2/3: Running benchmark to collect profile data (10s)..."
        if [ ! -f "${instrument_bin}" ]; then
                log_error "Stage 2 failed: instrumented binary not found at ${instrument_bin}"
                exit 1
        fi

        if ! "${instrument_bin}" --benchmark --bench-duration 10 2>/dev/null; then
                log_warn "Benchmark exited with non-zero status (may be normal in CI)"
        fi

        local profile_count
        profile_count=$(find "${pgo_dir}" -name "*.profraw" 2>/dev/null | wc -l)
        if [ "${profile_count}" -eq 0 ]; then
                log_error "Stage 2 failed: no profile data collected in ${pgo_dir}"
                log_info "Hint: ensure the benchmark ran for at least 5 seconds"
                exit 1
        fi
        log_success "Stage 2 complete: ${profile_count} profile file(s) collected"

        # Merge profile data
        local profdata_file="${pgo_dir}/cosmostrix.profdata"
        local profdata_tool=""
        if command -v llvm-profdata >/dev/null 2>&1; then
                profdata_tool="llvm-profdata"
        else
                # Try rustup llvm-tools
                local rustup_profdata
                rustup_profdata="$(rustc --print sysroot 2>/dev/null)/lib/rustlib/$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')/bin/llvm-profdata"
                if [ -x "${rustup_profdata}" ]; then
                        profdata_tool="${rustup_profdata}"
                fi
        fi
        if [ -n "${profdata_tool}" ]; then
                log_info "Merging profile data with ${profdata_tool}..."
                "${profdata_tool}" merge -o "${profdata_file}" "${pgo_dir}"/*.profraw
        else
                log_warn "llvm-profdata not found. Install with: rustup component add llvm-tools-preview"
                log_info "Using raw profdata directory (rustc can handle this)"
                profdata_file="${pgo_dir}"
        fi

        # Stage 3: Build optimized binary with profile data
        log_info "Stage 3/3: Building PGO-optimized nitro binary..."
        export COSMOSTRIX_BUILD="nitro-pgo"
        export COSMOSTRIX_PROFILE="pgo-use"
        export COSMOSTRIX_LTO="fat"
        export COSMOSTRIX_STRIP="yes"
        export RUSTFLAGS="${cpu_flag} -C profile-use=${profdata_file}"

        if ! cargo build --profile pgo-use --target "${TARGET}" --jobs "${MAX_JOBS}"; then
                log_error "Stage 3 failed: PGO-optimized build failed"
                exit 1
        fi

        local size
        size=$(du -h "${nitro_bin}" | cut -f1)
        log_success "PGO nitro build complete (${size})"
        log_info "Binary: ${nitro_bin}"
        log_info "Profile data: ${pgo_dir}"
        echo ""
        log_info "PGO gain: expected 5-15% FPS improvement over pro profile"
        log_info "Run: ${nitro_bin} --benchmark to measure"
}

# Main execution
main() {
        # Ensure we're in a Rust project
        if [ ! -f "Cargo.toml" ]; then
                log_error "Not in a Rust project directory (Cargo.toml not found)"
                exit 1
        fi

        # Setup environment
        if [ $NO_CACHE -eq 0 ]; then
                setup_build_cache
        fi

        local command="${COMMAND:-debug}"

        if [ ${#ARGS[@]} -ne 0 ]; then
                log_error "Unexpected extra arguments: ${ARGS[*]}"
                echo ""
                show_help
                exit 1
        fi

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
        check-all|--check-all)
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
