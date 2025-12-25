#!/bin/bash
# =============================================================================
# COSMOSTRIX BUILD AUTOMATION SCRIPT
# =============================================================================
# Optimized build script with intelligent core detection and advanced caching
# Author: rezky_nightky
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

readonly TARGET="${COSMOSTRIX_TARGET:-${LYVOXA_TARGET:-$(default_target)}}"
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

MAX_JOBS="${COSMOSTRIX_JOBS:-${LYVOXA_JOBS:-$(calculate_jobs)}}"
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

    if ! command -v rustup &> /dev/null; then
        log_error "rustup not installed. Install from: https://rustup.rs"
        exit 1
    fi

    if ! command -v rustc &> /dev/null; then
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
    if command -v sccache &> /dev/null; then
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
    if command -v mold &> /dev/null; then
        export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=mold"
        log_success "mold linker enabled (faster linking)"
    elif command -v lld &> /dev/null; then
        export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=lld"
        log_success "lld linker enabled"
    else
        log_warning "Fast linker not found (mold/lld)."
    fi

    # Setup cargo-nextest if available
    if command -v cargo-nextest &> /dev/null; then
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
    if command -v cargo-audit &> /dev/null; then
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

    if cargo build --target "${TARGET}" --jobs "${MAX_JOBS}"; then
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

    if cargo build --release --target "${TARGET}" --jobs "${MAX_JOBS}"; then
        local binary="target/${TARGET}/release/${PROJECT_NAME}"
        local size
        size=$(du -h "$binary" 2>/dev/null | cut -f1 || echo "unknown")
        log_success "Release build complete (${size})"
        echo "  └─ Binary: ${binary}"

        # Strip binary for smaller size (optional)
        if command -v strip &> /dev/null && [ -f "$binary" ]; then
            local before
            local after
            before=$(stat -f%z "$binary" 2>/dev/null || stat -c%s "$binary" 2>/dev/null)
            strip "$binary" || true
            after=$(stat -f%z "$binary" 2>/dev/null || stat -c%s "$binary" 2>/dev/null)
            if [ -n "${before:-}" ] && [ -n "${after:-}" ] && [ "$before" -ge "$after" ]; then
                local saved=$(((before - after) / 1024))
                log_info "Stripped binary (saved ${saved}KB)"
            fi
        fi
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

    if ! command -v cargo-audit &> /dev/null; then
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

run_deny_check() {
    log_step "Checking dependency policies..."

    if ! command -v cargo-deny &> /dev/null; then
        log_warning "cargo-deny not installed (skipping). Install: cargo install cargo-deny --locked"
        return 0
    fi

    if [ ! -f "deny.toml" ]; then
        log_warning "deny.toml not found (skipping cargo-deny). Add deny.toml to enforce policies."
        return 0
    fi

    if cargo deny check all; then
        log_success "Dependency policy checks passed"
    else
        log_error "Dependency policy violations found"
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
    run_clippy || ((failed++))
    run_tests || ((failed++))
    run_audit || ((failed++))
    run_deny_check || ((failed++))

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

    if command -v sccache &> /dev/null; then
        sccache --zero-stats 2>/dev/null || true
    fi

    log_success "Build artifacts cleaned"
}

show_cache_stats() {
    if command -v sccache &> /dev/null; then
        echo ""
        log_info "=== Build Cache Statistics ==="
        sccache --show-stats
    else
        log_warning "sccache not available"
    fi
}

run_benchmark() {
    log_step "Running benchmarks..."

    if cargo bench --no-fail-fast; then
        log_success "Benchmarks complete"
    else
        log_error "Benchmarks failed"
        return 1
    fi
}

show_help() {
    cat << 'EOF'
╔════════════════════════════════════════════════════════════════╗
║          Cosmostrix Build Script - Stellar 4.0                ║
╚════════════════════════════════════════════════════════════════╝

USAGE:
    ./build.sh [COMMAND] [OPTIONS]

COMMANDS:
    debug           Build debug version (default)
    release         Build optimized release version
    release-debug   Build release with debug symbols
    test            Run test suite
    bench           Run benchmarks

    check           Quick checks (fmt + clippy)
    check-all       Comprehensive checks (fmt + clippy + test + audit + deny)
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
    ./build.sh release                  # Build release version
    ./build.sh check-all                # Run all quality checks
    ./build.sh ci                       # Run CI pipeline
    COSMOSTRIX_JOBS=4 ./build.sh all    # Full build with 4 cores
    ./build.sh --verbose release        # Verbose release build

TOOLS INTEGRATION:
    sccache   - Build caching (install: cargo install sccache)
    nextest   - Fast test runner (install: cargo install cargo-nextest)
    audit     - Security auditing (install: cargo install cargo-audit)
    deny      - Dependency policies (install: cargo install cargo-deny)

EOF
}

# Parse options (options can appear anywhere)
VERBOSE=0
NO_CACHE=0
COMMAND=""

ARGS=()
while [ $# -gt 0 ]; do
    case "$1" in
        --verbose|-v)
            VERBOSE=1
            export RUST_BACKTRACE=full
            shift
            ;;
        --no-cache)
            NO_CACHE=1
            unset RUSTC_WRAPPER
            shift
            ;;
        help|-h|--help)
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
        bench|benchmark)
            check_rust_toolchain
            run_benchmark
            ;;
        check)
            check_rust_toolchain
            run_quick_check
            ;;
        check-all)
            run_comprehensive_check
            ;;
        ci)
            run_comprehensive_check
            build_release
            ;;
        fmt|format)
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
        help|-h|--help)
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
