# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/build/build.sh — not a standalone executable (no main, no
#   argument parsing; the entry script owns both).
#
# COSMOSTRIX BUILD MODULE: help
# the `build.sh help` text (heredoc). Kept in its own module so the
# entry script stays dispatch-only.
#
# Part of the build.sh split (NIGHT-lts-2): the single-entry orchestrator
# was de-monolithed into sourced modules; function bodies are byte-identical
# moves from the former flat script.

show_help() {
	cat <<'EOF'
cosmostrix build script

USAGE:
    ./scripts/build/build.sh [COMMAND] [OPTIONS]

Version bumping is handled by ./scripts/release/version-to.sh — see its --help for
details. The recommended one-shot bump+build flow is:

    ./scripts/release/version-to.sh vX.Y.Z && ./scripts/build/build.sh release

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
                    ./scripts/release/version-to.sh --check <cargo-toml-version>)
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
    --quiet, -q     Suppress passing output in check-all: hide [INFO]/[OK]/
		    [>] step banners, Miri status, section dividers, and
		    per-check success summaries. Only failures ([X]),
		    warnings ([!]) and underlying-tool violation output
		    (cargo fmt diffs, clippy errors, test failures, LOC
		    VIOLATES lines, version-anti-pattern FAIL blocks)
		    are surfaced. Use in CI and fast local re-checks.

ENVIRONMENT:
    COSMOSTRIX_JOBS             Override CPU core limit (default: all detected cores)
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
    ./scripts/build/build.sh release                  # optimized release build
    ./scripts/build/build.sh check-all                # all quality gates
    ./scripts/build/build.sh pgo --auto               # PGO with auto CPU detection
    cargo use-pgo                               # same as above
    COSMOSTRIX_JOBS=4 ./scripts/build/build.sh all    # full pipeline, 4 cores
    ./scripts/build/build.sh version-sync             # verify all version refs in sync

    # Bump version + build in one flow (version-to.sh handles the bump):
    ./scripts/release/version-to.sh vX.Y.Z && ./scripts/build/build.sh release
    ./scripts/release/version-to.sh vX.Y.Z && ./scripts/build/build.sh pgo --auto

OPTIONAL TOOLS (auto-detected, silently skipped if absent):
    sccache   - Build caching       (cargo install sccache)
    mold/lld  - Fast linker         (system package manager)
    nextest   - Fast test runner    (cargo install cargo-nextest)
    audit     - Security auditing   (cargo install cargo-audit)

MIRI VERIFICATION:
    Miri (https://github.com/rust-lang/miri) detects undefined behavior in
    unsafe Rust. It runs under the nightly toolchain, which is auto-installed
    on first `./scripts/build/build.sh miri` invocation. The full test suite under
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
    to STALE). To force a re-verify: ./scripts/build/build.sh miri

EOF
}
