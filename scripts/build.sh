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
                log_info "Build cache: $(IFS=', '; echo "${bits[*]}")"
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

run_shellcheck() {
        log_step "Running shellcheck on scripts/*.sh..."

        if ! command -v shellcheck >/dev/null 2>&1; then
                log_warning "shellcheck not installed (skipping). Install: apt install shellcheck or brew install shellcheck"
                return 0
        fi

        if shellcheck scripts/*.sh; then
                log_success "Shellcheck passed"
        else
                log_error "Shellcheck failed — fix warnings before committing"
                return 1
        fi
}

run_python_lint() {
        log_step "Running ruff check + format on scripts/*.py..."

        if ! command -v ruff >/dev/null 2>&1; then
                log_warning "ruff not installed (skipping Python lint). Install: pip install ruff"
                return 0
        fi

        local py_failed=0
        if ! ruff check scripts/*.py; then
                log_error "ruff check failed — fix Python lint issues"
                ((py_failed++))
        fi

        if ! ruff format --check scripts/*.py; then
                log_error "ruff format check failed — run 'ruff format scripts/*.py' to fix"
                ((py_failed++))
        fi

        if [ $py_failed -eq 0 ]; then
                log_success "Python lint + format passed"
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
        run_shellcheck || ((failed++))
        run_python_lint || ((failed++))
        run_version_sync || ((failed++))
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
Cosmostrix build script

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
                    Shortcut: cargo use-pgo
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

EOF
}

# ── Version sync (verification only) ───────────────────────────────────
# Version bumping is owned by ./scripts/version-to.sh — see its --help for
# the full list of files it touches (Cargo.toml, Cargo.lock, PKGBUILD,
# .SRCINFO, README.md, docs/workflow/about-ci.md). build.sh only exposes
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

        if "${bumper}" --check "${current}"; then
                log_success "All active version refs agree with Cargo.toml (v${current})"
        else
                log_error "Version desync detected — run './scripts/version-to.sh v${current}' to fix"
                return 1
        fi
}

# Parse options (options can appear anywhere)
VERBOSE=0
NO_CACHE=0
PGO_AUTO=0
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
        log_step "Starting PGO nitro build (2-stage: instrument → profile → optimize)"

        local pgo_dir="${PWD}/target/pgo-data"
        local instrument_bin="target/${TARGET}/pgo-instrument/${PROJECT_NAME}"
        local nitro_bin="target/${TARGET}/pgo-use/${PROJECT_NAME}"

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

        # Stage 1: Build instrumented binary
        log_info "Stage 1/3: Building instrumented binary..."
        mkdir -p "${pgo_dir}"
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

        # Stage 2: Run benchmark to collect profile data
        log_info "Stage 2/3: Running benchmark to collect profile data (10s)..."
        if [ ! -f "${instrument_bin}" ]; then
                log_error "Stage 2 failed: instrumented binary not found at ${instrument_bin}"
                exit 1
        fi

        if ! "${instrument_bin}" --benchmark --bench-duration 10 2>/dev/null; then
                log_warning "Benchmark exited with non-zero status (may be normal in CI)"
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
                log_warning "llvm-profdata not found. Install with: rustup component add llvm-tools-preview"
                log_info "Using raw profdata directory (rustc can handle this)"
                profdata_file="${pgo_dir}"
        fi

        # Stage 3: Build optimized binary with profile data
        log_info "Stage 3/3: Building PGO-optimized nitro binary..."
        export COSMOSTRIX_BUILD="nitro-pgo"
        export COSMOSTRIX_PROFILE="pgo-use"
        export COSMOSTRIX_LTO="fat"
        export COSMOSTRIX_STRIP="yes"
        # Use final_cpu (the matrix-specified or auto-detected target) for the
        # shipping binary. This may be more aggressive than instrument_cpu
        # (e.g., v4 vs v3) — that's fine, the binary never runs on the host.
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

        # Setup environment for anything that actually builds or tests.
        if [ $NO_CACHE -eq 0 ]; then
                setup_build_cache
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
